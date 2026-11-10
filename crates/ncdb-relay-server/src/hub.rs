//! Where a push tells the subscriptions of its space that the log grew
//! (RFC 0007, "Subscribe": nothing polls), and the open connections of a
//! member that it was added to a space (decision R13). Each space has a
//! watch of its head; a subscription waits for it to change and then reads
//! its new entries through the relay core. Each authenticated connection
//! is listed under its member, with its outgoing queue.
//!
//! **What this does not do.** It only sees the pushes of this server
//! process: another process that appends to the same SQLite database does
//! not wake this one's subscriptions, nor tell this one's connections that
//! their member was added.

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use ncdb_core::ids::{MemberId, SpaceId};
use ncdb_relay::store::Position;
use tokio::sync::{mpsc, watch};

use crate::connection::Out;
use crate::wire;

/// The heads the subscriptions of each space watch, and the open
/// connections of each member.
#[derive(Default)]
pub(crate) struct Hub {
    heads: Mutex<HashMap<SpaceId, watch::Sender<Position>>>,
    members: Mutex<HashMap<MemberId, HashMap<u64, mpsc::Sender<Out>>>>,
    next: AtomicU64,
}

impl Hub {
    /// Lists a connection of `member`, whose outgoing queue is `out`; its
    /// id in the hub.
    pub fn list(&self, member: MemberId, out: mpsc::Sender<Out>) -> u64 {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let mut members = self.members.lock().unwrap_or_else(|e| e.into_inner());
        members.entry(member).or_default().insert(id, out);
        id
    }

    /// The connection `id` of `member` closed.
    pub fn unlist(&self, member: MemberId, id: u64) {
        let mut members = self.members.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(connections) = members.get_mut(&member) {
            connections.remove(&id);
            if connections.is_empty() {
                members.remove(&member);
            }
        }
    }

    /// Sends `added {space}` to every open connection of each of `added`
    /// except `from`, which queues its own right after its `pushed`.
    /// Best-effort: each message waits for room in its connection's queue
    /// on a task of its own, so a slow connection does not hold up the
    /// push, and a connection that closes first loses it.
    pub fn added(&self, space: SpaceId, added: &[MemberId], from: u64) {
        let members = self.members.lock().unwrap_or_else(|e| e.into_inner());
        for member in added {
            let Some(connections) = members.get(member) else {
                continue;
            };
            for (_, out) in connections.iter().filter(|(id, _)| **id != from) {
                let out = out.clone();
                let message = wire::added(space);
                tokio::spawn(async move {
                    let _ = out.send(Out::Message(message)).await;
                });
            }
        }
    }

    /// A watch that changes whenever an append to `space` is announced.
    pub fn watch(&self, space: SpaceId) -> watch::Receiver<Position> {
        let mut spaces = self.heads.lock().unwrap_or_else(|e| e.into_inner());
        spaces
            .entry(space)
            .or_insert_with(|| watch::channel(0).0)
            .subscribe()
    }

    /// Entries up to `head` were appended to `space`.
    pub fn appended(&self, space: SpaceId, head: Position) {
        let mut spaces = self.heads.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(sender) = spaces.get(&space) {
            if sender.receiver_count() == 0 {
                spaces.remove(&space);
            } else {
                sender.send_modify(|h| *h = (*h).max(head));
            }
        }
    }
}
