//! What the relay knows of each open connection between invocations, and
//! the connection API it posts through. A Lambda invocation keeps nothing in
//! memory for the next, so the step of a connection's opening, the member
//! it acts as, and its subscriptions live in a store: the `connections`
//! table on AWS (RFC 0007, "Tables and keys"), with a TTL for what a missed
//! `$disconnect` leaves behind; [`MemoryConnections`] here, for tests and
//! the local harness.
//!
//! A store either forgets a connection when it ends, by default, or keeps
//! its records for a number of days the operator sets, for abuse
//! investigation (RFC 0007, "What the relay sees"; NOC-532): which member
//! id was connected when, and to which spaces it subscribed. A kept record
//! reads as gone, exactly as a forgotten one.

use std::collections::HashMap;
use std::sync::Mutex;

use ncdb_core::ids::{MemberId, SpaceId};
use ncdb_relay::store::{Epoch, StoreError};

/// How long API Gateway keeps a WebSocket connection open: 2 hours (RFC
/// 0007, "Liveness"). A connection's record expires then.
pub const MAX_CONNECTION_AGE: u64 = 2 * 60 * 60 * 1000;

/// One day, in milliseconds: the unit a store keeps ended records for.
pub const DAY: u64 = 24 * 60 * 60 * 1000;

/// The step of a connection's opening (RFC 0007, "Opening a connection").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// Opened; the relay waits for `hello`.
    Hello,
    /// The relay sent this nonce in its challenge and waits for `auth`.
    Auth {
        /// The challenge's nonce.
        nonce: [u8; 32],
    },
    /// The connection acts as this member.
    Member(MemberId),
}

/// One open connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Connection {
    /// The step of its opening.
    pub stage: Stage,
    /// When it was opened, in milliseconds since the Unix epoch.
    pub connected_at: u64,
    /// When its record expires, in milliseconds since the Unix epoch: 2
    /// hours after it was opened.
    pub expires_at: u64,
}

/// A subscription of a connection to a space (RFC 0007, "Subscribe").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Subscription {
    /// The member its connection acts as.
    pub member: MemberId,
    /// The request id of its `subscribe`, which an error ending it carries.
    pub req: u64,
    /// The relay epoch it subscribed under.
    pub epoch: Epoch,
}

/// The open connections and their subscriptions. Its futures are `Send`.
/// A connection whose record expired or that ended, and its subscriptions,
/// read as gone, whether the store deleted them or keeps them. [`crate::contract::run`]
/// holds every store to these rules.
pub trait Connections: Send + Sync {
    /// Stores `connection` under `id`, replacing what was stored.
    fn put(
        &self,
        id: &str,
        connection: Connection,
    ) -> impl Future<Output = Result<(), StoreError>> + Send;

    /// The connection `id`, if it is open.
    fn get(&self, id: &str) -> impl Future<Output = Result<Option<Connection>, StoreError>> + Send;

    /// Ends the connection `id` and its subscriptions: forgets them, or
    /// keeps them as ended when the store keeps records.
    fn remove(&self, id: &str) -> impl Future<Output = Result<(), StoreError>> + Send;

    /// Subscribes the connection `id` to `space`, replacing a subscription
    /// it had there.
    fn subscribe(
        &self,
        id: &str,
        space: SpaceId,
        subscription: Subscription,
    ) -> impl Future<Output = Result<(), StoreError>> + Send;

    /// Ends the subscription of the connection `id` to `space`: forgets it,
    /// or keeps it as ended when the store keeps records.
    fn unsubscribe(
        &self,
        id: &str,
        space: SpaceId,
    ) -> impl Future<Output = Result<(), StoreError>> + Send;

    /// The subscriptions to `space`, with their connections' ids.
    fn subscribers(
        &self,
        space: SpaceId,
    ) -> impl Future<Output = Result<Vec<(String, Subscription)>, StoreError>> + Send;

    /// The ids of the connections that act as `member`.
    fn of_member(
        &self,
        member: MemberId,
    ) -> impl Future<Output = Result<Vec<String>, StoreError>> + Send;
}

/// What posting to a connection did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Posted {
    /// API Gateway took the message.
    Sent,
    /// The connection is gone (API Gateway's `GoneException`).
    Gone,
}

/// API Gateway's connection API: post a message to a connection, and close
/// one. Its futures are `Send`.
pub trait ConnectionApi: Send + Sync {
    /// Posts one message to the connection `id`: the base64 text of an
    /// RFC 0007 message, within one of API Gateway's 32 KB frames.
    fn post(
        &self,
        id: &str,
        message: Vec<u8>,
    ) -> impl Future<Output = Result<Posted, StoreError>> + Send;

    /// Closes the connection `id`; API Gateway then sends `$disconnect`.
    fn close(&self, id: &str) -> impl Future<Output = Result<(), StoreError>> + Send;
}

/// The connections in memory, for tests and the local harness. It keeps
/// ended records without end when it keeps records at all; [`Kept::until`]
/// says when a store on a TTL would delete them.
#[derive(Default)]
pub struct MemoryConnections {
    inner: Mutex<Memory>,
}

/// What a store that keeps records holds of an ended connection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Kept {
    /// The member it acted as, once it authenticated.
    pub member: Option<MemberId>,
    /// When it was opened, in milliseconds since the Unix epoch.
    pub connected_at: u64,
    /// When it ended.
    pub ended_at: u64,
    /// When the record goes: the days kept after it ended.
    pub until: u64,
    /// Every space it subscribed to, in order of id.
    pub spaces: Vec<SpaceId>,
}

