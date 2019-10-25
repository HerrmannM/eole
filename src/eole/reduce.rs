use super::compactor::{self, Compactor};
use super::gc::GC;
use super::net::{self, *};

use super::super::conversion;

use std::collections::HashMap;
use std::collections::HashSet;
use std::marker::PhantomData;
use std::path::Path;


// --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- ---
// LAZY REDUCER
// --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- ---


/// Return a function implementing a lazy reducer, stopping on a weak head normal form.
/// It corresponds to a "leftmost, outermost" reduction strategy stoping as soon as a non-redex
/// is met. In other words, if the term is a lambda, it does not analyse its body.
///
/// The reduction function maintains an internal stack of destructor to be applied:
///     * If the stack is empty, we look at the root term.
///
///         * If it is a constructor, the reduction is over
///
///         * Else, we push the destructor on the stack and start again the process with a non
///           empty stack.
///
///     * Else, if the stack is non empty, we pop the top, which is a destructor.
///       We test the main port of the destructor:
///
///         * Linked to the main port of a constructor: interaction! The next iteration of the loop
///           will either deal with a previously stacked destructor or reach the empty stack case
///           above. This mechanism is enough for inner constructors to "rise" towards their
///           destructors.
///
///         * Linked to an auxiliary port of a constructor: end of the reduction process.
///           Poping the stack of destructor is useless as we know (see below) that their main
///           port is linked to an other destructor, hence cannot interact.
///           Note: This case is only acceptable for the aux port of an abstraction.
///                 Reaching a fan out by an aux port is an error.
///
///         * Linked to an auxiliary port of an other destructor:
///           Push again the current destructor, then push the reached destructor.
///           The reached destructor will be the current one of the next iteration.
///           Note: Reaching a destructor by its main port is an error.
pub fn get_reducer_lazy<'a, MyGC: GC, MyCPTR: Compactor>(
    should_compact: &'a dyn Fn(&Net<MyGC>)->bool,
    mut action: Box<dyn FnMut(&Net<MyGC>, ((usize,net::DstrK), &Vec<(usize, net::DstrK)>)) + 'a>
    ) ->  impl FnMut(&mut Net<MyGC>, bool, usize) + 'a {

    move |net:&mut Net<MyGC>, test_credit:bool, mut credit:usize|
    {
        // Reduction stack: made of destructors.
        // contains the index and the destructor's info
        let mut stack: Vec<(usize, net::DstrK)> = vec![];

        loop {
            if test_credit && credit == 0 { break; }

            // DEBUG
            if cfg!(debug_assertions) {
                let mut iter = stack.iter();
                iter.for_each(|item| net.search_available_indexes(item.0));
            }

            // Check the top of the stack
            match stack.pop() {
                // Empty stack:
                // Check if the root is linked to a constructor or a destructor
                None => {
                    let index = net.follow(Net::<MyGC>::ROOT_VERTEX).get_index();
                    match net.get_node(index).0 {
                        NodeKind::CstrK(_) => {
                            break;
                        } // Constructor: stop
                        NodeKind::DstrK(d) => {
                            // Push the index of the destructor and its kind, loop.
                            stack.push((index, d))
                        }
                    }
                }

                // Non empty stack:
                // Check the main port of our the destructor
                Some(head) => {
                    let (destr_index, destr_kind) = head;
                    let main = main(destr_index);
                    let (tgt_index, tgt_port) = net.follow(main).as_tuple();
                    match &net.get_node(tgt_index).0 {
                        // Constructor: we have an interaction if on port 0
                        NodeKind::CstrK(c) => {
                            if tgt_port.0 == 0 {
                                // Manage the credit
                                if test_credit {credit-=1;}
                                // Action (Graph printing)
                                action(&net, (head, &stack));
                                // We must use clone() as c may contains a String (Abs case).
                                let c = c.clone();
                                net.interact(destr_index, destr_kind, tgt_index, c);
                                MyGC::do_gc(net);
                                if (should_compact)(net) {
                                    let mut cptr = MyCPTR::new();
                                    cptr.init(net);
                                    cptr.compact(net);
                                    stack = stack .iter() .map(|&x| (cptr.adjust_i(x.0), x.1)) .collect::<Vec<_>>();
                                }
                            } else {
                                break;
                            }
                        }

                        // Destructor: stack and relaunch
                        NodeKind::DstrK(d) => {
                            stack.push(head);
                            stack.push((tgt_index, *d));
                        }
                    }
                }
            }
        }
    }
}



// --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- ---
// FULL REDUCER
// --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- ---

