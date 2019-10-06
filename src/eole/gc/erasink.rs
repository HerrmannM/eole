//! # Erase-Sink Garbage CollectorNetwork

use std::mem;
use std::fmt::Write;

#[macro_use]
use crate::helpers::*;

use crate::eole::net::{self, *, NodeKind::*, CstrK::*, DstrK::*};
use crate::eole::gc::GC;



/// Erase-Sink GC.
pub struct EraSinkGC {

    /// List of vertex linked to the special "ERASE" node (entry points for the GC)
    pub to_erase:Vec<Vertex>,
    pub to_erase_list:Vec<Vertex>,

    /// List of vertex linked to the special "SINK" node (entry points for the GC)
    pub to_sink:Vec<Vertex>,

    // --- --- --- Statistics

    /// Number of nodes removed through erasing
    pub nb_remove_erase :u64,

    /// Number of nodes removed through sinking
    pub nb_remove_sink :u64,

    /// Number of `erase' calls
    pub nb_erase: u64,

    /// Number of `sink' calls
    pub nb_sink: u64,
}


/// Implementation block EraSink
impl EraSinkGC {

    /// Constant Sink index: 1.
    pub const SINK_INDEX:usize  = 1;

    /// Constant Sink vertex: the "binder" port (aux 2) of an Abstraction
    pub const SINK_VERTEX:Vertex = mkv(Self::SINK_INDEX, 2);

    /// Constant Erase index: 2.
    pub const ERASE_INDEX:usize = 2;

    /// Constant Erase vertex: the "body" port (aux 1) of an Abstraction
    pub const ERASE_VERTEX:Vertex = mkv(Self::ERASE_INDEX, 1);


    // --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- ---

    /// Check if a node is alive:
    /// Not NULL, and not one of the special GC's nodes.
    #[inline]
    fn is_alive(v:Vertex)->bool {
        v != Net::<Self>::NULL && v != Self::SINK_VERTEX && v != Self::ERASE_VERTEX
    }

    /// Set the target to be erased.
    /// Do not erase the SINK.
    #[inline]
    fn erase(net:&mut Net<Self>, tgt:Vertex){
        if tgt != Self::SINK_VERTEX {
            assert!(Self::is_alive(tgt));
            net.update_vertex(tgt, Self::ERASE_VERTEX);
            net.gc.to_erase.push(tgt);
        }
    }

    /// Set the source to be sunk.
    /// Do not sink the ERASE.
    #[inline]
    fn sink(net:&mut Net<Self>, src:Vertex){
        if src != Self::ERASE_VERTEX {
            assert!(Self::is_alive(src));
            net.update_vertex(src, Self::SINK_VERTEX);
            net.gc.to_sink.push(src);
        }
    }



    // --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- ---
    // NOTE: When running the sink and the erase, we must test that the target is indeed linked
    //       to ERASE or SINK, or that it is not linked to NULL.
    //       Cecause of loops in the graph, the vertex may be part of a node already collected.
    //       Example: id := !i->i
    //                We collect the asbtraction, the target of the body AUX/1 is added in the
    //                collection list, (i.e. we sink AUX/2), then we remove the node.
    //                Next collection loop: we sink AUX/2 which is NULL.

