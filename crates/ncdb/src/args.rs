//! The command line's arguments.

use std::path::PathBuf;

use crate::texts;

pub(crate) use crate::texts::USAGE;

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
        [] => return Err(texts::NO_COMMAND.into()),
        ["model", other, ..] => return Err(texts::unknown_model_command(other)),
        ["model"] => return Err(texts::MODEL_NEEDS_COMMAND.into()),
        [other, ..] => return Err(texts::unknown_command(other)),
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
            other => return Err(texts::unknown_option(other)),
        };
        match rest.next() {
            Some(value) if !value.starts_with("--") => *target = PathBuf::from(value),
            _ => return Err(texts::option_needs_path(option)),
        }
    }
    Ok(Command::Model { check, paths })
}
