//! A Rust device for the end-to-end test with a browser (NOC-178,
//! `scripts/sdk-browser-rust/`): a database of the Rust SDK, in memory, of
//! its own account, syncing with the relay at a URL, that the test drives
//! one command per line on stdin. Each answer is one line on stdout:
//! `ok …` or `err …`. Not part of the SDK: only the test builds and runs it.
//!
//! ```text
//! browser_peer <model.lock.json> <relay url>
//! ```
//!
//! On start it pauses syncing and writes `invitation <text>`, so that the
//! test can start the relay allowing this account, then `resume`s it.
//!
//! - `resume`: resumes syncing; `ok` once connected.
//! - `create`: a space with one task; `ok <space> <task>`.
//! - `add <space> <invitation> <fingerprint>`: adds the invitation's member
//!   as a writer. The account is new to this database, so the first add is
//!   refused with `UnknownAccount` and its fingerprint, which must be the one
//!   the test passes, as a person compares it; then it adds with it. `ok
//!   <notice code>`.
//! - `synced`: `ok` once connected with no change unconfirmed.
//! - `holds <space> <task>`: `ok` once the space holds the task.
//! - `update <space> <task>`: this device's concurrent write; `ok`.
//! - `read <space> <task>...`: the tasks as JSON; `ok <json>`.

use std::io::{BufRead, Write};
use std::str::FromStr;
use std::time::{Duration, Instant};

use nocentraldb::{
    AddMember, Connection, Db, ErrorCode, Invitation, ObjectId, OpenOptions, Role, Space, SpaceId,
    Value,
};

/// How long a command waits for syncing.
const WAIT: Duration = Duration::from_secs(30);

/// The output of an SDK future, which is ready at its first poll: every
/// call of the SDK finishes on the device; syncing runs on its own thread.
fn now<F: std::future::Future>(future: F) -> F::Output {
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    match std::pin::pin!(future).poll(&mut cx) {
        std::task::Poll::Ready(output) => output,
        std::task::Poll::Pending => panic!("an SDK call waited"),
    }
}

