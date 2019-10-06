use std::fmt;

/// A lambda term.
#[derive(Clone,Debug)]
pub enum Term {
    // Symbol
    Sym {vname:String},
    // Abstraction
    Lambda {vname:String, body:Box<Term>},
    // Application
    App {fun:Box<Term>, arg:Box<Term>},
}


impl fmt::Display for Term {
    fn fmt(&self, f:&mut fmt::Formatter)->fmt::Result{
        use self::Term::*;
        match self {
            Sym{vname}          => write!(f, "{}", vname),
            Lambda{vname, body} => write!(f, "({}->{})", vname, *body),
            App{fun, arg}       => write!(f, "({} {})", *fun, *arg)
        }
    }
}





/// A top-level item is called a "sentence"
#[derive(Clone,Debug)]
pub enum Sentence {
    // Definition
    Let(SLet),
    // Execution
    Run(SRun),
    // Import
    Read(SRead)
}


impl fmt::Display for Sentence {
    fn fmt(&self, f:&mut fmt::Formatter)->fmt::Result{
        use self::Sentence::*;
        match self {
            Let(s) => s.fmt(f),
            Run(s) => s.fmt(f),
            Read(s) => s.fmt(f)
        }
    }
}



/// A top-level "sentence" definition
#[derive(Clone,Debug)]
pub struct SLet {
    pub vname:String,
    pub body:Box<Term>
}

impl fmt::Display for SLet {
    fn fmt(&self, f:&mut fmt::Formatter)->fmt::Result{
        write!(f, "{} = {}.", self.vname, self.body)
    }
}




/// A top-level "sentence" term, to be reduced.
#[derive(Clone,Debug)]
pub struct SRun {
    pub term:Box<Term>
}


impl fmt::Display for SRun {
    fn fmt(&self, f:&mut fmt::Formatter)->fmt::Result{
        write!(f, "{}.", self.term)
    }
}


/// A top level "read" sentence.
#[derive(Clone,Debug)]
pub struct SRead {
    pub path:String,
    pub name:Option<String>
}

impl fmt::Display for SRead {
    fn fmt(&self, f:&mut fmt::Formatter)->fmt::Result{
        match &self.name {
            None => write!(f, "read {}", self.path),
            Some(n) => write!(f, "read {} as {}", self.path, n)
        }
    }
}


/// Processing function used by the parser
pub fn read_process(mut rpath: String)->String{
    // Remove the 'read' part
    rpath.drain(0..4);
    // Remove the whitespaces
    rpath.trim().to_string()
}





// 
// 
// #[allow(non_camel_case_types)]
// #[derive(Clone,Debug)]
// pub enum Tok<'input> {
//     // Decorated
//     Identifier(&'input str),
//     // Keywords
//     KWlet, KWrun,
//     // Other
//     lpar, rpar,
//     comma, dot,
//     exclam, arrow, equal,
// }
// 
// 
// impl<'input> fmt::Display for Tok<'input> {
//     fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
//         use self::Tok::*;
//         let s = match *self {
//             Identifier(_) => "Identifier",
//             KWlet => "KWlet",
//             KWrun => "KWrun,",
//             // Other
//             lpar => "lpar",
//             rpar => "rpar",
//             comma => "comma",
//             dot => "dot",
//             exclam => "exclam",
//             arrow => "arrow",
//             equal => "equal",
//         };
//         s.fmt(f)
//     }
// }
// 
// 
// pub enum LexicalError {
//     /* */
// }
// 
// 
// use std::str::CharIndices;
// 
// pub struct Lexer<'input> {
//     chars: CharIndices<'input>,
// }
// 
// impl<'input> Lexer<'input> {
//     pub fn new(input: &'input str) -> Self {
//         Lexer { chars: input.char_indices() }
//     }
// }
// 
// 
// 
// 
