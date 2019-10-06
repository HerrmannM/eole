
use super::super::net::*;
use super::super::gc::GC;
use super::Compactor;

/// Compactor working with interval.
/// Contains a table create from the `net.available_indexes'
/// ```
///     ( > x, offset_x)
///     ( > y, offset_y)
///     ...
///     ( > z, offset_z)
/// ```
/// With x > y > z, and offset_x > offset_y > offset_z
/// The first column represents a cutoff: index above this value must be offseted by -offset
/// With net.available_indexes = [0, 1, 2, 4, 5], we have the:
/// ```
///     ( >5, 5)
///     ( >2, 3)
/// ```
/// So, the index 3 is mapped to 0 and the index 6 is mapped to 1.
pub struct Interval(pub Vec<(usize,usize)>);

impl Compactor for Interval {

    /// Create a new interval compactor
    fn new()->Self {
       Interval(Vec::<(usize,usize)>::new())
    }

    /// Initialisation: create the compactor's table.
    /// Modify `net.available_indexes'!
    fn init<MyGC:GC>(&mut self, net:&mut Net<MyGC>){

        self.0.reserve(net.available_indexes.len()/2);

        // Sort
        net.available_indexes.sort_unstable();

        match net.available_indexes.split_first() {
            None => (),
            Some((smallest, tail)) => {
                let mut offset = 1;
                let mut previous:usize = *smallest;

                for index in tail {
                    // If not contiguous: add an entry with "previous"
                    if *index != previous+1 {
                        // Not contiguous:
                        self.0.push((previous, offset));
                    }
                    offset+=1;
                    previous = *index;
                }

                // Finish the table
                self.0.push((previous, offset));
                self.0.reverse();
            }
        }
    }


    /// Adjust an index.
    /// The index must be "valid"  (not in "available_indexes")
    #[inline]
    fn adjust_i(&mut self, index:usize) -> usize {
        match self.0.iter().find(|&&record| index > (record.0)){
            None => index,
            Some(r) => index-(r.1)
        }
    }

    /// Adjust a vertex.
    /// The vertex must be "valid" (it's index not in "available_indexes")
    #[inline]
    fn adjust_v(&mut self, v:Vertex) -> Vertex {
        let (index, port) = v.as_tuple();
        match self.0.iter().find(|&&record| index > (record.0)){
            None => v,
            Some(r) => Vertex::new(index-(r.1), port)
        }
    }

}
