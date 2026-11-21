//! The second device: a new device of the same account, paired by the
//! first, which then catches up on the imported space from the relay. It
//! runs as a process of its own, `bulk_import catch-up`, so that its
//! memory is its own; the first device drives it through stdin and stdout,
//! one line each way.
//!
//! It prints `invitation <text>`, reads `answer <text> <space> <last>`,
//! completes the pairing, waits until it holds the item `last` of `space`,
//! counts the items it reads through the index, and prints
//! `caught-up <millis> <items> <database bytes> <log bytes> <peak memory
//! bytes>`.

use std::io::{BufRead, Write};
use std::path::Path;
use std::str::FromStr;
use std::time::Instant;

use nocentraldb::{ObjectId, PairingAnswer, SpaceId};

use crate::device;

/// Runs the second device in `dir`, syncing with `relay`.
pub async fn run(dir: &Path, relay: &str) -> Result<(), String> {
    let db = device::open(dir, "tablet", Some(relay), true).await?;
    println!("invitation {}", db.invitation().await);
    std::io::stdout().flush().map_err(|e| e.to_string())?;

    let mut line = String::new();
    std::io::stdin()
        .lock()
        .read_line(&mut line)
        .map_err(|e| e.to_string())?;
    let mut words = line.split_whitespace();
    let (Some("answer"), Some(answer), Some(space), Some(last)) =
        (words.next(), words.next(), words.next(), words.next())
    else {
        return Err(format!("unexpected line: {line}"));
    };
    let answer = PairingAnswer::from_str(answer).map_err(|e| e.to_string())?;
    let space = SpaceId::from_str(space).map_err(|e| e.to_string())?;
    let last = ObjectId::from_str(last).map_err(|e| e.to_string())?;

    let start = Instant::now();
    db.account()
        .complete_pairing(&answer)
        .await
        .map_err(|e| e.to_string())?;
    device::until("the second device holds the last item", || {
        let Ok(space) = futures_now(db.spaces().get(space)) else {
            return Ok(false);
        };
        Ok(futures_now(space.get("tasks", last))
            .map_err(|e| e.to_string())?
            .is_some())
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
    println!(
        "caught-up {} {items} {file} {wal} {}",
        caught_up.as_millis(),
        crate::memory::own_peak().unwrap_or(0)
    );
    std::io::stdout().flush().map_err(|e| e.to_string())
}

/// The output of an SDK future, which is ready at its first poll: every
/// call of the SDK finishes on the device; syncing runs on its own thread.
fn futures_now<F: std::future::Future>(future: F) -> F::Output {
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    match std::pin::pin!(future).poll(&mut cx) {
        std::task::Poll::Ready(output) => output,
        std::task::Poll::Pending => unreachable!("an SDK call waited"),
    }
}