    /// GC: Erase a target vertex
    fn run_erase(net:&mut Net<Self>, tgt:Vertex){
        if net.get_vertex(tgt) == Self::ERASE_VERTEX {
            // Stats
            net.gc.nb_erase += 1;
            let (tgt_index, tgt_port) = tgt.as_tuple();
            match net.get_node(tgt_index).0.clone() {

                CstrK(c) => match c {

                    // Abstraction:
                    //    Check that we are not erasing the sink (which is also an abstraction)
                    //    Can be erase by the main port (0) or the binder port (2) but not the body!
                    //    Main port: erase the body, sink the binder (if bound). Warning: binder: check reverse edge!
                    //    Binder port: mark the abstraction as unbound, DO NOT remove the node! (will be paired with an abs)
                    Abs(vname, bound) => {
                        assert_ne!(tgt_port.0, 1, "Cannot erase an abstraction by the body");
                        if tgt_port.0 == 0 { // Main port
                            let a1 = net.get_vertex(mkv(tgt_index, 1));
                            // if Self::is_alive(a1) { Self::erase(net, a1); }
                            { Self::erase(net, a1); }
                            if bound {
                                let a2 = net.get_vertex(mkv(tgt_index, 2));
                                // if Self::is_alive(a2) { Self::sink(net, a2); }
                                { Self::sink(net, a2); }
                            }
                            // Stats & cleaning
                            net.gc.nb_remove_erase += 1;
                            net.remove(tgt_index);

                        } else { // Binder port
                            assert!(bound, "Erasing by an unbound variable port");
                            net.nodes[tgt_index].0 = CstrK(Abs(vname, false));
                        }
                    }

                    // Fan out:
                    //    Check that we erase by the main port
                    //    Erase the two auxs
                    FanOut(lbl) => {
                        assert_eq!(tgt_port.0, 0, "Erasing a fan out by an auxiliary port.");
                        let a1 = net.get_vertex(mkv(tgt_index, 1));
                        let a2 = net.get_vertex(mkv(tgt_index, 2));
                        //if Self::is_alive(a1) { Self::erase(net, a1); }
                        //if Self::is_alive(a2) { Self::erase(net, a2); }
                         { Self::erase(net, a1); }
                         { Self::erase(net, a2); }
                        // Stats & cleaning
                        net.gc.nb_remove_erase += 1;
                        net.remove(tgt_index);
                    }

                } //  End of  Constr(c) => match c

                DstrK(d) => match d {

                    // Apply:
                    //    Check that we erase from A1
                    //    Erase the MAIN and the ARG
                    Apply => {
                        assert_eq!(tgt_port.0, 1, "erasing a apply by a port other then A1.");
                        let a0 = net.get_vertex(mkv(tgt_index, 0));
                        let a2 = net.get_vertex(mkv(tgt_index, 2));
                        // if Self::is_alive(a0) { Self::erase(net, a0); }
                        // if Self::is_alive(a2) { Self::erase(net, a2); }
                        { Self::erase(net, a0); }
                        { Self::erase(net, a2); }
                        // Stats & cleaning
                        net.gc.nb_remove_erase += 1;
                        net.remove(tgt_index);
                    }

                    // Fan In:
                    //    Check that we erase by an aux port.
                    //    If the other port is also being erased, erase MAIN
                    //    Else, if (differentiated) not stem write back
                    //    Else, if stem, update the linking: remove the fan in and reconnect the "living side" to main.
                    FanIn(status) => {
                        assert_ne!(tgt_port.0, 0, "erasing a fan in by the MAIN port");
                        let other_port:u8 = if tgt_port.0 == 1 {2} else {1};
                        let other = net.get_vertex(mkv(tgt_index, other_port));

                        // Other should not be NULL (node not remove yet)
                        // Other should not be sinking ("wrong" edge direction)
                        assert_ne!(other, Net::<Self>::NULL, "Other is NULL");
                        assert_ne!(other, Self::SINK_VERTEX, "Other is SINKING");

                        // In any case, if the other port is being erased, erase everything
                        if other == Self::ERASE_VERTEX {
                            let a0 = net.get_vertex(mkv(tgt_index, 0));
                            // if Self::is_alive(a0) { Self::erase(net, a0); }
                            { Self::erase(net, a0); }
                            // Stats & cleaning
                            net.gc.nb_remove_erase += 1;
                            net.remove(tgt_index);
                        } else {
                            // Just to be sure:
                            assert!(Self::is_alive(other), "Other should be alive");
                            match status {
                                // Nothing to do here
                                FIStatus::Labeled(_) => {}
                                // Update linking
                                FIStatus::Stem => {
                                    // Update linking
                                    net.stitch_old_old(mkv(tgt_index, other_port), main(tgt_index));
                                    // Stats & cleaning
                                    net.gc.nb_remove_erase += 1;
                                    net.remove(tgt_index);
                                }
                            }
                        }
                    }// End of FanIn()
              } // End of Destr(d) => match d
            }
        } else {
            assert_eq!(net.get_vertex(tgt), Net::<Self>::NULL, "Vertex to be erased should be ERASE or NULL");
        }
    }



    /// GC: Sink a source vertex
    fn run_sink(net:&mut Net<Self>, src:Vertex) {
        if net.get_vertex(src) == Self::SINK_VERTEX {
            // Stats
            net.gc.nb_sink += 1;
            let (src_index, src_port) = src.as_tuple();
            match net.get_node(src_index).0.clone() {

                CstrK(c) => { }

                DstrK(d) => match d {

                    // Apply:
                    //    Sinking by the main port: Sink UP (A1, reverse edge), erase the ARG (A2)
                    //    Sinking by the arg port: Sink UP (A1, reverse edge), erase the MAIN
                    Apply => {
                        assert_ne!(src_port.0, 1, "Sinking an apply by Aux1");
                        // We always sink A1
                        let a1 = net.get_vertex(mkv(src_index, 1));
                        // if Self::is_alive(a1){ Self::sink(net, a1); }
                        { Self::sink(net, a1); }

                        if src_port.0 == 0 { // Sinking by the main port: erase the arg
                            let a2 = net.get_vertex(mkv(src_index, 2));
                            // if Self::is_alive(a2){ Self::erase(net, a2); }
                            { Self::erase(net, a2); }
                        } else { // Sinking by the arg: erase the main
                            let a0 = net.get_vertex(mkv(src_index, 0));
                            // if Self::is_alive(a0){ Self::erase(net, a0); }
                            { Self::erase(net, a0); }
                        }

                        // Stats & cleaning
                        net.gc.nb_remove_sink += 1;
                        net.remove(src_index);
                    }


                    // FanIn:
                    //    Check that we sink by the main port
                    //    Sink the two auxs (reverse edges lookup)
                    FanIn(status) => {
                        assert_eq!(src_port.0, 0, "Sinking a fan in by an aux port.");
                        let a1 = net.get_vertex(mkv(src_index, 1));
                        let a2 = net.get_vertex(mkv(src_index, 2));
                        // if Self::is_alive(a1){ Self::sink(net, a1); }
                        // if Self::is_alive(a2){ Self::sink(net, a2); }
                        { Self::sink(net, a1); }
                        { Self::sink(net, a2); }
                        // Stats & cleaning
                        net.gc.nb_remove_sink += 1;
                        net.remove(src_index);
                    }// End of FanIn()


              } // End of Destr(d) => match d
            }

        } else {
            assert_eq!(net.get_vertex(src), Net::<Self>::NULL, "Vertex to be sink should be SINK or NULL");
        }
    }
}



