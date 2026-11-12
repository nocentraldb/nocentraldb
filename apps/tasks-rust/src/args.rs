//! The command line of `tasks`: global options, then one command.

use std::path::PathBuf;
use std::time::Duration;

pub const USAGE: &str = "\
usage: tasks [--data <dir>] [--key-file <path>] [--relay <url>] <command>

commands:
  invitation                     print this device's invitation and its account's fingerprint
  create                         create a space for tasks, and print its id
  spaces                         list the spaces of this device, with its role in each
  invite <space> <invitation> [--fingerprint <digits>]
                                 add the device of <invitation> to <space> as a writer
  add <space> <title>            add an open task to <space>, and print its id
  done <space> <task>            mark a task as done
  list                           list every task of every space: open ones first
  watch [--until <title>] [--timeout <seconds>]
                                 print the list whenever it changes; with --until, stop
                                 once a task with that title is listed
  status <space>                 the space's status: role, sync, what was parked or refused
  sync [--space <space>] [--timeout <seconds>]
                                 wait until every change of this device is at the relay,
                                 and, with --space, until that space has arrived

--data names the directory the database lives in, ./tasks-data by default.
--key-file keeps the device's keys in that file instead of the platform
keystore, for a machine without one. --relay syncs with a relay, such as
ws://127.0.0.1:7700; without it the database stays on this device.
";

/// How long a command that waits gives up after, unless --timeout says.
pub const WAIT: Duration = Duration::from_secs(30);

/// What the command line asks for.
pub struct Args {
    pub data: PathBuf,
    pub key_file: Option<PathBuf>,
    pub relay: Option<String>,
    pub command: Command,
}

/// One command.
pub enum Command {
    Help,
    Invitation,
    Create,
    Spaces,
    Invite {
        space: String,
        invitation: String,
        fingerprint: Option<String>,
    },
    Add {
        space: String,
        title: String,
    },
    Done {
        space: String,
        task: String,
    },
    List,
    Watch {
        until: Option<String>,
        timeout: Option<Duration>,
    },
    Status {
        space: String,
    },
    Sync {
        space: Option<String>,
        timeout: Duration,
    },
}

/// Reads the arguments after the program's name.
pub fn parse(args: &[String]) -> Result<Args, String> {
    let mut data = PathBuf::from("tasks-data");
    let mut key_file = None;
    let mut relay = None;
    let mut rest = args;
    loop {
        match rest {
            [flag, value, tail @ ..] if flag == "--data" => {
                data = PathBuf::from(value);
                rest = tail;
            }
            [flag, value, tail @ ..] if flag == "--key-file" => {
                key_file = Some(PathBuf::from(value));
                rest = tail;
            }
            [flag, value, tail @ ..] if flag == "--relay" => {
                relay = Some(value.clone());
                rest = tail;
            }
            [flag] if matches!(flag.as_str(), "--data" | "--key-file" | "--relay") => {
                return Err(format!("{flag} needs a value"));
            }
            _ => break,
        }
    }
    let command = command(rest)?;
    Ok(Args {
        data,
        key_file,
        relay,
        command,
    })
}

fn command(words: &[String]) -> Result<Command, String> {
    let words: Vec<&str> = words.iter().map(String::as_str).collect();
    let (name, rest) = match words.split_first() {
        Some((name, rest)) => (*name, rest),
        None => return Err("no command".into()),
    };
    let (positional, options) = split(rest)?;
    let option = |name: &str| options.iter().find(|(o, _)| *o == name).map(|(_, v)| *v);
    let known = |allowed: &[&str]| -> Result<(), String> {
        match options.iter().find(|(o, _)| !allowed.contains(o)) {
            Some((o, _)) => Err(format!("{name} takes no option {o}")),
            None => Ok(()),
        }
    };
    let seconds = |name: &str| -> Result<Option<Duration>, String> {
        option(name)
            .map(|v| {
                v.parse::<u64>()
                    .map(Duration::from_secs)
                    .map_err(|_| format!("{name} takes a number of seconds"))
            })
            .transpose()
    };
    let owned = |s: &str| s.to_owned();
    let command = match (name, positional.as_slice()) {
        ("help" | "--help" | "-h", []) => Command::Help,
        ("invitation", []) => Command::Invitation,
        ("create", []) => Command::Create,
        ("spaces", []) => Command::Spaces,
        ("invite", [space, invitation]) => {
            known(&["--fingerprint"])?;
            Command::Invite {
                space: owned(space),
                invitation: owned(invitation),
                fingerprint: option("--fingerprint").map(owned),
            }
        }
        ("add", [space, title]) => Command::Add {
            space: owned(space),
            title: owned(title),
        },
        ("done", [space, task]) => Command::Done {
            space: owned(space),
            task: owned(task),
        },
        ("list", []) => Command::List,
        ("watch", []) => {
            known(&["--until", "--timeout"])?;
            Command::Watch {
                until: option("--until").map(owned),
                timeout: seconds("--timeout")?,
            }
        }
        ("status", [space]) => Command::Status {
            space: owned(space),
        },
        ("sync", []) => {
            known(&["--space", "--timeout"])?;
            Command::Sync {
                space: option("--space").map(owned),
                timeout: seconds("--timeout")?.unwrap_or(WAIT),
            }
        }
        _ => {
            return Err(format!(
                "unknown command or wrong arguments: {}",
                words.join(" ")
            ));
        }
    };
    if !matches!(
        command,
        Command::Invite { .. } | Command::Watch { .. } | Command::Sync { .. }
    ) {
        known(&[])?;
    }
    Ok(command)
}

/// The positional words and the `--option value` pairs of a command.
type Split<'a> = (Vec<&'a str>, Vec<(&'a str, &'a str)>);

fn split<'a>(words: &[&'a str]) -> Result<Split<'a>, String> {
    let (mut positional, mut options) = (Vec::new(), Vec::new());
    let mut words = words.iter();
    while let Some(word) = words.next() {
        if word.starts_with("--") {
            let value = words
                .next()
                .ok_or_else(|| format!("{word} needs a value"))?;
            options.push((*word, *value));
        } else {
            positional.push(*word);
        }
    }
    Ok((positional, options))
}
