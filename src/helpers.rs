//! # Helpers
//! General helpers that do not fit in any other specific files.


/// Print on stderr when in debug mode.
/// Works like `eprint!'
/// ```no_run
/// let a = 5;
/// dprint("value of a is {}\n", a);
/// ```
#[macro_export]
macro_rules! dprint {
    ($($arg:tt)*) => {
        if cfg!(debug_assertions) { eprint!($($arg)*); }
    };
}

/// Print on stderr when in debug mode.
/// Works like `eprintln!'
/// ```no_run
/// let a = 5;
/// dprintln("value of a is {}\n", a);
/// ```
#[macro_export]
macro_rules! dprintln {
    ($($arg:tt)*) => {
        if cfg!(debug_assertions) { eprintln!($($arg)*); }
    };
}
