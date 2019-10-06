//! Network Compactor
//! This is not a garbage collector!
//! Its job is to compact the size of the "nodes" removing the "available indexes".
//! This requires to update the index contained in vertices.

use super::net::*;
use super::gc::GC;

mod interval;
mod mapped;

pub use interval::Interval;
pub use mapped::Mapped;

pub trait Compactor {

    /// Create a new compactor
    fn new()->Self;

    /// Initialisation of a compactor.
    /// Should gather the informations required to performe the compaction.
    /// This code is allowed to modify "net.available_indexes".
    fn init<MyGC:GC>(&mut self, net:&mut Net<MyGC>);

    /// Adjust an index.
    /// The index must be "valid"  (not in "available_indexes")
    fn adjust_i(&mut self, index:usize) -> usize;

    /// Adjust a vertex.
    /// The vertex must be "valid" (it's index not in "available_indexes")
    fn adjust_v(&mut self, v:Vertex) -> Vertex;

    /// Adjust all the vertices in a node.
    /// The node must be "valid" (all it's vertices must be "valid")
    #[inline]
    fn adjust_n(&mut self, n:&Node) -> Node {
        Node(n.0.clone(), [
                self.adjust_v(n.1[0]),
                self.adjust_v(n.1[1]),
                self.adjust_v(n.1[2])
            ]
       )
    }


    /// Compact the network.
    /// After this function executes:
    ///     * "net.nodes" MUST be compacted
    ///     * "net.available_indexes" MUST be empty
    fn compact<MyGC:GC>(&mut self, net:&mut Net<MyGC>){

        if net.max_node_len < net.nodes.len() {
            net.max_node_len = net.nodes.len();
        }

        if net.max_node_capacity < net.nodes.capacity() {
            net.max_node_capacity = net.nodes.capacity();
        }

        // Update the special nodes
        for i in (0..net.nb_special_nodes){
            net.nodes[i] = self.adjust_n(&net.nodes[i]);
        }

        // Counter
        let mut nb_nodes = net.nb_special_nodes;

        // Update the other nodes
        for i in (net.nb_special_nodes..net.nodes.len()){
            let n = &net.nodes[i];
            if n.1 != [Net::<MyGC>::NULL;3] {
                let ni = self.adjust_i(i);
                let nn = self.adjust_n(n);
                net.nodes[ni]=nn;
                nb_nodes+=1;
            }
        }

        net.nodes.truncate(nb_nodes);
        net.nodes.shrink_to_fit();
        net.available_indexes.clear();
    }

}
