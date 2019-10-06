//! # Conversion between éole networks and lambda terms

use std::collections::HashMap;


use eole_parser::ast::{*, Term::*};
use crate::eole::{self, *, net::*, gc::GC};

use std::path::Path;
use std::fs::{self, File};
use std::io::{Write, Error};


/// Status of an abstraction's binder.
#[derive(Clone,Debug)]
enum VUsed {
    /// Mark a binder as unused. Record the associated vertex.
    Unused{binding_vertex:Vertex},
    /// Mark a binder as used. Record the binder's vertex and the user vertex.
    Used{binding_vertex:Vertex, user_vertex:Vertex}
}





/// Convert a lambda expression into a network.
//pub fn to_network<MyGC:GC>(sentences:&Vec<Sentence>, root:&Path) -> Net<MyGC> {
pub fn to_network<MyGC:GC>(sentences:&Vec<Sentence>) -> Net<MyGC> {
    let mut def:Vec<&SLet> = vec![];
    let mut run:Vec<&SRun> = vec![];
    let mut read:Vec<&SRead> = vec![];

    // Gather the sentences...
    for s in sentences {
        match s {
            Sentence::Let(ldef) => def.push(ldef),
            Sentence::Run(lrun) => run.push(lrun),
            Sentence::Read(lread) => read.push(lread)
        }
    };

    // Read the imports
    for r in &read {
        match r {
            // Import without prefix
            SRead{path, name:None} => { unimplemented!(); }

            // Import with prefix
            SRead{path, name:Some(n)} => { unimplemented!(); }
        }
    };



    // Run
    match run.first() {
        None => Net::new(),
        Some(SRun{term}) => {
            // Create a new lambda term with all the definitions
            let base = term.clone();
            let lambda:Box<Term> = def.iter()
                .rev()
                .fold(base,
                      |acc, d|{
                          let SLet{vname, body} = d;
                            Box::new(App{fun:Box::new(Lambda{vname:vname.clone(), body:acc}), arg:body.clone()})
                      });
            //println!("{}", &lambda);
            // Convert
            to_network1(&lambda)
        }
    }

}


fn to_network1<MyGC:GC>(term:&Box<Term>) -> Net<MyGC> {
    // Create a new empty net
    let mut n = Net::<MyGC>::new();
    let up = Net::<MyGC>::ROOT_VERTEX;
    // TODO: Embed the rundef under all the definition
    // Convert:
    let mut env = HashMap::new();
    to_network_(term, up, &mut n, &mut env);
    n
}

fn to_network_<MyGC:GC>(term:&Box<Term>, up:Vertex, net: &mut Net<MyGC>, env: &mut HashMap<String, VUsed>) {
    match **term {

        // --- Symbol
        Sym {ref vname} => {
            // Check if the variable has been used.
            match env.get(vname).expect(&format!("Symbol {} not found.", vname)) {

                // Never used before: link 'up->λ/Aux2' and update the environment.
                // 'up' becomes the "user port" of the binder.
                VUsed::Unused{binding_vertex} => {
                    let p = *binding_vertex;
                    env.insert(vname.clone(), VUsed::Used{binding_vertex:p, user_vertex:up});
                    // Do the binding
                    net.create_edge(up, p);
                },

                // Used:  Insert a new fan in at the binding vertex : 'Fan In/Main -> λ/Aux2/binding vertex'
                //        Plug up on the fan                        : 'up -> Fan In/Aux1'
                //        Plug the former user on the fan           : 'user_vertex -> Fan In/Aux2'
                //        Update the environment: 'Fan In/Main' is the new user of the vertex.
                VUsed::Used{binding_vertex, user_vertex} => {
                    let p = *binding_vertex;
                    let u = *user_vertex;
                    //let (user_v, user_p) = as_tuple(u);
                    // Create a new fan in without a label
                    let fin = net.new_fin(FIStatus::Stem);
                    // Update the linking
                    net.create_edge(main(fin), p);
                    net.create_edge(up, mkv(fin, 1));
                    net.create_edge(u, mkv(fin, 2));
                    // Update the environment: 'Fan In/Main' is the new user of the vertex.
                    env.insert(vname.clone(), VUsed::Used{binding_vertex:p, user_vertex:main(fin)});
                }
            }
        }

        // Abstraction
        Lambda {ref vname, ref body} => {
            // Checking and declaring
            if env.contains_key(vname) {
                panic!(format!("Variable {} already declared", vname));
            }
            let abs = net.new_abs(vname.clone(), true); // Used by default
            // Update the environment
            env.insert(vname.clone(), VUsed::Unused{binding_vertex:mkv(abs, 2)});
            // Go in the body with 'up = λ/Aux1'
            to_network_(body, mkv(abs, 1), net, env);
            // Check if the variable is used: mark it has unused if not.
            match env.get(vname).unwrap() {
                VUsed::Used{..} => (),// Nothing to do
                VUsed::Unused{..} => {
                    net.nodes[abs].0 = NodeKind::CstrK(CstrK::Abs(vname.clone(), false));
                }
            };
            // Clean the env
            env.remove(vname);
            // Link the 'up->λ/Main'
            net.create_edge(up, main(abs));
        }

        // Application
        // Create a new node and link it. Then go down in the fun and the arg:
        //  'up     -> @/Aux1'
        //  '@/M    -> fun'
        //  '@/Aux2 -> arg'
        App {ref fun, ref arg} => {
            // Create the node
            let app = net.new_app();
            // Update the linking
            net.create_edge(up, mkv(app, 1));
            // Launch recursively with the good 'up'
            to_network_(fun, main(app), net, env);
            to_network_(arg, mkv(app, 2), net, env);
        }
    }

}












