//! What the relay knows of each open connection between invocations, and
//! the connection API it posts through. A Lambda invocation keeps nothing in
//! memory for the next, so the step of a connection's opening, the member
//! it acts as, and its subscriptions live in a store: the `connections`
//! table on AWS (RFC 0007, "Tables and keys"), with a TTL for what a missed
//! `$disconnect` leaves behind; [`MemoryConnections`] here, for tests and
//! the local harness.

use std::collections::HashMap;
use std::sync::Mutex;

use ncdb_core::ids::{MemberId, SpaceId};
use ncdb_relay::store::{Epoch, StoreError};

/// How long API Gateway keeps a WebSocket connection open: 2 hours (RFC
/// 0007, "Liveness"). A connection's record expires then.
pub const MAX_CONNECTION_AGE: u64 = 2 * 60 * 60 * 1000;

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
pub trait Connections: Send + Sync {
    /// Stores `connection` under `id`, replacing what was stored.
    fn put(
        &self,
        id: &str,
        connection: Connection,
    ) -> impl Future<Output = Result<(), StoreError>> + Send;

    /// The connection `id`, if it is open.
    fn get(&self, id: &str) -> impl Future<Output = Result<Option<Connection>, StoreError>> + Send;

    /// Forgets the connection `id` and its subscriptions.
    fn remove(&self, id: &str) -> impl Future<Output = Result<(), StoreError>> + Send;

    /// Subscribes the connection `id` to `space`, replacing a subscription
    /// it had there.
    fn subscribe(
        &self,
        id: &str,
        space: SpaceId,
        subscription: Subscription,
    ) -> impl Future<Output = Result<(), StoreError>> + Send;

    /// Ends the subscription of the connection `id` to `space`.
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
    /// Posts one binary message to the connection `id`.
    fn post(
        &self,
        id: &str,
        message: Vec<u8>,
    ) -> impl Future<Output = Result<Posted, StoreError>> + Send;

    /// Closes the connection `id`; API Gateway then sends `$disconnect`.
    fn close(&self, id: &str) -> impl Future<Output = Result<(), StoreError>> + Send;
}

/// The connections in memory, for tests and the local harness.
#[derive(Default)]
pub struct MemoryConnections {
    inner: Mutex<Memory>,
}

#[derive(Default)]
struct Memory {
    connections: HashMap<String, Connection>,
    subscriptions: HashMap<(String, SpaceId), Subscription>,
}

impl MemoryConnections {
    fn with<T>(&self, f: impl FnOnce(&mut Memory) -> T) -> T {
        f(&mut self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()))
    }

    /// The number of open connections, for tests.
    pub fn len(&self) -> usize {
        self.with(|memory| memory.connections.len())
    }

    /// Whether no connection is open.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The number of subscriptions, for tests.
    pub fn subscriptions(&self) -> usize {
        self.with(|memory| memory.subscriptions.len())
    }
}

impl Connections for MemoryConnections {
    async fn put(&self, id: &str, connection: Connection) -> Result<(), StoreError> {
        self.with(|memory| memory.connections.insert(id.to_owned(), connection));
        Ok(())
    }

    async fn get(&self, id: &str) -> Result<Option<Connection>, StoreError> {
        Ok(self.with(|memory| memory.connections.get(id).copied()))
    }

    async fn remove(&self, id: &str) -> Result<(), StoreError> {
        self.with(|memory| {
            memory.connections.remove(id);
            memory.subscriptions.retain(|(conn, _), _| conn != id);
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
            memory
                .subscriptions
                .insert((id.to_owned(), space), subscription)
        });
        Ok(())
    }

    async fn unsubscribe(&self, id: &str, space: SpaceId) -> Result<(), StoreError> {
        self.with(|memory| memory.subscriptions.remove(&(id.to_owned(), space)));
        Ok(())
    }

    async fn subscribers(&self, space: SpaceId) -> Result<Vec<(String, Subscription)>, StoreError> {
        Ok(self.with(|memory| {
            let mut found: Vec<(String, Subscription)> = memory
                .subscriptions
                .iter()
                .filter(|((_, s), _)| *s == space)
                .map(|((conn, _), sub)| (conn.clone(), *sub))
                .collect();
            found.sort_by(|a, b| a.0.cmp(&b.0));
            found
        }))
    }

    async fn of_member(&self, member: MemberId) -> Result<Vec<String>, StoreError> {
        Ok(self.with(|memory| {
            let mut found: Vec<String> = memory
                .connections
                .iter()
                .filter(|(_, c)| c.stage == Stage::Member(member))
                .map(|(id, _)| id.clone())
                .collect();
            found.sort();
            found
        }))
    }
}
