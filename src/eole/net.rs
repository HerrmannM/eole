//! # Eole Network
//! Contains the basic structures and logic of the eole networks.
//!
//! ## Fan Status
//! Label are used to decide if a pair of fan is matching or not.
//! New labels are generated "on demand" but they are never "collected".
//! This is not the cleaneast way to go, but certainly one of the most efficient.
//!
//! A Fan out only contains a label.
//! On the other hand, a fan in can be either "labeled" (i.e. paired with some fan out)
//! or "stem" (i.e. ready to start a new sharing).
//!
//! ## Nodes
//! Nodes are splitted into two kinds: constructors and destructors.
//! Note that we can only have destructors><constructors interactions.


#[macro_use]
use crate::helpers::*;

use crate::eole::gc::GC;

use std::mem;
use std::fmt::Write;
use std::collections::HashMap;

/// Type of label.
/// Will be upgraded to 128bits if someone ever reach the 64bits limit....
pub type Label = u64;

/// Status of a fan in.
#[derive(Copy,Clone,Debug,PartialEq,Eq)]
pub enum FIStatus {
    /// `Labeled' fan in: part of a sharing
    Labeled(Label),
    /// `Stem' fan in: can create a new sharing
    Stem
}

impl FIStatus {
    /// Test if fan in status is matching a label.
    /// The label should be coming from a fan out.
    #[inline]
    pub fn is_matching (&self, fan_out_label:Label) -> bool {
        match self {
            Self::Labeled(l) => *l == fan_out_label,
            Self::Stem => false
        }
    }
}


/// Constructors Kind
#[derive(Clone, Debug)]
pub enum CstrK {
    /// Abstraction (lambda): records the name of the variable and if the abs is bound or not.
    Abs(String, bool),

    /// Fan Out: contains the label
    FanOut(Label)
}

/// Destructors Kind
#[derive(Copy, Clone, Debug)]
pub enum DstrK {
    /// Application
    Apply,

    /// Fan In: either a stem node or a part of a share
    FanIn(FIStatus)
}


/// Kind of node: either a constructor or a destructor.
#[derive(Clone, Debug)]
pub enum NodeKind {
    CstrK(CstrK),
    DstrK(DstrK)
}



#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Port(pub u8);

impl Port {
    pub const MAIN:Port = Port(0);
    pub const AUX1:Port = Port(1);
    pub const AUX2:Port = Port(2);
}

impl From<u8> for Port {
    fn from(p:u8)->Self {
        Port(p)
    }
}

impl From<Port> for u8 {
    fn from(p:Port)->Self{
        p.0
    }
}

/// A Vertex is made of a node index (in an array) and a port number.
/// We have a main port (0), and two auxiliary ports (1 & 2).
/// Note: if we extend Éole with sum and product types, we will need multiple auxiliary ports.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Vertex(usize, Port);

impl Vertex {

    /// Create a new vertex
    #[inline]
    pub const fn new(index:usize, port:Port) -> Vertex {
        Vertex(index, port)
    }

    /// Create a new vertex with the main port
    #[inline]
    pub const fn main(index:usize) -> Vertex {
        Vertex(index, Port::MAIN)
    }

    /// Get the index of a vertex
    #[inline]
    pub const fn get_index(&self) -> usize {
        self.0
    }

    /// Get the port of a vertex
    #[inline]
    pub const fn get_port(&self) -> Port {
        self.1
    }

    /// Deconstruct a vertex into a tuple (Index, Port)
    #[inline]
    pub const fn as_tuple(&self) -> (usize, Port) {
        (self.get_index(), self.get_port())
    }

}

/// Create a vertex from an index and a port number
#[inline]
pub const fn mkv(index:usize, port:u8) -> Vertex {
    Vertex::new(index, Port(port))
}

/// Create a main vertex from an index
#[inline]
pub const fn main(index:usize) -> Vertex {
    Vertex::main(index)
}


/// A Node is actually made of 3 sub-nodes: vertices.
#[derive(Clone, Debug)]
pub struct Node(pub NodeKind, pub [Vertex; 3]);





use FIStatus::*;
use self::CstrK::*;
use self::DstrK::*;
use NodeKind::*;






