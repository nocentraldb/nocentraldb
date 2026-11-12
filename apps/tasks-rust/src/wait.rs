//! Waiting for the relay, and for a stream's next item.
//!
//! The SDK syncs in the background. A command line exits after each
//! command, so before it does, it waits until the relay holds what this
//! device wrote, and until its spaces are synced. It follows the SDK's own
//! streams for that: `db.sync().subscribe()` and each space's
//! `space.status().subscribe()`. Nothing follows the list of spaces, so
//! waiting for a space another device shared with this one asks again
//! every 100 ms (a gap the README names).

use std::future::poll_fn;
use std::pin::Pin;
use std::time::Duration;

use futures_core::Stream;
use nocentraldb::{Connection, Db, SpaceId, SyncStatus};

/// How often `spaces().get` is asked again while a space has not arrived.
const POLL: Duration = Duration::from_millis(100);

/// Waits until this device is connected and the relay holds every change
/// it wrote; with `space`, until that space has arrived; and until each of
/// its spaces finished its first sync and is not behind. Fails after
/// `timeout`, saying what it still waits for.
pub async fn synced(db: &Db, space: Option<SpaceId>, timeout: Duration) -> Result<(), String> {
    let waiting = std::cell::RefCell::new(String::from("the relay"));
    let wait = async {
        let mut sync = db.sync().subscribe();
        while let Some(status) = next(&mut sync).await {
            match pending(&status) {
                Some(why) => *waiting.borrow_mut() = why,
                None => break,
            }
        }
        if let Some(id) = space {
            *waiting.borrow_mut() = format!("the space {id} to arrive");
            while db.spaces().get(id).await.is_err() {
                tokio::time::sleep(POLL).await;
            }
        }
        for info in db.spaces().list().await.map_err(|e| e.to_string())? {
            let Ok(space) = db.spaces().get(info.id).await else {
                continue;
            };
            *waiting.borrow_mut() = format!("the space {} to sync", info.id);
            let mut statuses = space.status().subscribe();
            while let Some(status) = next(&mut statuses).await {
                if status.initial_sync_complete && status.behind == 0 {
                    break;
                }
            }
        }
        Ok::<(), String>(())
    };
    match tokio::time::timeout(timeout, wait).await {
        Ok(result) => result,
        Err(_) => Err(format!(
            "not synced after {}s: still waiting for {}",
            timeout.as_secs(),
            waiting.borrow()
        )),
    }
}

/// What a sync status still waits for, or `None` when this device is
/// connected and the relay holds every change it wrote.
fn pending(status: &SyncStatus) -> Option<String> {
    if let Some(error) = &status.error {
        return Some(format!("the relay, which reports: {error}"));
    }
    if status.connection != Connection::Connected {
        return Some(format!("the connection, now {:?}", status.connection));
    }
    if status.unconfirmed > 0 {
        return Some(format!("{} changes to reach the relay", status.unconfirmed));
    }
    None
}

/// The next item of a stream.
pub async fn next<S: Stream + Unpin>(stream: &mut S) -> Option<S::Item> {
    poll_fn(|cx| Pin::new(&mut *stream).poll_next(cx)).await
}
