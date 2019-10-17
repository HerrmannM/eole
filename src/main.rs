// Dev: remove dead code warning at the create level
#![allow(dead_code)]
#![allow(unused)]

// --- --- --- Command line
// Command line tool
#[macro_use]
extern crate clap;
use clap::{App, Arg};

fn get_cli<'a, 'b>() -> clap::App<'a, 'b> {
    App::new("Éole")
        .version(crate_version!())
        .author("Matthieu Herrmann")
        .about("A lambda calculus evaluator `Jean-Jacques Lévy'-optimal.")
        .bin_name("eole")
        .setting(clap::AppSettings::ArgRequiredElseHelp)
        .setting(clap::AppSettings::NextLineHelp)
        // Input files
        .arg(Arg::with_name("INPUTS")
             .required(true)
             .multiple(true)
             .value_name("input files")
             .help("Input files.")
        )
        // Verbose mode
        .arg(Arg::with_name("VERBOSE")
             .short("v")
             .long("verbose")
             .help("Verbose mode")
        )
        // Limit the read-back
        .arg(
            Arg::with_name("LIMIT_L")
                .short("l")
                .long("lambda-limit")
                .max_values(1)
                .value_name("conversion limit")
                .validator(as_usize)
                .help("Limit the depth of the lambda expression when performing the read-back")
        )
        // Limit the reduction
        .arg(
            Arg::with_name("LIMIT_R")
                .short("r")
                .long("reduction-limit")
                .max_values(1)
                .value_name("reduction limit")
                .validator(as_usize)
                .help("Max number of reduction steps")
        )
        // Create an initial and a final graph
        .arg(
            Arg::with_name("GRAPH_FILA")
                .short("g")
                // .min_values(0)
                // .max_values(1)
                // .value_name("graph folder")
                .help("Create graphviz dot files before and after all reductions (creates a 'generated' folder)")
                .conflicts_with("G")
        )
        // Create graphs at each step
        .arg(
            Arg::with_name("GRAPH_ALL")
                .short("G")
                // .min_values(0)
                // .max_values(1)
                // .value_name("graph folder")
                .help("Create graphviz dot files for all reduction states (creates a 'generated' folder)")
        )
        // GC options
        .arg(
            Arg::with_name("GC_MODE")
                .short("m")
                .max_values(1)
                .possible_values(&["erasink", "none"])
                .value_name("GC name")
                .help("Memory option: Set the garbage collector to use (defaults to erasink)")
        )
        // Compactor options
        .arg(
            Arg::with_name("CPTR_MODE")
                .short("M")
                .max_values(1)
                .validator(as_usize)
                .value_name("Compactor ratio")
                .help("Memory option: Set the ratio (in power of 2) of free nodes triggering the compactor. 0 to disable (default).")
        )
        // Reduction mode
        .arg(
            Arg::with_name("RED_STRATEGY")
                .short("s")
                .max_values(1)
                .possible_values(&["lazy", "full"])
                .value_name("strategy name")
                .help("Reduction strategy option: Set reduction strategy (default to full).")
        )
}

// Validators
fn as_usize(v: String) -> Result<(), String> {
    match v.parse::<usize>() {
        Err(_) => Err(format!("The value must be a positive integer")),
        _ => Ok(()),
    }
}

#[inline]
fn get_cli_usize(matches: &clap::ArgMatches, name: &str) -> Option<usize> {
    matches.value_of(name).map(|s| s.parse::<usize>().unwrap())
}

// Import: eolelib
use eolelib::{
    conversion,
    eole::{
        compactor::{self, Compactor},
        gc::{self, GC},
        net::{self, Net},
        reduce,
    },
};

use eole_parser::parser;

// Import: standard lib
use std::fs::{self, File};
use std::path::Path;
use std::time::{Duration, Instant}; // Use for benchmarking

/// Option: reduction mode
#[derive(Debug)]
enum RedOpt {
    LAZY,
    FULL,
}