/// The network
#[derive(Clone, Debug)]
pub struct Net<MyGC:GC> {

    // --- --- --- Main fields

    /// Store the next available label.
    pub next_label:Label,

    /// Home of the nodes.
    pub nodes:Vec<Node>,

    /// Indexes of available nodes in `nodes'. Use for recycling.
    pub available_indexes:Vec<usize>,

    // --- --- --- GC
    pub gc:MyGC,

    // --- --- --- Compaction
    pub nb_special_nodes:usize,


    // --- --- --- Statistics

    /// Maximum length of the nodes vec
    pub max_node_len:usize,

    /// Maximum capacity of the nodes vec
    pub max_node_capacity:usize,

    /// Total number of nodes used.
    pub nb_used: u64,

    /// Amount of recycled nodes among `nb_node_used'.
    pub nb_reused: u64,

    /// Number of nodes removed through interactions (annihilations).
    pub nb_remove_inter:u64,



    /// Number of App><Abs interactions.
    pub nb_appabs: u64,

    /// Number of App><Fou interactions.
    pub nb_appfou: u64,

    /// Number of Fin><Abs interactions.
    pub nb_finabs: u64,

    /// Number of Fin><Fou Annihilation interactions.
    pub nb_finfou_a: u64,

    /// Number of Fin><Fou Duplication interactions.
    pub nb_finfou_d: u64
}




impl<MyGC:GC> Net<MyGC> {

    /// Constant Root index: 0.
    pub const ROOT_INDEX:usize = 0;

    /// Constant Root vertex: the "body" port (aux 1) of an abstraction
    pub const ROOT_VERTEX:Vertex = mkv(Self::ROOT_INDEX, 1);

    /// Constant NULL vertex.
    /// Note: The fact that this is coded as the main port of an abstraction is irrelevant.
    ///       We simply give an other use to the index 0.
    pub const NULL:Vertex = mkv(0,0);


    // --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- ---
    // --- --- --- Create a new network

    /// Create a new network initialized with the root node.
    pub fn new()->Net<MyGC>{
        // Initialisation
        let mut res = Net {

            // --- --- ---
            next_label:1,
            nodes:vec![],
            available_indexes:vec![],

            // --- --- ---
            gc:MyGC::new(),

            // --- --- ---
            nb_special_nodes:0,

            // --- --- ---
            max_node_len: 0,
            max_node_capacity: 0,

            nb_used: 0,
            nb_reused: 0,

            nb_remove_inter:0,

            nb_appabs: 0,
            nb_appfou: 0,
            nb_finabs: 0,
            nb_finfou_a: 0,
            nb_finfou_d: 0,
        };
        // Add the ROOT/NULL node
        res.new_abs(String::from("ROOT"), false);   // Index 0
        // Init the GC
        MyGC::init(&mut res);
        // Store the number of "special nodes"
        res.nb_special_nodes = res.nodes.len();

        res
    }



    // --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- ---
    // --- --- --- Print statistics