#[derive(Default)]
struct Memory {
    /// How long ended records are kept, in milliseconds; 0 keeps none.
    keep: u64,
    connections: HashMap<String, Connection>,
    subscriptions: HashMap<(String, SpaceId), Subscription>,
    /// When each kept connection, and each kept subscription, ended.
    ended: HashMap<String, u64>,
    ended_subscriptions: HashMap<(String, SpaceId), u64>,
}

impl Memory {
    /// The connection `id`, unless it ended or its record expired by `now`,
    /// as the `connections` table's TTL would delete it.
    fn live(&self, id: &str, now: u64) -> Option<Connection> {
        if self.ended.contains_key(id) {
            return None;
        }
        self.connections
            .get(id)
            .copied()
            .filter(|connection| connection.expires_at > now)
    }
}

impl MemoryConnections {
    /// A store that keeps an ended connection's records for `days`.
    pub fn keeping(days: u32) -> MemoryConnections {
        let store = MemoryConnections::default();
        store.with(|memory| memory.keep = u64::from(days) * DAY);
        store
    }

    fn with<T>(&self, f: impl FnOnce(&mut Memory) -> T) -> T {
        f(&mut self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()))
    }

    /// The number of connections it holds, kept ones included, for tests.
    pub fn len(&self) -> usize {
        self.with(|memory| memory.connections.len())
    }

    /// Whether it holds no connection.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The number of subscriptions, kept ones included, for tests.
    pub fn subscriptions(&self) -> usize {
        self.with(|memory| memory.subscriptions.len())
    }

    /// The kept records of the ended connection `id`, for tests.
    pub fn kept(&self, id: &str) -> Option<Kept> {
        self.with(|memory| {
            let ended_at = *memory.ended.get(id)?;
            let connection = memory.connections.get(id)?;
            let mut spaces: Vec<SpaceId> = memory
                .subscriptions
                .keys()
                .filter(|(conn, _)| conn == id)
                .map(|(_, space)| *space)
                .collect();
            spaces.sort();
            Some(Kept {
                member: match connection.stage {
                    Stage::Member(member) => Some(member),
                    _ => None,
                },
                connected_at: connection.connected_at,
                ended_at,
                until: ended_at.saturating_add(memory.keep),
                spaces,
            })
        })
    }
}

impl Connections for MemoryConnections {
    async fn put(&self, id: &str, connection: Connection) -> Result<(), StoreError> {
        self.with(|memory| {
            memory.ended.remove(id);
            memory.connections.insert(id.to_owned(), connection)
        });
        Ok(())
    }

    async fn get(&self, id: &str) -> Result<Option<Connection>, StoreError> {
        let now = crate::contract::now();
        Ok(self.with(|memory| memory.live(id, now)))
    }

    async fn remove(&self, id: &str) -> Result<(), StoreError> {
        let now = crate::contract::now();
        self.with(|memory| {
            if memory.keep == 0 {
                memory.connections.remove(id);
                memory.subscriptions.retain(|(conn, _), _| conn != id);
                return;
            }
            if !memory.connections.contains_key(id) || memory.ended.contains_key(id) {
                return;
            }
            memory.ended.insert(id.to_owned(), now);
            let open: Vec<(String, SpaceId)> = memory
                .subscriptions
                .keys()
                .filter(|(conn, _)| conn == id)
                .cloned()
                .collect();
            for key in open {
                memory.ended_subscriptions.entry(key).or_insert(now);
            }
        });
        Ok(())
    }

    async fn subscribe(
        &self,
        id: &str,
        space: SpaceId,
        subscription: Subscription,
    ) -> Result<(), StoreError> {
        self.with(|memory| {
            let key = (id.to_owned(), space);
            memory.ended_subscriptions.remove(&key);
            memory.subscriptions.insert(key, subscription)
        });
        Ok(())
    }

    async fn unsubscribe(&self, id: &str, space: SpaceId) -> Result<(), StoreError> {
        let now = crate::contract::now();
        self.with(|memory| {
            let key = (id.to_owned(), space);
            if memory.keep == 0 {
                memory.subscriptions.remove(&key);
            } else if memory.subscriptions.contains_key(&key) {
                memory.ended_subscriptions.entry(key).or_insert(now);
            }
        });
        Ok(())
    }

    async fn subscribers(&self, space: SpaceId) -> Result<Vec<(String, Subscription)>, StoreError> {
        let now = crate::contract::now();
        Ok(self.with(|memory| {
            let mut found: Vec<(String, Subscription)> = memory
                .subscriptions
                .iter()
                .filter(|(key, _)| !memory.ended_subscriptions.contains_key(*key))
                .filter(|((conn, s), _)| *s == space && memory.live(conn, now).is_some())
                .map(|((conn, _), sub)| (conn.clone(), *sub))
                .collect();
            found.sort_by(|a, b| a.0.cmp(&b.0));
            found
        }))
    }

    async fn of_member(&self, member: MemberId) -> Result<Vec<String>, StoreError> {
        let now = crate::contract::now();
        Ok(self.with(|memory| {
            let mut found: Vec<String> = memory
                .connections
                .iter()
                .filter(|(_, c)| c.stage == Stage::Member(member) && c.expires_at > now)
                .filter(|(id, _)| !memory.ended.contains_key(id.as_str()))
                .map(|(id, _)| id.clone())
                .collect();
            found.sort();
            found
        }))
    }
}