/// Option: graph mode
#[derive(Debug)]
enum GraphOpt<'a> {
    NONE,
    FILAS(&'a Path),
    ALL(&'a Path),
}

/// Option: GC mode
#[derive(Debug)]
enum GCOpt {
    NOGC,
    ERASINK,
}

/// Option: Compactor mode
#[derive(Debug)]
enum CptrOpt {
    NONE,
    FACTOR(usize),
}


pub fn main() {
    // --- --- --- Check the args
    let matches = get_cli().get_matches();
    let input_paths: Vec<&str> = matches.values_of("INPUTS").map(|i| i.collect()).unwrap();
    let is_verbose = matches.is_present("VERBOSE");
    let limit_lambda = get_cli_usize(&matches, "LIMIT_L");
    let limit_reduce = get_cli_usize(&matches, "LIMIT_R");

    // Create a closure for the verbose
    let vprint: &dyn Fn(String) = if is_verbose {
        &|s: String| {
            println!("{}", s);
        }
    } else {
        &|s: String| {}
    };

    // Get the graph option: Create the output folder if needed
    let graph_opt = if matches.is_present("GRAPH_FILA") {
        let p = if let Some(folder) = matches.value_of("GRAPH_FILA") {
            Path::new(folder)
        } else {
            Path::new("generated")
        };
        if !p.exists() {
            fs::create_dir(p).expect("Could not create the graph directory");
        }
        GraphOpt::FILAS(p)
    } else if matches.is_present("GRAPH_ALL") {
        let p = if let Some(folder) = matches.value_of("GRAPH_ALL") {
            Path::new(folder)
        } else {
            Path::new("generated")
        };
        if !p.exists() {
            fs::create_dir(p).expect("Could not create the graph directory");
        }
        GraphOpt::ALL(p)
    } else {
        GraphOpt::NONE
    };

    // Get the GC option
    let gc_opt = match matches.value_of("GC_MODE").unwrap_or("erasink") {
        "none" => GCOpt::NOGC,
        "erasink" => GCOpt::ERASINK,
        _ => panic!("Should not happen"),
    };

    // Get the compactor option
    let cptr_opt = if let Some(v) = matches.value_of("CPTR_MODE") {
        let v = v.parse::<usize>().unwrap();
        if v == 0 {
            CptrOpt::NONE
        } else {
            CptrOpt::FACTOR(v)
        }
    } else {
        CptrOpt::NONE
    };

    // Get the strategy option
    let red_opt = match matches.value_of("RED_STRATEGY").unwrap_or("full") {
        "lazy" => RedOpt::LAZY,
        "full" => RedOpt::FULL,
        _ => panic!("Should not happen"),
    };

    // --- --- --- File Parsing and converting
    let path = input_paths.first().unwrap();
    let source = std::fs::read_to_string(path).unwrap();
    let text = parser::TextParser::new().parse(&source).unwrap();

    // --- --- --- Do the work
    // For now, we keep the net in the main...
    type MyCPTR = compactor::Interval;
    let res = match gc_opt {
        GCOpt::ERASINK => {
            type MyGC = gc::EraSinkGC;
            type MyNet = net::Net<MyGC>;

            let mut net = conversion::to_network::<MyGC>(&text);
            file_run::<MyGC, MyCPTR>(red_opt, graph_opt, cptr_opt, limit_reduce, vprint, &mut net);
            conversion::from_net(&net, limit_lambda)
        }

        GCOpt::NOGC => {
            type MyGC = gc::NoGC;
            type MyNet = net::Net<MyGC>;

            let mut net = conversion::to_network::<MyGC>(&text);
            file_run::<MyGC, MyCPTR>(red_opt, graph_opt, cptr_opt, limit_reduce, vprint, &mut net);
            conversion::from_net(&net, limit_lambda)
        }
    };

    match res {
        None => println!("<No conversion done -- lambda limit={:?}>", limit_lambda),
        Some(l)=> println!("{}", l)
    };

}


fn file_run<'a, 'b, MyGC: GC, MyCPTR: Compactor>(
    red_opt: RedOpt,
    graph_opt: GraphOpt<'b>,
    cptr_opt: CptrOpt,
    limit_reduce:Option<usize>,
    vprint: &'a dyn Fn(String),
    // --- --- ---
    mut net: &mut Net<MyGC>,
) {
    // "Should compact" function
    let should_compact: Box<dyn Fn(&Net<MyGC>) -> bool> = match cptr_opt {
        CptrOpt::NONE => Box::new(|net: &Net<MyGC>| false),

        CptrOpt::FACTOR(f) => {
            Box::new(|net: &Net<MyGC>| net.available_indexes.len() << 1 > net.nodes.len())
        }
    };

    // Create the first graph if "FIRST/LAST".
    // Note:    the action is called BEFORE a reduction: the first graph for "ALL" will be done.
    match graph_opt {
        GraphOpt::FILAS(folder) => conversion::do_graph(&net, folder, 0),
        _ => {}
    }

    // Create the counter graph
    let mut stepcell = std::cell::Cell::new(0);

    let step = &mut stepcell;
    let mut reducer: Box<dyn FnMut(&mut Net<MyGC>, bool, usize)> = match red_opt {
        RedOpt::LAZY => {
            // "Action" function, based on the graph_opt
            // Specific per reduction kind
            let do_graph: Box<dyn FnMut(&Net<MyGC>, ((usize, net::DstrK), &Vec<(usize, net::DstrK)>))> = match graph_opt {
                GraphOpt::ALL(folder) => {
                    Box::new(move |net: &Net<MyGC>, extra:((usize, net::DstrK), &Vec<(usize, net::DstrK)>)| {
                        conversion::do_graph_lazy(net, folder, step.get(), extra);
                        *step.get_mut() += 1;
                    })
                }

                _ => Box::new(|net: &Net<MyGC>, extra:((usize, net::DstrK), &Vec<(usize, net::DstrK)>)| {}),
            };
            Box::new(reduce::get_reducer_lazy::<MyGC, MyCPTR>(
                &should_compact,
                do_graph,
            ))
        }

        RedOpt::FULL => {
            // "Action" function, based on the graph_opt
            // Specific per reduction kind
            let do_graph: Box<dyn FnMut(&Net<MyGC>, (usize, &Vec<(net::Vertex, net::NodeKind)>))> =
                match graph_opt {
                    GraphOpt::ALL(folder) => Box::new(
                        move |net: &Net<MyGC>, (idx, s): (usize, &Vec<(net::Vertex, net::NodeKind)>)| {
                            conversion::do_graph_full(net, folder, step.get(), idx, s);
                            *step.get_mut() += 1;
                        },
                    ),

                    _ => Box::new(|net: &Net<MyGC>, extra: _| {}),
                };
            Box::new(reduce::get_reducer_full::<MyGC, MyCPTR>(
                &should_compact,
                do_graph,
            ))
        }
    };

    vprint(format!("Starting reduction..."));
    let now = Instant::now();
    match limit_reduce {
        None => reducer(&mut net, false, 0),
        Some(l) => reducer(&mut net, true, l)
    };
    std::mem::drop(reducer); // Kill the closure, releasing ownership over cell stepcell
    let duration = now.elapsed();
    let mili = duration.subsec_micros() / 1000; // quotient
    let micro = duration.subsec_micros() % 1000; // remainder
    vprint(format!(
        "Done in {}s {:03}ms {:03}μs ",
        duration.as_secs(),
        mili,
        micro
    ));
    vprint(net.print_stats());

    // Create the last graph, if needed
    // Note:    the action is called BEFORE a reduction: we need to produce the last graph
    //          for both "FIRST/LAST" and "ALL" options.
    match graph_opt {
        GraphOpt::FILAS(folder) => conversion::do_graph(&net, folder, 1),
        GraphOpt::ALL(folder) => conversion::do_graph(&net, folder, stepcell.get()),
        _ => {}
    }
}