    pub fn print_stats(&mut self) -> String {
        let fansteps =  self.nb_appfou + self.nb_finabs + self.nb_finfou_a + self.nb_finfou_d;
        let steps = self.nb_appabs + fansteps;
        let total_remove = self.gc.get_nb_collected() + self.nb_remove_inter;

        if self.max_node_len < self.nodes.len() {
            self.max_node_len = self.nodes.len();
        }

        if self.max_node_capacity < self.nodes.capacity() {
            self.max_node_capacity = self.nodes.capacity();
        }

        let created_size_mo = (self.max_node_len*mem::size_of::<Node>())/(1024*1024);
        let created_size_ko = ((self.max_node_len*mem::size_of::<Node>())/1024)-(created_size_mo*1024);

        let max_size_mo = (self.max_node_capacity*mem::size_of::<Node>())/(1024*1024);
        let max_size_ko = ((self.max_node_capacity*mem::size_of::<Node>())/1024)-(max_size_mo*1024);

        let size_mo = (self.nodes.capacity()*mem::size_of::<Node>())/(1024*1024);
        let size_ko = ((self.nodes.capacity()*mem::size_of::<Node>())/1024)-(size_mo*1024);

        let mut res = String::new();

        write!(&mut res, "* * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * *\n");
        write!(&mut res, "Reductions           : {}\n", steps);
        write!(&mut res, "    APP-LAMBDA           : {}\n", self.nb_appabs);
        write!(&mut res, "    FAN                  : {}\n", fansteps);
        write!(&mut res, "        APP-FOU              : {}\n", self.nb_appfou);
        write!(&mut res, "        FIN-LAMBDA           : {}\n", self.nb_finabs);
        write!(&mut res, "        FIN-FOU (dup)        : {}\n", self.nb_finfou_d);
        write!(&mut res, "        FIN-FOU (ann)        : {}\n", self.nb_finfou_a);
        write!(&mut res, "\n");

        write!(&mut res, "* * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * *\n");
        write!(&mut res, "Nodes used           : {}\n", self.nb_used);
        write!(&mut res, "    Max created          : {} ~ {}Mo {}Ko\n", self.max_node_len, created_size_mo, created_size_ko);
        write!(&mut res, "    Max allocation       : {} ~ {}Mo {}Ko\n", self.max_node_capacity, max_size_mo, max_size_ko);
        write!(&mut res, "    End allocation       : {} ~ {}Mo {}Ko\n", self.nodes.capacity(), size_mo, size_ko);
        write!(&mut res, "        Nodes in use         : {}\n", self.nodes.len()-self.available_indexes.len());
        write!(&mut res, "        Nodes available      : {}\n", self.available_indexes.len());
        write!(&mut res, "    Reused               : {}\n", self.nb_reused);
        write!(&mut res, "\n");

        write!(&mut res, "* * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * *\n");
        write!(&mut res, "Nodes removed        : {}\n", total_remove);
        write!(&mut res, "    Interactions         : {}\n", self.nb_remove_inter);
        write!(&mut res, "    GC                   : {}\n", self.gc.get_nb_collected());
        write!(&mut res, "\n");

        write!(&mut res, "* * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * *\n");
        res.push_str(&self.gc.get_stats());
        write!(&mut res, "\n");

        res
    }


    // --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- ---
    // --- --- --- Label functions

    /// Get a new label
    #[inline]
    pub fn new_label(&mut self) -> u64 {
        let res = self.next_label;
        self.next_label+=1;
        res
    }



    // --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- ---
    // --- --- --- Node functions

    /// Get a reference on a node from the network
    #[inline]
    pub fn get_node(&self, index:usize) -> &Node {
        &self.nodes[index]
    }

    /// Create a new node.
    /// Returns the index of the node.
    /// Will attempt to reuse an old node.
    #[inline]
    pub fn new_node(&mut self, kind:NodeKind) -> usize {
        self.nb_used+=1;
        let nn = Node(kind, [Self::NULL;3]);
        /*
                self.nodes.push(nn);
                self.nodes.len()-1
                    */
        let res = match self.available_indexes.pop() {
            Some(idx) => {
                self.nb_reused+=1;
                self.nodes[idx] = nn;
                idx
            }
            None => {
                self.nodes.push(nn);
                self.nodes.len()-1
            }
        };
        res
    }

    /// Create a new abstraction node.
    #[inline]
    pub fn new_abs(&mut self, vname:String, bound:bool) -> usize {
        self.new_node(CstrK(Abs(vname, bound)))
    }

    /// Create a new fan out node.
    #[inline]
    pub fn new_fout(&mut self, label:Label) -> usize {
        self.new_node(CstrK(FanOut(label)))
    }

    /// Create a new apply node.
    #[inline]
    pub fn new_app(&mut self) -> usize {
        self.new_node(DstrK(Apply))
    }

    /// Create a new fan in node.
    #[inline]
    pub fn new_fin(&mut self, fis:FIStatus) -> usize {
        self.new_node(DstrK(FanIn(fis)))
    }

    /// Remove a node by its index
    #[inline]
    pub fn remove(&mut self, index:usize){
        self.nodes[index].1 = [Self::NULL;3];
        self.available_indexes.push(index);
    }



    // --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- ---
    // --- --- --- Edge/Vertex functions

    /// Follow an edge: given the index and port of the source, returns the target.
    /// Low level version. Using `follow' provide extra checking in debug mode.
    #[inline]
    pub fn get_index_port(&self, index:usize, port:usize) -> Vertex {
        self.nodes[index].1[port]
    }

