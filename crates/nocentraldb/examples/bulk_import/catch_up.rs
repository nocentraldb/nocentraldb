//! The second device: a device of the same account, paired by the first,
//! which catches up on the burst from the relay. It runs as a process of
//! its own, `bulk_import catch-up <dir> <relay> <pair|resume>`, so that its
//! memory is its own; the first device drives it through stdin and stdout,
//! one line each way.
//!
//! With `pair`, it is new: it prints `invitation <text>`, reads `answer
//! <text> <space>`, completes the pairing, waits until it holds `space` and
//! prints `joined`. With `resume`, it opens the database it paired before.
//! Then it reads one more line: `leave` ends it, and `wait <space> <last>`
//! waits until it holds the item `last` of `space`, counts the items it
//! reads through the index and prints `caught-up <millis> <items> <database
//! bytes> <log bytes> <peak memory bytes>`. The time runs from the pairing,
//! or from the open with `resume`.

use std::io::{BufRead, Write};
use std::path::Path;
use std::str::FromStr;
use std::time::Instant;

use nocentraldb::{Db, ObjectId, PairingAnswer, SpaceId};

use crate::device;

/// The next line on stdin, split into words.
fn read() -> Result<Vec<String>, String> {
    let mut line = String::new();
    std::io::stdin()
        .lock()
        .read_line(&mut line)
        .map_err(|e| e.to_string())?;
    Ok(line.split_whitespace().map(str::to_owned).collect())
}

fn say(line: &str) -> Result<(), String> {
    println!("{line}");
    std::io::stdout().flush().map_err(|e| e.to_string())
}

/// Whether `db` holds the item `id` of `space`.
fn holds(db: &Db, space: SpaceId, id: Option<ObjectId>) -> Result<bool, String> {
    let Ok(space) = now(db.spaces().get(space)) else {
        return Ok(false);
    };
    match id {
        None => Ok(true),
        Some(id) => Ok(now(space.get("tasks", id))
            .map_err(|e| e.to_string())?
            .is_some()),
    }
}

/// Runs the second device in `dir`, syncing with `relay`, in `mode`.
pub async fn run(dir: &Path, relay: &str, mode: &str) -> Result<(), String> {
    let (db, start) = match mode {
        "pair" => {
            let db = device::open(dir, "tablet", Some(relay), true).await?;
            say(&format!("invitation {}", db.invitation().await))?;
            let words = read()?;
            let [command, answer, space] = &words[..] else {
                return Err(format!("unexpected line: {words:?}"));
            };
            if command != "answer" {
                return Err(format!("unexpected line: {words:?}"));
            }
            let answer = PairingAnswer::from_str(answer).map_err(|e| e.to_string())?;
            let space = SpaceId::from_str(space).map_err(|e| e.to_string())?;
            let start = Instant::now();
            db.account()
                .complete_pairing(&answer)
                .await
                .map_err(|e| e.to_string())?;
            device::until("the second device holds the space", || {
                holds(&db, space, None)
            })
            .await?;
            say("joined")?;
            (db, start)
        }
        "resume" => {
            let start = Instant::now();
            (
                device::open(dir, "tablet", Some(relay), false).await?,
                start,
            )
        }
        _ => return Err(format!("unknown mode {mode}")),
    };

    let words = read()?;
    let (space, last) = match &words[..] {
        [command] if command == "leave" => return Ok(()),
        [command, space, last] if command == "wait" => (
            SpaceId::from_str(space).map_err(|e| e.to_string())?,
            ObjectId::from_str(last).map_err(|e| e.to_string())?,
        ),
        _ => return Err(format!("unexpected line: {words:?}")),
    };
    device::until("the second device holds the last item", || {
        holds(&db, space, Some(last))
    })
    .await?;
    let caught_up = start.elapsed();

    let mut items = 0;
    let all = db
        .index("tasks", "byPriority")
        .map_err(|e| e.to_string())?
        .take(1000);
    let mut page = db.find(&all).await.map_err(|e| e.to_string())?;
    loop {
        items += page.items.len();
        match page.cursor {
            Some(cursor) if page.has_more => {
                page = db
                    .find(all.clone().after(cursor))
                    .await
                    .map_err(|e| e.to_string())?;
            }
            _ => break,
        }
    }
    let (file, wal) = device::size(&dir.join("tablet"));
    say(&format!(
        "caught-up {} {items} {file} {wal} {}",
        caught_up.as_millis(),
        crate::memory::own_peak().unwrap_or(0)
    ))
}

/// The output of an SDK future, which is ready at its first poll: every
/// call of the SDK finishes on the device; syncing runs on its own thread.
fn now<F: std::future::Future>(future: F) -> F::Output {
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    match std::pin::pin!(future).poll(&mut cx) {
        std::task::Poll::Ready(output) => output,
        std::task::Poll::Pending => unreachable!("an SDK call waited"),
    }
}
