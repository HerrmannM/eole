use super::compactor::{self, Compactor};
use super::gc::GC;
use super::net::{self, *};

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
/// the result usually conatins non-reduced redex. This is not convenient to see what is going on.
/// The full reducer will avoid that.
/// Note that this is not an implementation of a "strict" (or "eager") evaluation as the argument
/// in a function call is not reduced before the function.
pub fn get_reducer_full<'a, MyGC: GC, MyCPTR: Compactor>(
    should_compact: &'a dyn Fn(&Net<MyGC>)->bool,
    mut action: Box<dyn FnMut(&Net<MyGC>, (usize, usize, &Vec<Vertex>)) + 'a>
    ) ->  impl FnMut(&mut Net<MyGC>, bool, usize) + 'a {

    move |net:&mut Net<MyGC>, test_credit:bool, mut credit:usize|{

        // Set of root to visit. Each root comes with a "history" represented by a FanStack.
        let mut roots: Vec<(Vertex, FanStack)> = vec![(Net::<MyGC>::ROOT_VERTEX, vec![])];

        while !roots.is_empty() {
            let (mut root, mut fstack) = roots.pop().unwrap();
            let mut stack: Vec<Vertex> = vec![net.follow(root)];
            let mut backtracking = false;

            // println!("\nRoots:\n{:?}", roots);

            while !stack.is_empty() {
                if test_credit && credit == 0 { break; }

                // Check how we are coming in
                let entry = stack.pop().unwrap();
                let (index, port) = entry.as_tuple();
                let port = port.0;
                let node = net.get_node(index);

                // println!("Root = {:?}   Entry = {:?}     Backtrack = {:?}", root, entry, backtracking);

                match &node.0 {
                    NodeKind::CstrK(c) => {
                        //crate::conversion::do_graph_inter(net, folder_path, step, index, root.get_index(), &stack);
                        match c {
                            // Abstraction: if port 0
                            //   Entry by main: pass throufh
                            //            aux2: backtrack
                            CstrK::Abs(_, _) => {
                                assert!(!backtracking, "Backtracking on a constructor");
                                match port {
                                    0 => {
                                        stack.push(net.follow(mkv(index, 1)));
                                    }
                                    2 => {
                                        backtracking = true;
                                    }
                                    _ => {
                                        panic!("{:?} Reaching an abstraction by the body", entry);
                                    }
                                }
                            }

                            // Fan out: must be paired
                            CstrK::FanOut(l) => {
                                if backtracking {
                                    // Backtracking: do nothing here
                                    assert!(port != 0, "Reaching a saved fan by the main port");
                                } else {
                                    // Check the port:
                                    // If MAIN: We enter the fanout. Get the exit port from the fstack
                                    // If AUX: fanout was saved for further reductions
                                    //         save it again, and re-exit by the same port (ie same
                                    //         vertex)
                                    if port == 0 {
                                        // MAIN: new entry case
                                        match lookup_port(&mut fstack, *l) {
                                            None => {
                                                // crate::conversion::do_graph_inter(net, folder_path, step, index, root.get_index(), &stack);
                                                panic!("FanOUT {:?} {:?} not paired", index, node);
                                            }
                                            Some(p) => {
                                                stack.push(mkv(index, p.0));
                                                stack.push(net.follow(mkv(index, p.0)));
                                            }
                                        }
                                    } else {
                                        // AUX: re-entry case
                                        stack.push(entry);
                                        stack.push(net.follow(entry));
                                    }
                                }
                            }
                        }
                    }

                    NodeKind::DstrK(d) => {
                        // Check the target of the main port
                        let main = main(index);
                        let target = net.follow(main);
                        let (target_index, target_port) = target.as_tuple();
                        let target_port = target_port.0;
                        let target_node = net.get_node(target_index);

                        if target_port == 0 {
                            assert!(!backtracking, "Backtracking on a interaction");
                           //   Special action if fanin-fanout? fstack is growing to infinity
                           //   with example lije delta delta (which is not too surprising, but can
                           //   we do better??)
                           // Special action if we have an interacting faned fan in:
                           // If we reached that one while "backtracking" the stack (double quote,
                           // because this is not while backtracking=true, but a poping operation)
                           // then we need to pop the fan stack. We can test this by looking at
                           // the index stored in the fab stack
                           if let DstrK::FanIn(FIStatus::Labeled(l)) = d {
                               match fstack.pop(){
                                   None => {} // Pop it; push it back if non matching index
                                   Some((ls,p,i)) => if index != i { fstack.push((ls,p,i));  }
                               }
                           }


                            // Manage the credit
                            if test_credit {credit-=1;}
                            // If reaching the target of the main port, *must* be a constructor.
                            // Action (graph printing)
                            action(net, (index, root.get_index(), &stack) );
                            // Interact
                            let constr = as_constr(net, target_index);
                            let d = *d; // ease the borrow checker
                            net.interact(index, d, target_index, constr);
                            // GC and compaction
                            MyGC::do_gc(net);
                            // Compact network + reduction state
                            if should_compact(net) {
                                let mut cptr = MyCPTR::new();
                                cptr.init(net);
                                cptr.compact(net);
                                stack = stack.iter().map(|&x| cptr.adjust_v(x)).collect::<Vec<_>>();
                                root = cptr.adjust_v(root);
                                roots = roots.into_iter().map(|x| (cptr.adjust_v(x.0), x.1)).collect();
                                fstack = fstack.into_iter().map(|x|(x.0, x.1, cptr.adjust_i(x.2))).collect();
                            }



                        } else {
                            if backtracking {
                                match d {
                                    DstrK::Apply => {
                                        let to_push = mkv(index, 2);
                                        if to_push != root {
                                            roots.push((to_push, fstack.clone()));
                                        }
                                    }
                                    DstrK::FanIn(FIStatus::Labeled(l)) => {
                                        fstack.pop();
                                    }
                                    DstrK::FanIn(FIStatus::Stem) => {}
                                }
                            } else {
                                match d {
                                    DstrK::Apply => {
                                        if root == mkv(index, 2) {
                                            stack.push(net.follow(mkv(index, 2))); // Looping over the root
                                        } else {
                                            stack.push(entry); // For backtracking
                                            stack.push(net.follow(main)); // Next one
                                        }
                                    }

                                    DstrK::FanIn(FIStatus::Stem) => {
                                        stack.push(entry); // For backtracking
                                        stack.push(net.follow(main)); // Next one
                                    }

                                    DstrK::FanIn(FIStatus::Labeled(l)) => {
                                        stack.push(entry); // For backtracking
                                        stack.push(net.follow(main)); // Next one
                                        fstack.push((*l, Port(port), index)); // Stack the association label<->port
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
                // END OF MATCH

                // Before looping: handle special with the root
                if stack.is_empty() && !backtracking {
                    stack.push(net.follow(root));
                }
            }
        }
    }
}



// --- Helper for the above implementation

/// StackFan, used to re tecord the label of a crossed fan in and the port used to enterd it.
/// Also store the index of the node pushing the fan
/// Unlike in a metal concert, those fans do not autonomously stack themselves.
type FanStack = Vec<(Label, Port, usize)>;

/// Get the port for a label in the stack of fan_in.
/// If found, remove the item from the stack
#[inline]
fn lookup_port(stack: &mut FanStack, lb: Label) -> Option<Port> {
    // Warning! this is a stack: use rposition (reverse, starting from the end)
    match stack.iter().rposition(|&itm| itm.0 == lb) {
        None => None,
        Some(idx) => {
            let res = stack.get(idx).map(|lp| (lp.1));
            stack[idx].0 = 0;
            res
        }
    }
}

#[inline]
fn as_constr<MyGC: GC>(net: &Net<MyGC>, index: usize) -> CstrK {
    match &net.get_node(index).0 {
        NodeKind::CstrK(c) => c.clone(),
        _ => panic!("As Constructor: found something else"),
    }
}
