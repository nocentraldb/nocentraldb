//! Where a push tells the subscriptions of its space that the log grew
//! (RFC 0007, "Subscribe": nothing polls). Each space has a watch of its
//! head; a subscription waits for it to change and then reads its new
//! entries through the relay core.
//!
//! **What this does not do.** It only sees the pushes of this server
//! process: another process that appends to the same SQLite database does
//! not wake this one's subscriptions.

use std::collections::HashMap;
use std::sync::Mutex;

use ncdb_core::ids::SpaceId;
use ncdb_relay::store::Position;
use tokio::sync::watch;

/// The heads the subscriptions of each space watch.
#[derive(Default)]
pub(crate) struct Hub(Mutex<HashMap<SpaceId, watch::Sender<Position>>>);

impl Hub {
    /// A watch that changes whenever an append to `space` is announced.
    pub fn watch(&self, space: SpaceId) -> watch::Receiver<Position> {
        let mut spaces = self.0.lock().unwrap_or_else(|e| e.into_inner());
        spaces
            .entry(space)
            .or_insert_with(|| watch::channel(0).0)
            .subscribe()
    }

    /// Entries up to `head` were appended to `space`.
    pub fn appended(&self, space: SpaceId, head: Position) {
        let mut spaces = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(sender) = spaces.get(&space) {
            if sender.receiver_count() == 0 {
                spaces.remove(&space);
            } else {
                sender.send_modify(|h| *h = (*h).max(head));
            }
        }
    }
}
