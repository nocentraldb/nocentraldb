//! What the app writes to stdout. A reader may close stdout before the app
//! is done, as `tasks … invitation | head -1` does after the first line:
//! the app then ends at once, quietly and with status 0, as command-line
//! tools do, instead of panicking as `println!` does on a broken pipe
//! (NOC-340).

use std::io::{ErrorKind, Write};

/// Writes `args` to stdout. Ends the app quietly when nothing reads it any
/// more, and with status 1 and a message when stdout fails otherwise.
pub fn write(args: std::fmt::Arguments<'_>) {
    let mut stdout = std::io::stdout().lock();
    let written = stdout.write_fmt(args).and_then(|()| stdout.flush());
    match written {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::BrokenPipe => std::process::exit(0),
        Err(error) => {
            eprintln!("tasks: cannot write the output: {error}");
            std::process::exit(1)
        }
    }
}

/// `print!` that ends the app quietly on a broken pipe.
macro_rules! out {
    ($($arg:tt)*) => {
        $crate::output::write(format_args!($($arg)*))
    };
}

/// `println!` that ends the app quietly on a broken pipe.
macro_rules! outln {
    () => {
        $crate::output::write(format_args!("\n"))
    };
    ($($arg:tt)*) => {
        $crate::output::write(format_args!("{}\n", format_args!($($arg)*)))
    };
}

pub(crate) use {out, outln};
