//! Garbage collection
//!
//! Contains the trait GC used by the network.

use crate::eole::net::*;

mod erasink;
mod nogc;

pub use erasink::EraSinkGC;
pub use nogc::NoGC;

/// Trait for the garbage collector.
pub trait GC:Sized {

    /// Create a new GC instance
    fn new()->Self;

    // --- --- ---

    /// Allow the GC to act on the network before starting the reduction loop.
    /// When called, the network only contains one node, the root, at index 0.
    /// Note: If the GC create special nodes, it should place them after the root
    ///       e.g. at index 1, 2, 3...
    fn init(net:&mut Net<Self>);

    /// Check an edge before it is added in the network.
    /// Allow the GC to take specific action when an edge originates/targets a special node.
    /// Must returns `true' if the network must insert the edge, and `false' if it must not.
    fn check_edge(net:&mut Net<Self>, src:Vertex, tgt:Vertex)->bool;

    /// Mark a vertex to be collected.
    /// Collection do not need to start immediately.
    fn to_collect(net:&mut Net<Self>, v:Vertex);

    /// Starts a round of collection.
    /// Called in the reduction loop if a call to 'to_collect' was made.
    fn do_gc(net:&mut Net<Self>);


    // --- --- --- Statistics

    /// Get the number of collected nodes
    fn get_nb_collected(&self) -> u64;

    /// Get the GC statistics
    fn get_stats(&self) -> String;
}
