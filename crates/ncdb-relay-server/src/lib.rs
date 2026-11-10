//! The NoCentralDB relay server: RFC 0007's protocol (`docs/rfc/0007-relay.md`,
//! "The protocol") over WebSocket, in front of the relay core of
//! `ncdb-relay`. The server checks nothing itself: every push, pull and
//! subscription goes through [`ncdb_relay::Relay`], and the connection's
//! signed challenge through [`ncdb_relay::auth`].
//!
//! [`wire`] reads and writes the messages, `connection` serves one member's
//! connection, `subscription` one subscription, and `hub` tells the
//! subscriptions of a space that a push appended to it. The `ncdb-relay`
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

use ncdb_relay::store::RelayStore;
use ncdb_relay::{Config, Relay};
use tokio::net::{TcpListener, ToSocketAddrs, lookup_host};

use connection::Shared;
use hub::Hub;

mod connection;
mod hub;
mod subscription;
pub mod wire;

/// The largest message, in bytes, on every backend (RFC 0007, decision R4).
pub const MAX_MESSAGE: usize = 128 * 1024;

/// The most frames one push carries (RFC 0007, decision R4).
pub const MAX_PUSH: usize = 32;

/// The largest frame this server accepts: the largest whose entry still
/// fits into one `entries` message, since this server carries no large
/// changes (RFC 0007, "Large changes").
pub const MAX_FRAME: usize = MAX_MESSAGE - 1024;

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
        });
        Ok(Server { listener, shared })
    }

    /// The address the server listens on.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.listener.local_addr()
    }

    /// Accepts connections until the listener fails.
    pub async fn run(self) -> io::Result<()> {
        loop {
            let (stream, _) = self.listener.accept().await?;
            tokio::spawn(connection::serve(stream, self.shared.clone()));
        }
    }
}
