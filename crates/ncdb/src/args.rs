//! The command line's arguments.

use std::path::PathBuf;

pub const USAGE: &str = "\
usage: ncdb model build [--model <path>] [--lock <path>]
       ncdb model check [--model <path>] [--lock <path>]

build reads the schema module (--model, default ./model.ts, which exports
`model`) and the previous lock (--lock, default ./model.lock.json), refuses a
change that clients on the previous model could not keep, and writes the lock.
check does the same and writes nothing: it fails when the lock is missing or
out of date, or the change is refused. Both run the schema with Node.js 24 or
later, found on PATH.
";

/// The paths `ncdb model` works on, as given.
pub struct Paths {
    pub model: PathBuf,
    pub lock: PathBuf,
}

pub enum Command {
    Help,
    Model { check: bool, paths: Paths },
}

pub fn parse(args: &[String]) -> Result<Command, String> {
    let words: Vec<&str> = args.iter().map(String::as_str).collect();
    let (check, rest) = match words.as_slice() {
        ["--help" | "-h" | "help"] => return Ok(Command::Help),
        ["model", "build", rest @ ..] => (false, rest),
        ["model", "check", rest @ ..] => (true, rest),
        [] => return Err("no command".into()),
        ["model", other, ..] => return Err(format!("unknown command: model {other}")),
        ["model"] => return Err("model needs a command: build or check".into()),
        [other, ..] => return Err(format!("unknown command: {other}")),
    };
    let mut paths = Paths {
        model: PathBuf::from("model.ts"),
        lock: PathBuf::from("model.lock.json"),
    };
    let mut rest = rest.iter();
    while let Some(option) = rest.next() {
        let target = match *option {
            "--model" => &mut paths.model,
            "--lock" => &mut paths.lock,
            other => return Err(format!("unknown option: {other}")),
        };
        match rest.next() {
            Some(value) if !value.starts_with("--") => *target = PathBuf::from(value),
            _ => return Err(format!("{option} needs a path")),
        }
    }
    Ok(Command::Model { check, paths })
}