/// Waits until `done` holds, or fails after [`WAIT`].
fn eventually(what: &str, mut done: impl FnMut() -> bool) -> Result<(), String> {
    let start = Instant::now();
    while !done() {
        if start.elapsed() > WAIT {
            return Err(format!("{what}: not in time"));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    Ok(())
}

fn space(db: &Db, id: &str) -> Result<Space, String> {
    let id = SpaceId::from_str(id).map_err(|e| format!("{e}"))?;
    now(db.spaces().get(id)).map_err(|e| format!("{e}"))
}

fn task(id: &str) -> Result<ObjectId, String> {
    ObjectId::from_str(id).map_err(|e| format!("{e}"))
}

/// A JSON string of plain text.
fn json(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// A task as the test compares it with the browser's read.
fn read(project: &Space, id: ObjectId) -> Result<String, String> {
    let Some(object) = now(project.get("tasks", id)).map_err(|e| format!("{e}"))? else {
        return Ok("null".into());
    };
    let text = |name| -> Result<String, String> {
        Ok(
            match object.get::<&str>(name).map_err(|e| format!("{e}"))? {
                Some(text) => json(text),
                None => "null".into(),
            },
        )
    };
    let priority = match object.get::<i64>("priority").map_err(|e| format!("{e}"))? {
        Some(n) => n.to_string(),
        None => "null".into(),
    };
    let mut tags: Vec<String> = Vec::new();
    if let Some(elements) = object.get::<&[Value]>("tags").map_err(|e| format!("{e}"))? {
        for element in elements {
            let tag: &str = element.try_into().map_err(|e| format!("{e}"))?;
            tags.push(tag.to_owned());
        }
    }
    tags.sort();
    let tags: Vec<String> = tags.iter().map(|t| json(t)).collect();
    Ok(format!(
        "{{\"id\":{},\"deleted\":{},\"title\":{},\"status\":{},\"priority\":{},\"tags\":[{}]}}",
        json(&object.id().to_string()),
        object.deleted(),
        text("title")?,
        text("status")?,
        priority,
        tags.join(",")
    ))
}

fn command(db: &Db, line: &str) -> Result<String, String> {
    let words: Vec<&str> = line.split_whitespace().collect();
    match words.as_slice() {
        ["resume"] => {
            now(db.sync().resume());
            eventually("connected", || {
                db.sync().current().connection == Connection::Connected
            })?;
            Ok(String::new())
        }
        ["create"] => {
            let project = now(db.spaces().create()).map_err(|e| format!("{e}"))?;
            let written = now(project.write(|tx| {
                tx.create(
                    "tasks",
                    [
                        ("title", Value::from("from Rust")),
                        ("status", Value::from("open")),
                        ("priority", Value::from(1)),
                        ("tags", Value::add(["rust"])),
                    ],
                )?;
                Ok(())
            }))
            .map_err(|e| format!("{e}"))?;
            Ok(format!("{} {}", project.id(), written.ids[0]))
        }
        ["add", project, invitation, fingerprint @ ..] => {
            let project = space(db, project)?;
            let invitation = Invitation::from_str(invitation).map_err(|e| format!("{e}"))?;
            let unconfirmed = AddMember {
                role: Role::Writer,
                fingerprint: None,
            };
            let refused = match now(project.members().add(&invitation, unconfirmed)) {
                Ok(_) => return Err("added without a confirmed fingerprint".into()),
                Err(error) if error.code() == ErrorCode::UnknownAccount => error,
                Err(error) => return Err(format!("{error}")),
            };
            let shown = *refused
                .fingerprint()
                .ok_or("UnknownAccount without a fingerprint")?;
            // A person compares the fingerprint the browser shows.
            if shown.to_string() != fingerprint.join(" ") {
                return Err(format!(
                    "the fingerprints differ: {shown} here, {} in the browser",
                    fingerprint.join(" ")
                ));
            }
            let confirmed = AddMember {
                role: Role::Writer,
                fingerprint: Some(shown),
            };
            let notice =
                now(project.members().add(&invitation, confirmed)).map_err(|e| format!("{e}"))?;
            Ok(format!("{:?}", notice.code))
        }
        ["synced"] => {
            eventually("synced", || {
                let status = db.sync().current();
                status.connection == Connection::Connected && status.unconfirmed == 0
            })?;
            Ok(String::new())
        }
        ["holds", project, id] => {
            let id = task(id)?;
            let mut held = None;
            eventually("the space and the task are held", || {
                held = space(db, project)
                    .ok()
                    .filter(|p| matches!(now(p.get("tasks", id)), Ok(Some(_))));
                held.is_some()
            })?;
            Ok(String::new())
        }
        ["update", project, id] => {
            let project = space(db, project)?;
            let id = task(id)?;
            now(project.write(|tx| {
                tx.update(
                    "tasks",
                    id,
                    [
                        ("title", Value::from("Rust's title")),
                        ("tags", Value::add(["x"])),
                    ],
                )?;
                Ok(())
            }))
            .map_err(|e| format!("{e}"))?;
            Ok(String::new())
        }
        ["read", project, ids @ ..] => {
            let project = space(db, project)?;
            let mut read_ = Vec::new();
            for id in ids {
                read_.push(read(&project, task(id)?)?);
            }
            Ok(format!("[{}]", read_.join(",")))
        }
        _ => Err(format!("unknown command: {line}")),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [lock, relay] = args.as_slice() else {
        eprintln!("usage: browser_peer <model.lock.json> <relay url>");
        std::process::exit(2);
    };
    let lock = std::fs::read_to_string(lock).expect("the lock reads");
    let db = now(Db::open(OpenOptions::new(&lock).in_memory().relay(relay)))
        .expect("the database opens");
    now(db.sync().pause());
    let invitation = now(db.invitation());
    let mut out = std::io::stdout().lock();
    writeln!(out, "invitation {invitation}").expect("stdout");
    out.flush().expect("stdout");
    for line in std::io::stdin().lock().lines() {
        let line = line.expect("stdin");
        if line.trim() == "quit" {
            break;
        }
        let answer = match command(&db, &line) {
            Ok(text) if text.is_empty() => "ok".to_owned(),
            Ok(text) => format!("ok {text}"),
            Err(why) => format!("err {why}"),
        };
        writeln!(out, "{answer}").expect("stdout");
        out.flush().expect("stdout");
    }
}