/// Public conversion function.
/// Assume that the network is plugged on 'net_root', which is the case if you used the 'to_net' function.
pub fn from_net<MyGC:GC>(net:&Net<MyGC>, limit:Option<usize>) -> Option<Box<Term>> {
    let mut stack = vec![];
    from_net_(net, &mut stack, Net::<MyGC>::ROOT_VERTEX, limit)
}

/// Record the label of a crossed fan in and the port used to enterd it.
type FanStack = Vec<(Label, Port)>;

/// Get the port for a label in the stack of fan_in.
/// If found, remove the item from the stack
/// If not found, abort
//fn lookup_port(stack:&mut FanStack, lb:Label) -> u8 {
//    let idx:usize = stack.iter().position(|&itm| itm.0 == lb).expect("No matching label found");
//    let res:u8 = stack.get(idx).unwrap().1;
//    stack.remove(idx);
//    res
//}


/// Get the port for a label in the stack of fan_in.
/// If found, remove the item from the stack
fn lookup_port(stack:&mut FanStack, lb:Label) -> Option<Port> {
    // Warning! this is a stack: use rposition (reverse, starting from the end)
    match stack.iter().rposition(|&itm| itm.0 == lb){
        None => None,
        Some(idx) => {
            let res = stack.get(idx).map(|lp| (lp.1) );
            stack[idx].0=0;
            res
        }
    }
}



/// When converting from a network, it is important to be able to check the edge,
/// i.e. both the source and target vertex.
/// The source vertex is the argument, and the target vertex (which represent the current node)
/// is looked up in the graph.
fn from_net_<MyGC:GC>(net:&Net<MyGC>, stack:&mut FanStack, src:Vertex, limit:Option<usize>) -> Option<Box<Term>> {
    // Check the limit
    let lim = match limit {
        None => Some(None),
        Some(l) => if l == 0 { None } else { Some(Some(l-1)) }
    };
    // Build the term if limit is acceptable
    lim.and_then(|new_limit|{
        // Access the target of src
        let tgt = net.follow(src);
        let (tgt_index, tgt_port) = tgt.as_tuple();
        let tgt_node = net.get_node(tgt_index);

        match &tgt_node.0 {
            // Constructors
            NodeKind::CstrK(kind) => match kind {

                // λ: Check where we are comming in from:
                // Should not enter an abstraction through the body
                CstrK::Abs(vname, bound) => {
                    assert_ne!(tgt_port.0, 1, "Should not enter an abstraction node through the body");
                    let bname = String::from(vname) + &tgt_index.to_string();
                    if tgt_port.0 == 0 {
                        // Entering by 'up' (Main): analyse the body and create the abstraction.
                        from_net_(net, stack, mkv(tgt_index, 1), new_limit)
                        .map(|body|{ Box::new(Lambda{vname:bname, body}) })
                    } else {
                        Some(Box::new(Sym{vname:bname}))
                    }
                }

                // Fan out: lookup the associated port and follow it.
                CstrK::FanOut(label) => {
                    assert_eq!(tgt_port.0, 0, "Should not enter a fan out node through an auxiliary port");
                    match lookup_port(stack, *label) {
                        Some(port) => {
                            // println!("Index {}    Label {}    out {:?}      {:?}", tgt_index, *label, port, stack);
                            from_net_(net, stack, mkv(tgt_index, port.0), new_limit)
                        }
                        None => Some(Box::new(Sym{vname:String::from("∆")}))
                    }
                }
            }

            // Destructors
            NodeKind::DstrK(kind) => match kind {

                // Application
                // Should not be entered by the port 1
                DstrK::Apply => {
                    assert_eq!(tgt_port.0, 1, "Should enter an app through aux1 port");
                    let mut st = stack.clone();
                    from_net_(net, &mut st, main(tgt_index), new_limit)
                    .and_then(|fun|
                        from_net_(net, stack, mkv(tgt_index, 2), new_limit)
                        .and_then(|arg| Some(Box::new(App{fun,arg})) )
                    )
                }

                // Fan in: If paired, record the port we went entered.
                // Should not be entered by the main port
                DstrK::FanIn(in_status) => {
                    assert_ne!(tgt_port.0, 0, "Should not enter a fan in node through the main port");
                    // Record the pair label/port if it is a paired fan in
                    match in_status {
                        FIStatus::Labeled(l) => {
                            stack.push((*l, tgt_port));
                            // println!("Index {}    Label {:?}    push {:?}", tgt_index, *l, tgt_port);
                            // Exit by the main port
                            let res = from_net_(net, stack, main(tgt_index), new_limit);
                            stack.pop();
                            res
                        }
                        FIStatus::Stem => from_net_(net, stack, main(tgt_index), new_limit)
                    }
                }
            }// End of Destr(kind) => match kind
        }// End of match &tgtNode.0
    })// End of closure
}





