//! The NoCentralDB relay server: RFC 0007's protocol (`docs/rfc/0007-relay.md`,
//! "The protocol") over WebSocket, in front of the relay core of
//! `ncdb-relay`. The server checks nothing itself: every push, pull and
//! subscription goes through [`ncdb_relay::Relay`], and the connection's
//! signed challenge through [`ncdb_relay::auth`].
//!
//! [`wire`] reads and writes the messages, `connection` serves one member's
//! connection, `subscription` one subscription, and `hub` tells the
//! subscriptions of a space that a push appended to it, and the open
//! connections of a member that a push added it to a space (`added`). The `ncdb-relay`
//! binary runs a [`Server`] locally on the SQLite backend.
//!
//! **What this does not do yet**, refused with one of RFC 0007's codes:
//! large changes ("Large changes"), since the largest frame this server
//! accepts, [`MAX_FRAME`], fits into one message, so `upload` and an upload
//! reference answer `too_large`. A `keys` or `person-keys` answer that does
//! not fit into one message answers `too_large` too. It serves plain
//! WebSocket, so it binds only a loopback address: TLS is left to a proxy
//! on the same host (see the crate's README). Its subscriptions wake
//! only on pushes to this server process.

use std::collections::BTreeSet;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use ncdb_relay::store::RelayStore;
use ncdb_relay::{Config, Relay};
use tokio::net::{TcpListener, ToSocketAddrs, lookup_host};
use tokio::sync::Semaphore;

use connection::Shared;
use hub::Hub;

mod accept;
mod connection;
mod hub;
mod subscription;
pub mod wire;

/// The largest message, in bytes, on every backend (RFC 0007, "Transport
/// and encoding").
pub const MAX_MESSAGE: usize = 128 * 1024;

/// The most frames one push carries (RFC 0007, "Transport and encoding").
pub const MAX_PUSH: usize = 32;

/// The largest frame this server accepts: the largest whose entry still
/// fits into one `entries` message, since this server carries no large
/// changes (RFC 0007, "Large changes").
pub const MAX_FRAME: usize = MAX_MESSAGE - 1024;

/// The most messages a connection's outgoing queue holds, at most 4 MiB.
pub const QUEUE: usize = 32;

/// How long a connection waits for progress in writing a message to its
/// member before it closes the connection (RFC 0007, "Liveness").
pub const WRITE_TIMEOUT: Duration = Duration::from_secs(60);

/// How long each step of the opening handshake may take (RFC 0007,
/// "Opening a connection"): the WebSocket upgrade, `hello`, and `auth`
/// after the challenge. RFC 0007 names no deadline for it; a member on a
/// working network finishes each step in well under a second.
pub const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

/// The most connections a server holds open at once, authenticated or
/// not. A connection above it is closed as soon as it is accepted. It
/// stays below the 1,024 file descriptors many systems give a process by
/// default, and bounds the outgoing queues of all connections to 2 GiB.
pub const MAX_CONNECTIONS: usize = 512;

/// The most connections a server holds open at once that have not
/// finished the opening handshake. A connection above it is closed as soon
/// as it is accepted, so peers that stop in the handshake cannot take the
/// places of authenticated members.
pub const MAX_HANDSHAKES: usize = 64;

/// The protocol versions this server speaks.
pub const VERSIONS: [u64; 1] = [1];

/// The relay core's configuration for this server: members of these
/// accounts may create spaces.
pub fn config(allowed_accounts: BTreeSet<[u8; 32]>) -> Config {
    let mut config = Config::new(allowed_accounts);
    config.max_frame = MAX_FRAME;
    config
}

/// A relay server, bound to its address.
pub struct Server<S> {
    listener: TcpListener,
    shared: Arc<Shared<S>>,
    max_connections: usize,
    max_handshakes: usize,
}

impl<S: RelayStore + 'static> Server<S> {
    /// A server on `addr` over the relay core `relay`, whose deployment has
    /// the stable id `relay_id`.
    ///
    /// `addr` must name only loopback addresses: this server serves plain
    /// WebSocket, and RFC 0007 requires TLS on the network, which a proxy
    /// on the same host adds. Any other address is `InvalidInput`, and
    /// nothing is bound.
    pub async fn bind(
        addr: impl ToSocketAddrs,
        relay: Relay<S>,
        relay_id: [u8; 16],
    ) -> io::Result<Server<S>> {
        let addrs: Vec<SocketAddr> = lookup_host(addr).await?.collect();
        if let Some(beyond) = addrs.iter().find(|a| !a.ip().is_loopback()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "{beyond} is not a loopback address: this server serves plain ws://, \
                     so it listens only on the loopback, behind a TLS proxy on the same host"
                ),
            ));
        }
        let listener = TcpListener::bind(addrs.as_slice()).await?;
        let shared = Arc::new(Shared {
            relay: Arc::new(relay),
            relay_id,
            hub: Hub::default(),
            write_timeout: WRITE_TIMEOUT,
            handshake_timeout: HANDSHAKE_TIMEOUT,
        });
        Ok(Server {
            listener,
            shared,
            max_connections: MAX_CONNECTIONS,
            max_handshakes: MAX_HANDSHAKES,
        })
    }

    /// A connection closes once writing a message to its member makes no
    /// progress for `timeout`, instead of [`WRITE_TIMEOUT`].
    pub fn with_write_timeout(mut self, timeout: Duration) -> Server<S> {
        if let Some(shared) = Arc::get_mut(&mut self.shared) {
            shared.write_timeout = timeout;
        }
        self
    }

    /// Each step of the opening handshake closes the connection once it
    /// takes longer than `timeout`, instead of [`HANDSHAKE_TIMEOUT`].
    pub fn with_handshake_timeout(mut self, timeout: Duration) -> Server<S> {
        if let Some(shared) = Arc::get_mut(&mut self.shared) {
            shared.handshake_timeout = timeout;
        }
        self
    }

    /// The server holds at most `max` connections open at once, instead of
    /// [`MAX_CONNECTIONS`].
    pub fn with_max_connections(mut self, max: usize) -> Server<S> {
        self.max_connections = max;
        self
    }

    /// The server holds at most `max` connections open at once that have
    /// not finished the opening handshake, instead of [`MAX_HANDSHAKES`].
    pub fn with_max_handshakes(mut self, max: usize) -> Server<S> {
        self.max_handshakes = max;
        self
    }

    /// The address the server listens on.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.listener.local_addr()
    }

    /// Accepts connections until the listener itself is gone. Any other
    /// error from `accept`, such as running out of file descriptors, is
    /// logged to stderr, and the server accepts again after a pause.
    ///
    /// A connection takes a place among the server's connections for as
    /// long as it is open, and one among its handshakes until it is sent
    /// `welcome` or closes. When either has no place left, the server
    /// closes the connection as soon as it accepts it, before the
    /// WebSocket upgrade: RFC 0007 lets a relay close any connection at
    /// any time ("Liveness"), and the member reconnects with back-off.
    pub async fn run(self) -> io::Result<()> {
        let connections = Arc::new(Semaphore::new(self.max_connections));
        let handshakes = Arc::new(Semaphore::new(self.max_handshakes));
        loop {
            let stream = accept::next(&self.listener).await?;
            let (Ok(place), Ok(handshake)) = (
                connections.clone().try_acquire_owned(),
                handshakes.clone().try_acquire_owned(),
            ) else {
                drop(stream);
                continue;
            };
            let shared = self.shared.clone();
            tokio::spawn(async move {
                connection::serve(stream, shared, handshake).await;
                drop(place);
            });
        }
    }
}