/// Implementation of the GC trait for EraSink
impl GC for EraSinkGC {

    /// Create a new instance of the GC
    #[inline]
    fn new()->Self {
        EraSinkGC {
            // --- --- ---
            to_erase:vec![],
            to_erase_list:vec![],

            to_sink:vec![],

            // --- --- ---
            nb_remove_erase:0,
            nb_remove_sink:0,
            nb_erase:0,
            nb_sink:0,
        }
    }


    /// Get the number of collected nodes
    #[inline]
    fn get_nb_collected(&self) -> u64 {
        self.nb_remove_erase + self.nb_remove_sink
    }

    /// Get the GC statistics
    #[inline]
    fn get_stats(&self) -> String {
        let mut res = String::new();

        write!(&mut res, "GC Details           :\n");
        write!(&mut res, "    Removed              : {}\n", self.nb_remove_erase+self.nb_remove_sink);
        write!(&mut res, "        Erase                : {}\n", self.nb_remove_erase);
        write!(&mut res, "        Sink                 : {}\n", self.nb_remove_sink);
        write!(&mut res, "    Calls                : {}\n", self.nb_erase+self.nb_sink);
        write!(&mut res, "        Erase                : {}\n", self.nb_erase);
        write!(&mut res, "        Sink                 : {}\n", self.nb_sink);

        res
    }



    /// Allow the GC to act on the network before starting the reduction loop.
    /// Add the SINK (index 1) and the ERASE (index 2) nodes.
    #[inline]
    fn init(net:&mut Net<Self>){
        net.new_abs(String::from("SINK"), false);   // Index 1
        net.new_abs(String::from("ERASE"), false);  // Index 2
    }

    /// Check an edge before it is added to the network.
    /// Allow the GC to take specific action when an edge originates/targets a special node.
    /// Must returns `true' if the network must insert the edge, and `false' if it must not.
    #[inline]
    fn check_edge(net:&mut Net<Self>, src:Vertex, tgt:Vertex)->bool{
        if tgt.get_index() == Self::SINK_INDEX {
            // Source -> SINK
            Self::sink(net, src);
            false
        } else if src.get_index() == Self::ERASE_INDEX {
            // ERASE -> Target
            Self::erase(net, tgt);
            false
        } else {
            true
        }
    }

    /// Mark a vertex to be collected.
    /// Collection do not need to start immediately.
    #[inline]
    fn to_collect(net:&mut Net<Self>, v:Vertex){
        /// Simply transmit the call to `erase'
        Self::erase(net, v);
    }

    /// Starts a round of collection.
    /// Called in the reduction loop if a call to 'to_collect' was made.
    #[inline]
    fn do_gc(net:&mut Net<Self>){

        // Loop while we need to collect vertices
        while(! (net.gc.to_erase.is_empty() && net.gc.to_sink.is_empty()) ){

            if (!net.gc.to_erase.is_empty()){

                // This code use a raw pointer, bailing out of the borrow checker.
                // This allows us to use avoir recreating a new vec.
                // To be tested on a large scale to see if this is a real win (probably not).
                //
                // mem::swap(&mut net.gc.to_erase_list, &mut net.gc.to_erase);
                // let to_erase_list:*mut Vec<_> = &mut net.gc.to_erase_list;
                // unstem {
                //     let it = (*to_erase_list).iter();
                //     net.gc.to_erase.clear();
                //     it.for_each(|v| Self::run_erase(net, *v));
                // }

                 let mut erase_list:Vec<Vertex> = vec![];
                 mem::swap(&mut erase_list, &mut net.gc.to_erase);
                 erase_list.iter().for_each(|&v| Self::run_erase(net, v));
            }

            if (!net.gc.to_sink.is_empty()){
                let mut sink_list:Vec<Vertex> = vec![];
                mem::swap(&mut sink_list, &mut net.gc.to_sink);
                sink_list.iter().for_each(|&v| Self::run_sink(net,v));
            }
        }
    }
}

