//! # Do nothing implementation of GC

use std::fmt::Write;

#[macro_use]
use crate::helpers::*;

use crate::eole::net::*;
use crate::eole::gc::GC;

/// Erase-Sink GC.
pub struct NoGC {
}

/// Implementation block EraSink
impl NoGC {

    /// Constant Erase index: 1.
    pub const ERASE_INDEX:usize = 1;

    /// Constant Erase vertex: the "body" port (aux 1) of an Abstraction
    pub const ERASE_VERTEX:Vertex = mkv(Self::ERASE_INDEX, 1);
}

/// Implementation of the GC trait for EraSink
impl GC for NoGC {

    /// Create a new instance of the GC
    #[inline]
    fn new()->Self { NoGC { } }


    /// Get the number of collected nodes
    #[inline]
    fn get_nb_collected(&self) -> u64 { 0 }

    /// Get the GC statistics
    #[inline]
    fn get_stats(&self) -> String {
        let mut res = String::new();
        write!(&mut res, "GC Details           : NO GC\n");
        res
    }

    /// Allow the GC to act on the network before starting the reduction loop.
    /// Add an ERASE node (index 1).
    #[inline]
    fn init(net:&mut Net<Self>){
        net.new_abs(String::from("ERASE"), false);  // Index 1
    }

    /// Check an edge before it is added to the network.
    /// Do nothing, always returns true.
    #[inline]
    fn check_edge(net:&mut Net<Self>, src:Vertex, tgt:Vertex)->bool{ true }

    /// Mark a vertex to be collected.
    /// Update the value of the vertex with ERASE
    #[inline]
    fn to_collect(net:&mut Net<Self>, v:Vertex){
            net.update_vertex(v, Self::ERASE_VERTEX);
    }

    /// Starts a round of collection.
    /// Called in the reduction loop if a call to 'to_collect' was made.
    #[inline]
    fn do_gc(net:&mut Net<Self>){}
}