    /// Follow an edge: given the source, returns the target.
    /// Low level version. Using `follow' provide extra checking in debug mode.
    #[inline]
    pub fn get_vertex(&self, src:Vertex) -> Vertex {
        let (index, port) = src.as_tuple();
        self.get_index_port(index, port.0 as usize)
    }

    /// Follow an edge: "should be use" version
    #[inline]
    pub fn follow(&self, src:Vertex) -> Vertex {
        let res = self.get_vertex(src);

        if cfg!(debug_assertions){
            let (index, port) = src.as_tuple();
            assert_ne!(res, Self::NULL, "Accessing a NULL vertex from ({}, {}): {:?}", index, port.0, self.nodes[index]);
            self.search_available_indexes(res.get_index());
        }

        res
    }

    /// Update the value `v' of a vertex with its (`index', `port') pair.
    #[inline]
    pub fn update_index_port(&mut self, index:usize, port:usize, v:Vertex){
        self.nodes[index].1[port] = v;
    }

    /// Update a vertex `src' with `tgt'.
    #[inline]
    pub fn update_vertex(&mut self, src:Vertex, tgt:Vertex){
        let (index, port) = src.as_tuple();
        self.update_index_port(index, port.0 as usize, tgt);
    }

    /// Create a new edge src -> tgt.
    /// In our graph, edges are directed.
    /// However, we maintain a double link, as this allow a node to know who is targeting it.
    #[inline]
    pub fn create_edge_raw(&mut self, src:Vertex, tgt:Vertex) {
        self.update_vertex(src, tgt);
        self.update_vertex(tgt, src);
    }

    /// Create a new edge src -> tgt.
    /// Let the GC checks first the src or the tgt
    #[inline]
    pub fn create_edge(&mut self, src:Vertex, tgt:Vertex) {
        if MyGC::check_edge(self, src, tgt) {
            self.create_edge_raw(src, tgt);
        }
    }



    // --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- ---
    // --- --- --- Helper functions, use to perform extra checking.

    /// Check if `available_indexes' contains a given index, and panic! if so.
    /// Only works in DEBUG mode.
    pub fn search_available_indexes(&self, index:usize){
        if cfg!(debug_assertions){
            let mut iter = self.available_indexes.iter();
            match (iter.find(|&&x| x==index)) {
                None => (),
                Some(a) => assert!(false, "pointing to a deleted node {}", a)
            }
        }
    }

    /// Check if the network contains a port pointing to the provided index, and panic! if so.
    /// Only works in DEBUG mode.
    pub fn search_index(&self, index:usize){
        if cfg!(debug_assertions){
            let mut iter = self.nodes.iter();
            iter.for_each(|n|{
                let p = n.1;
                assert_ne!(p[0].get_index(), index, "{:?} still pointing to {:?}", n, p[0]);
                assert_ne!(p[1].get_index(), index, "{:?} still pointing to {:?}", n, p[1]);
                assert_ne!(p[2].get_index(), index, "{:?} still pointing to {:?}", n, p[2]);
            });
        }
    }



    // --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- --- ---
    // --- --- --- Interactions

    // Stitching
    //   Stitching amounts to update the edges of the network.
    //   Stitching functions provide a higher level view of those updates, depending on the node destiny:
    //   Indeed, if a node is going to be destroyed ("old"), we want to look where that node is connected.
    //   Here is an example:
    //         R
    //         | A1      Stitching src:@A1 -> tgt:λA1
    //         @         Get the vertex targeting @A1   (get reverse @A1): src_src, source of the source
    //        / \ A2     Get the vertex targeted by λA1 (get λA1): tgt_tgt, target of the target
    //   A2--λ           Create the (and the associated reverse edge) edge src_src -> tgt_tgt
    //       | A1        In the end, we have the connection R->B
    //       B
    //
    //  Note: stitching is directed. A source should be the target of something else,
    //  hence "following the source" should gives us the source of the source.

    /// Stitch between 2 nodes that are going to be erased
    #[inline]
    pub fn stitch_old_old(&mut self, src:Vertex, tgt:Vertex){
        let src_src = self.follow(src);
        let tgt_tgt = self.follow(tgt);
        self.create_edge(src_src, tgt_tgt);
    }