pub fn to_graphviz<MyGC:GC>(net:&Net<MyGC>, output: &mut File, step:usize, as_string:String, extra:String){

    // Intro text
    write!(output,
r#"
digraph graph{} {{
    newrank = true;
    ranksep = "1 equally";
    label="Step {}"; //: {}";
    labelloc=top;
    labeljust=left;

    graph [resolution=256, fontsize=12, nodesep=0.75];

    edge [dir=normal, fontsize=18, labeldistance=2, labelfloat=true, penwidth=1.5];

    node [peripheries=1, nodesep=10.5, margin=0];

    // --- --- --- Nodes
"#, step, step, as_string);

    // Nodes
    for (idx, n) in net.nodes.iter().enumerate() {

        if idx >= net.nb_special_nodes &&  n.1 == [Net::<MyGC>::NULL;3] { continue; }

        let col = get_color(net.nb_special_nodes, idx, &n.0);

        match &n.0 {
            NodeKind::CstrK(c) => {
                match &c {
                    CstrK::Abs(s,b) => {
                        let s = format!("{} λ{}{}", idx, String::from(if *b { "" } else {"●  "}), s);
                        write!(output, "    {} [color=\"{}\", shape=egg, label=\"{}\"];\n", get_node_name(idx, &n.0),col, s);
                    }

                    CstrK::FanOut(l) => {
                        let s = format!("{} ▲ {}", idx, l);
                        write!(output, "    {} [color=\"{}\", shape=septagon, label=\"{}\"];\n", get_node_name(idx, &n.0), col, s);
                    }
                }
            }

            NodeKind::DstrK(d) => {
                match &d {
                    DstrK::Apply => {
                        let s = format!("{} @", idx);
                        write!(output, "    {} [color=\"{}\", shape=ellipse, label=\"{}\"];\n", get_node_name(idx, &n.0), col, s);
                    }

                    DstrK::FanIn(status) => {
                        let s = format!("{} ▼ {}", idx, if let FIStatus::Labeled(l) = status { format!("{}", l)} else {format!("stem")} );
                        write!(output, "    {} [color=\"{}\", shape=septagon, orientation=180, label=\"{}\"];\n", get_node_name(idx, &n.0), col, s);
                    }
                }
            }
        }
    }

    // Edges
    write!(output, "\n    // --- --- --- Edges\n");
    for (idx, n) in net.nodes.iter().enumerate() {

        let targets = n.1;

        if idx >= net.nb_special_nodes &&  targets == [Net::<MyGC>::NULL;3] { continue; }

        let src_main = mkv(idx, 0);
        let src_aux1 = mkv(idx, 1);
        let src_aux2 = mkv(idx, 2);

        let tgt_main = targets[0];
        let (tgt_main_idx, tgt_main_port) = tgt_main.as_tuple();

        let tgt_aux1 = targets[1];
        let (tgt_aux1_idx, tgt_aux1_port) = tgt_aux1.as_tuple();

        let tgt_aux2 = targets[2];
        let (tgt_aux2_idx, tgt_aux2_port) = tgt_aux2.as_tuple();


        match &n.0 {
            NodeKind::CstrK(c) => match &c {
                CstrK::Abs(_,b) => {
                    if idx > tgt_main_idx { write_edge(output, net, tgt_main, src_main); }
                    if idx > tgt_aux1_idx { write_edge(output, net, src_aux1, tgt_aux1); }
                    if idx >= tgt_aux2_idx { write_edge(output, net, tgt_aux2, src_aux2); } // >= for identity
                }

                CstrK::FanOut(_) => {
                    if idx > tgt_main_idx { write_edge(output, net, tgt_main, src_main); }
                    if idx > tgt_aux1_idx { write_edge(output, net, src_aux1, tgt_aux1); }
                    if idx > tgt_aux2_idx { write_edge(output, net, src_aux2, tgt_aux2); }
                }
            }


            NodeKind::DstrK(d) => match &d {
                DstrK::Apply => {
                    if idx > tgt_main_idx { write_edge(output, net, src_main, tgt_main); }
                    if idx > tgt_aux1_idx { write_edge(output, net, tgt_aux1, src_aux1); }
                    if idx > tgt_aux2_idx { write_edge(output, net, src_aux2, tgt_aux2); }
                }

                DstrK::FanIn(_) => {
                    if idx > tgt_main_idx { write_edge(output, net, src_main, tgt_main); }
                    if idx > tgt_aux1_idx { write_edge(output, net, tgt_aux1, src_aux1); }
                    if idx > tgt_aux2_idx { write_edge(output, net, tgt_aux2, src_aux2); }
                }

            }
        }
    }

    write!(output, "\n    // --- --- --- Extra\n");
    write!(output, "{}", extra);

    write!(output, "\n\n}}\n\n");
}






fn get_node_name(index:usize, nk:&NodeKind)-> String {
    match nk {
        NodeKind::CstrK(c) => {
            match &c {
                CstrK::Abs(s,b) => format!("abs{}", index),
                CstrK::FanOut(l) => format!("fout{}", index)
            }
        }

        NodeKind::DstrK(d) => {
            match &d {
                DstrK::Apply => format!("app{}", index),
                DstrK::FanIn(status) => format!("fin{}", index)
            }
        }
    }
}



fn get_color(nb_special_nodes:usize, index:usize, nk:&NodeKind)->&str{
    if index < nb_special_nodes {
        "gray27"
    } else {
        match nk {
            NodeKind::CstrK(_) => {
                let i = index%6;
                ["aquamarine", "cadetblue1", "cyan3", "cornflowerblue", "dodgerblue2", "deepskyblue1"][i]
            }

            NodeKind::DstrK(_) => {
                let i = index%6;
                ["deeppink", "hotpink1", "indianred1", "lightsalmon2", "orange2", "tan"][i]
            }
        }
    }
}

fn get_compass<MyGC:GC>(net:&Net<MyGC>, v:Vertex) -> &str {
    let (index, port) = v.as_tuple();
    let port = port.0;
    match &net.get_node(index).0 {
        NodeKind::CstrK(c) => {
            match &c {
                CstrK::Abs(s,b) => {
                    if      port == 0   { ":n" }    // Up
                    else if port == 1   { ":e" }    // Body
                    else                { ":w" }    // Variable
                }

                CstrK::FanOut(l) =>{
                    if      port == 0   { ":n"  }   // Up
                    else if port == 1   { ":se" }   // Aux1
                    else                { ":sw" }   // Aux2
                }
            }
        }

        NodeKind::DstrK(d) => {
            match &d {
                DstrK::Apply =>{
                    if      port == 0 { ":sw" }  // Function
                    else if port == 1 { ":n"  }  // Up
                    else              { ":se" }  // Arg
                }

                DstrK::FanIn(status) => {
                    if      port == 0 { ":s"  }  // Down
                    else if port == 1 { ":nw" }  // Aux1
                    else              { ":ne" }  // Aux2
                }
            }
        }

    }
}









fn write_edge<MyGC:GC>(output: &mut File, net: &Net<MyGC>, src:Vertex, tgt:Vertex){

    if src == Net::<MyGC>::NULL || tgt == Net::<MyGC>::NULL { return; }

    let (src_idx, src_port) = src.as_tuple();
    let (tgt_idx, tgt_port) = tgt.as_tuple();

    let src_node = net.get_node(src_idx);
    let tgt_node = net.get_node(tgt_idx);

    // Special case for identity
    if src_idx == tgt_idx {
        write!(output, "    {}:s -> {}:s [{}];\n",
            get_node_name(src_idx, &src_node.0),
            get_node_name(tgt_idx, &tgt_node.0),
            get_edge_attr(src, tgt, get_color(net.nb_special_nodes, src_idx, &src_node.0))
        );

    } else {
        write!(output, "    {}{} -> {}{} [{}];\n",
            get_node_name(src_idx, &src_node.0),
            get_compass(net, src),
            get_node_name(tgt_idx, &tgt_node.0),
            get_compass(net, tgt),
            get_edge_attr(src, tgt, get_color(net.nb_special_nodes, src_idx, &src_node.0))
        );
    }

}

fn get_edge_attr(src:Vertex, tgt:Vertex, mut color:&str) -> String {
    let mut penwidth = "1.5";
    if src.get_port().0 == 0 && tgt.get_port().0 == 0 {
        color = "red";
        penwidth = "4";
    }
    format!("color=\"{}\", penwidth={}, taillabel={}, headlabel={}", color, penwidth, get_end_label(src, color), get_end_label(tgt, color))
}


/// Helper to generate the tail/head label of an edge.
fn get_end_label(v:Vertex, color:&str) -> String {
    let s = (
        if v.get_port().0 == 0 { "M" }
        else if v.get_port().0 == 1 { "A1" }
        else  { "A2" }
    );
    format!("<<font color=\"{}\"><b>{}</b></font>>", color, s)
}





pub fn do_graph<MyGC:GC>(net: &Net<MyGC>, folder_path:&Path, step:usize){

    // Check for the folder path
    if !folder_path.exists() {
        fs::create_dir(folder_path).expect("Could not create the graph directory");
    }

    // Create the file
    let file_path = folder_path.join(format!("graph_{:06}.dot",step));
    let mut outfile = File::create(&file_path).expect("Could not create the dot graph file");

    let txt = match file_path.to_str() {
        None => format!("Step {}", step),
        Some(s) =>  format!("Step {}    --    {}", step, s)
    };

    // Create the graph
    to_graphviz(net, &mut outfile, step, txt, String::new());
}


pub fn do_graph_lazy<MyGC:GC>(net: &Net<MyGC>, folder_path:&Path, step:usize,
                               (current, stack):((usize, DstrK), &Vec<(usize, DstrK)>)){
    // Create the file
    let file_path = folder_path.join(format!("graph_{:06}.dot",step));
    let mut outfile = File::create(&file_path).expect("Could not create the dot graph file");

    let txt = match file_path.to_str() {
        None => format!("Step {}", step),
        Some(s) =>  format!("Step {}    --    {}", step, s)
    };

    // Extra:
    let mut extra = String::new();

    // Put current (to interact) in green
    let (idx, _) = current;
    let dn = get_node_name(idx, &net.get_node(idx).0);
    extra.push_str(&format!("    {}[color=green, penwidth=5];\n", dn));

    // Stack in violet
    for (idx, _) in stack {
        let nn = get_node_name(*idx, &net.get_node(*idx).0);
        extra.push_str(&format!("    {}[color=violet, penwidth=5];\n", nn));
    };

    // Create the graph
    to_graphviz(net, &mut outfile, step, txt, extra);
}





pub fn do_graph_full<MyGC:GC>(net: &Net<MyGC>, folder_path:&Path, step:usize,
                               destr_index:usize, root_index:usize, stack:&Vec<Vertex>){

    // Create the file
    let file_path = folder_path.join(format!("graph_{:06}.dot",step));
    let mut outfile = File::create(&file_path).expect("Could not create the dot graph file");

    let txt = match file_path.to_str() {
        None => format!("Step {}", step),
        Some(s) =>  format!("Step {}    --    {}", step, s)
    };

    // Extra
    let d = net.get_node(destr_index);
    let dn = get_node_name(destr_index, &d.0);
    let mut extra:String = (
        format!("    {}[color=green, penwidth=5];\n", dn)
    );

    let r = net.get_node(root_index);
    let rn = get_node_name(root_index, &r.0);
    extra.push_str(&format!("    {}[color=black, penwidth=5];\n", rn));

    for v in stack {
        let i = v.get_index();
        let nn = get_node_name(i, &net.get_node(i).0);
        extra.push_str(&format!("    {}[color=violet, penwidth=5];\n", nn));
    }

    // Create the graph
    to_graphviz(net, &mut outfile, step, txt, extra);
}