/// Return a function implementing a full reducer.
/// Because the lazy reducer stops as soon as a constructor is at the root of the network,
/// the result usually contains non-reduced redex. This is not convenient to see what is going on.
/// The full reducer will avoid that.
/// Note that this is not an implementation of a "strict" (or "eager") evaluation as the argument
/// in a function call is not reduced before the function.
pub fn get_reducer_full<'a, MyGC: GC, MyCPTR: Compactor>(
    should_compact: &'a dyn Fn(&Net<MyGC>)->bool,
    mut action: Box<dyn FnMut(&Net<MyGC>, (usize, &Vec<(Vertex, net::NodeKind)>)) + 'a>
    ) ->  impl FnMut(&mut Net<MyGC>, bool, usize) + 'a {

    move |net:&mut Net<MyGC>, test_credit:bool, mut credit:usize|{

        // History stack
        let mut history: Vec<(Vertex, net::NodeKind)> = vec![];
        // History occurences
        let mut history_occ:HashMap<usize, i64> = HashMap::new();

        // Main loop
        loop {
            println!("---");
            if test_credit && credit == 0 { break; }

            // Check the history of nodes:
            match history.pop(){
                // Empty: locate the next destructor starting from the root
                None => {
                    match locate_next_destructor(net, &mut history, Net::<MyGC>::ROOT_VERTEX, &mut history_occ) {
                        None => { return; }
                        Some(vert_kind) => {
                            *history_occ.entry(vert_kind.0.get_index()).or_insert(0) += 1;
                            println!("[main loop] Push {:?}", vert_kind);
                            history.push(vert_kind);
                        }
                    }
                }
                // We have something
                Some(head) => {
                    let (vertex, _) = &head;
                    let (index, port) = vertex.as_tuple();
                    let kind = net.get_node(index).0.clone();
                    assert!(net.get_node(index).1!=[Net::<MyGC>::NULL; 3], "Corrupted history: contains a null node. [main loop, history.pop()]");
                    // Occurence change
                    *history_occ.get_mut(&index).expect("[1] Cannot find matching occurrence") -= 1;


                    match &kind {
                        NodeKind::CstrK(CstrK::Abs(_,_)) => { /* */ }

                        NodeKind::CstrK(CstrK::FanOut(l)) => { /* */ }

                        // Destructor: follow main
                        NodeKind::DstrK(d) => {
                            let target_v = net.follow(main(index));
                            let (target_i, target_p) = target_v.as_tuple();
                            assert!(net.get_node(target_i).1!=[Net::<MyGC>::NULL; 3], "Reaching a null node while checking a destructor's main port");
                            match &net.get_node(target_i).0 {
                                // Target Constructor
                                NodeKind::CstrK(c) => {
                                    if target_p.0 == 0 {
                                        // Manage the credit
                                        if test_credit {credit-=1;}

                                        // Occurence check
                                        assert!(*history_occ.entry(index).or_insert(0) == 0, "bam");
                                        assert!(*history_occ.entry(target_i).or_insert(0) == 0, "bam");

                                        // If reaching the target of the main port, *must* be a constructor.
                                        // Action (graph printing)
                                        action(net, (index, &history) );


                                        // Interaction.
                                        let c = c.clone();
                                        net.interact(index, *d, target_i, c);
                                        // GC and compaction
                                        MyGC::do_gc(net);
                                        if (should_compact)(net) {
                                            let mut cptr = MyCPTR::new();
                                            cptr.init(net);
                                            cptr.compact(net);
                                            history.iter_mut().for_each(|x|{ x.0 = (cptr.adjust_v(x.0)); });
                                        }

                                    } else {
                                        // No interaction. Must be an abstraction on port 2
                                        if let CstrK::FanOut(_) = c { panic!("Reaching a fan out by an aux port"); }
                                        assert!(target_p.0 == 2, "Reaching an Abstraction by the body");
                                        // Backtrack until we find an application;
                                        // visit its argument
                                        *history_occ.entry(head.0.get_index()).or_insert(0) += 1;
                                        println!("[Backtract - Repush head] Push {:?}", head);
                                        history.push(head); // Must be done to take care of the current node
                                        loop {
                                            match history.pop(){
                                                None => {
                                                    // dprintln!("Stop with empty history");
                                                    return;
                                                } // End of the process
                                                Some((v,k)) => {
                                                    let hl = history.len();
                                                    let (i,p) = v.as_tuple();

                                                    // Occurence change
                                                    *history_occ.get_mut(&i).expect("[2] Cannot find matching occurrence") -= 1;

                                                    assert!(net.get_node(i).1!=[Net::<MyGC>::NULL; 3], "Corrupted history: contains a null node. [backtrack loop, history.pop()]");
                                                    match k {
                                                        NodeKind::DstrK(DstrK::Apply) => {
                                                            let mut ho = history_occ.clone();
                                                            match locate_next_destructor(&net, &mut history, mkv(i,2), &mut ho) {
                                                                None => { history.truncate(hl); } // loop. Remove items added by locate_next_destructor
                                                                Some(c) => {
                                                                    history_occ = ho;
                                                                    *history_occ.entry(c.0.get_index()).or_insert(0) += 1;
                                                                    println!("[Backtract look args - Push next head] Push {:?}", c);
                                                                    history.push(c);
                                                                    break;
                                                                }
                                                            }
                                                        }
                                                        _ => {} // Loop
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                // Target Destructor
                                // Destructor: stack and relaunch
                                NodeKind::DstrK(d) => {
                                    *history_occ.entry(head.0.get_index()).or_insert(0) += 1;
                                    println!("[Main loop restack destr] Push {:?}", head);
                                    history.push(head);
                                    println!("[Main loop stack next] Push {:?}", (target_v, NodeKind::DstrK(*d)));
                                    *history_occ.entry(target_v.get_index()).or_insert(0) += 1;
                                    history.push((target_v, NodeKind::DstrK(*d)));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Get the "next" destructor following base.
/// Also update the history as it go down the graph.
/// On failure, the history must be restored (i.e. truncated) back to its original length.
#[inline]
fn locate_next_destructor<MyGC:GC>(
    net:&Net::<MyGC>, history:&mut Vec<(Vertex, net::NodeKind)>, mut base:Vertex,
    history_occ: &mut HashMap<usize, i64>
    ) -> Option<(Vertex, net::NodeKind)> {
    loop {
        let next_v = net.follow(base);
        let (next_i, next_p) = next_v.as_tuple();
        assert!(net.get_node(next_i).1!=[Net::<MyGC>::NULL; 3], "Reaching a null node while looking for a next destructor");
        let next_n = net.get_node(next_i);
        //
        match &next_n.0 {
            NodeKind::CstrK(CstrK::Abs(_,_)) => match next_p.0 {
                0 => {
                    *history_occ.entry(next_i).or_insert(0) += 1;
                    history.push((next_v, next_n.0.clone()));
                    println!("[locate destr] Push {:?}", (next_v, next_n.0.clone()));
                    base = mkv(next_i, 1);
                }
                2 => {return None;}
                _ => {panic!("Reaching an abstraction by the body");}
            }
            NodeKind::CstrK(CstrK::FanOut(l)) => {
                assert!(next_p.0 == 0, "Fan out must be entered by the main port");
                match get_matching_fan(net, *l, history) {
                    None => {
                        let path = Path::new("generated");
                        conversion::do_graph(net, path, 999999);
                        panic!("Cannot pair fan out {:?}\n{:?}",(next_i, l), history);
                    }
                    Some(p) => {
                        *history_occ.entry(next_i).or_insert(0) += 1;
                        history.push((next_v, next_n.0.clone()));
                        println!("[locate destr] Push {:?}", (next_v, next_n.0.clone()));
                        base = Vertex::new(next_i, p);
                    }
                }

            }
            NodeKind::DstrK(d) => {
                return Some((next_v, NodeKind::DstrK(*d)));
            }
        }
    }
}


#[inline]
pub fn get_matching_fan<MyGC:GC>(net:&Net::<MyGC>, fan_out_l:i64, history:&Vec<(Vertex, net::NodeKind)>) -> Option<Port> {
    let mut lab_skip:HashMap<i64, i64> = HashMap::new();

    for (v, _) in (history.iter()).rev() {
        let k = net.get_node(v.get_index()).0.clone();
        assert!(net.get_node(v.get_index()).1!=[Net::<MyGC>::NULL; 3], "Corrupted history: contains a null node. [matching fan history.iter()]");
        match &k {
            NodeKind::CstrK(CstrK::FanOut(l)) => {
                *lab_skip.entry(l.abs()).or_insert(0) += 1;
            }

            NodeKind::DstrK(DstrK::FanIn(FIStatus::Labeled(l))) => {
                match lab_skip.get_mut(&l.abs()) {
                    None => {
                        if l.abs() == fan_out_l.abs() { return Some(v.get_port()); }
                    }
                    Some(nb) => {
                        // Found
                        if *nb == 0 {
                            if l.abs() == fan_out_l.abs() { return Some(v.get_port()); }
                        }
                        else { *nb -=1; }
                    }
                }
            }
            _ => {}
        }
    }

    // Not found
    return None;
}
