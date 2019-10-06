
use super::super::net::*;
use super::super::gc::GC;
use super::Compactor;

use std::collections::HashMap;

/// Compactor working with a hastable.
/// Simply creates a mapping index->offseted index
pub struct Mapped(pub HashMap<usize,usize>);

impl Compactor for Mapped {

    /// Create a new interval compactor
    fn new()->Self {
       Mapped(HashMap::<usize,usize>::new())
    }

    /// Initialisation: create the compactor's hashtable.
    /// Modify `net.available_indexes'!
    fn init<MyGC:GC>(&mut self, net:&mut Net<MyGC>){
        // Reverse sort: smallest indexes at the end, so we can pop
        net.available_indexes.sort_unstable_by(|a, b| b.cmp(a));

        let mut offset = 0;
        let mut head = net.available_indexes.pop();

        for i in (0..net.nodes.len()){
            match head {
                None => { self.0.insert(i, i-offset); }
                Some(to_skip) => {
                    if i == to_skip {
                        offset +=1;
                        head = net.available_indexes.pop();
                    }
                    else {
                        self.0.insert(i, i-offset);
                    }
                }
            }
        }
    }


    /// Adjust an index.
    /// The index must be "valid"  (not in "available_indexes")
    #[inline]
    fn adjust_i(&mut self, index:usize) -> usize {
        match self.0.get(&index) {
            None => panic!("Try to adjust an invalid index: {}", index),
            Some(&i) => i
        }
    }

    /// Adjust a vertex.
    /// The vertex must be "valid" (it's index not in "available_indexes")
    #[inline]
    fn adjust_v(&mut self, v:Vertex) -> Vertex {
        let (index, port) = v.as_tuple();
        match self.0.get(&index) {
            None => panic!("Try to a vertex with an invalid index: {:?}", v),
            Some(&i) => Vertex::new(i, port)
        }
    }

}