    /// Stitch between a old (to be deleted) and a new vertex
    /// The source is old, the target is new
    #[inline]
    fn stitch_old_new(&mut self, src:Vertex, tgt:Vertex){
        let src_src = self.follow(src);
        self.create_edge(src_src, tgt);
    }

    /// Stitch between a new and an old (to be deleted) vertex
    /// The source is new, the target is old
    #[inline]
    fn stitch_new_old(&mut self, src:Vertex, tgt:Vertex){
        let tgt_tgt = self.follow(tgt);
        self.create_edge(src, tgt_tgt);
    }

    /// Stitch between 2 new vertices. Just an alias to insert_edge
    #[inline]
    fn stitch_new_new(&mut self, src:Vertex, tgt:Vertex){
        self.create_edge(src, tgt);
    }

    /// Clean two nodes (Destructor ID, Constructor ID) being in an interaction.
    fn clean_inter(&mut self, did:usize, cid:usize){
        self.remove(did);
        self.remove(cid);
        // Stats
        self.nb_remove_inter += 2;
    }


    /// Interaction
    /// An interaction can only exists between a destructor and a constructor.
    pub fn interact(&mut self, did:usize, dkind: self::DstrK, cid:usize, ckind: self::CstrK){
        dprintln!("DID {:?}:  {:?}", did, self.get_node(did));
        dprintln!("CID {:?}:  {:?}", cid, self.get_node(cid));
        match dkind {

            // --- Apply
            Apply => {
                match ckind {

                    // Apply >< Abstraction: annihilation
                    //      Stitch Apply/A1 ("up") on Abs/A1 ("body")
                    //      If not bound: erase the target of Apply/A2 (new entry point for the GC)
                    //      Else, stitch Abs/A2 ("binder") -> Apply/A2 ("arg")
                    Abs(_vname, bound) => {
                        dprintln!("Apply >< Abs {} {}", _vname, bound);


                        self.stitch_old_old(mkv(did, 1), mkv(cid, 1));
                        if bound { self.stitch_old_old(mkv(cid, 2), mkv(did, 2)); }
                        else { MyGC::to_collect(self, self.follow(mkv(did, 2))); }
                        // Stats & Cleaning
                        self.nb_appabs+=1;
                        self.clean_inter(did, cid);
                    }

                    // Apply >< Fan Out: duplication
                    // Warning: the fan node targeting App/A2 is a new fan in, not a fan out!
                    FanOut(label) => {
                        dprintln!("Apply >< Fan Out {}", label);
                        // Alias: makes thing easier...
                        let (oldapp, oldfan) = (did, cid);
                        // Create the new nodes                 // NEW         OLD
                        let app1 = self.new_app();              // App1/M   -> oldfan/A1
                        let app2 = self.new_app();              // App2/M   -> oldfan/A2
                        let fou1 = self.new_fout(label);        // FOut1/M  <- oldapp/A1
                        let fin2 = self.new_fin(Labeled(label));// FIn2/M   -> oldapp/A2
                        // "External" stitching
                        // Old -> New
                        self.stitch_old_new(mkv(oldapp, 1), main(fou1));
                        // New -> Old
                        self.stitch_new_old(main(app1), mkv(oldfan, 1));
                        self.stitch_new_old(main(app2), mkv(oldfan, 2));
                        self.stitch_new_old(main(fin2), mkv(oldapp, 2));
                        // "Internal" stitching
                        self.stitch_new_new(mkv(fou1, 1), mkv(app1, 1));
                        self.stitch_new_new(mkv(fou1, 2), mkv(app2, 1));
                        self.stitch_new_new(mkv(app1, 2), mkv(fin2, 1));
                        self.stitch_new_new(mkv(app2, 2), mkv(fin2, 2));
                        // Stats & Cleaning
                        self.nb_appfou+=1;
                        self.clean_inter(did, cid);
                    }
                }
            } // End of Apply

            // --- Fan In
            FanIn(in_status) => {
                match ckind {

                    // Fan In >< Abstraction:
                    //   This is where a Stem fan in starts a sharing!
                    //   Warning: * the fan node linking oldabs/A2 is a new fan out, not a fan in!
                    //            * only needed if bound=true
                    //   Do not generate a label if the fanin is paired
                    Abs(vname, bound) => {
                        dprintln!("Fan In  >< Abs {} {}", vname, bound);
                        // Alias: makes thing easier...
                        let (oldfan, oldabs) = (did, cid);
                        // Create a label if stem, else continue the sharing
                        let label = match in_status {
                            Labeled(l) => l,
                            Stem => self.new_label()
                        };
                        // Create the new nodes
                        let fin  = self.new_fin(Labeled(label));
                        let abs1 = self.new_abs(vname.clone(), bound);
                        let abs2 = self.new_abs(vname.clone(), bound);
                        // Stitiching without the fan out (done later)
                        // "External" stitching
                        // Old -> New
                        self.stitch_old_new(mkv(oldfan, 1), main(abs1));
                        self.stitch_old_new(mkv(oldfan, 2), main(abs2));
                        // New -> Old
                        self.stitch_new_old(main(fin), mkv(oldabs, 1));
                        // "Internal" stitching
                        self.stitch_new_new(mkv(abs1, 1), mkv(fin, 1));
                        self.stitch_new_new(mkv(abs2, 1), mkv(fin, 2));
                        // Do we need a fan out? Unbounded: no, else yes.
                        if bound {
                            let fout = self.new_fout(label);
                            // "External" stitching: Old -> New
                            self.stitch_old_new(mkv(oldabs, 2), main(fout));
                            // "Internal" stitching: New->New
                            self.stitch_new_new(mkv(fout, 1), mkv(abs1, 2));
                            self.stitch_new_new(mkv(fout, 2), mkv(abs2, 2));
                        }
                        // Stats & Cleaning
                        self.nb_finabs+=1;
                        self.clean_inter(did, cid);
                    }

                    // Fan In >< Fan Out: Annihilation or duplication
                    FanOut(outlabel) => {
                        let (oldfin, oldfou) = (did, cid);
                        if in_status.is_matching(outlabel) {
                            dprintln!("Fan In > MATCH < Fan Out {}", outlabel);
                            // Matching labels: annihilation.  Aux1 on Aux1, Aux2 on Aux2
                            self.stitch_old_old(mkv(oldfin, 1), mkv(oldfou, 1));
                            self.stitch_old_old(mkv(oldfin, 2), mkv(oldfou, 2));
                            // Stats & Cleaning
                            self.nb_finfou_a+=1;
                            self.clean_inter(did, cid);
                        } else {
                            dprintln!("Fan In > DUP < Fan Out {}", outlabel);
                            // Non matching labels: duplication. Create a new FanIn and FanOut
                            let fin1 = self.new_fin(in_status);
                            let fin2 = self.new_fin(in_status);
                            let fou1 = self.new_fout(outlabel);
                            let fou2 = self.new_fout(outlabel);
                            // "External" stitching
                            // Old->New
                            self.stitch_old_new(mkv(oldfin, 1), main(fou1));
                            self.stitch_old_new(mkv(oldfin, 2), main(fou2));
                            // New->Old
                            self.stitch_new_old(main(fin1), mkv(oldfou, 1));
                            self.stitch_new_old(main(fin2), mkv(oldfou, 2));
                            // "Internal" stitching
                            self.stitch_new_new(mkv(fou1, 1), mkv(fin1, 1));
                            self.stitch_new_new(mkv(fou2, 1), mkv(fin1, 2));
                            self.stitch_new_new(mkv(fou1, 2), mkv(fin2, 1));
                            self.stitch_new_new(mkv(fou2, 2), mkv(fin2, 2));
                            // Stats & Cleaning
                            self.nb_finfou_d+=1;
                            self.clean_inter(did, cid);
                        }
                    } // End of FanOut

                } // End inter match ckind
            } // End of Fan In

        } // End of outer match dkind

        // DEBUG
        if cfg!(debug_assertions){
            eprintln!("Search CID {}    DID {}", cid, did);
            eprintln!("Search CID {}", cid);
            self.search_index(cid);
            eprintln!("Search DID {}", did);
            self.search_index(did);
        }
    }  // End of fn interact

}
