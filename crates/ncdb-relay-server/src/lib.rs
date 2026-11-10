//! The NoCentralDB relay server: RFC 0007's protocol (`docs/rfc/0007-relay.md`,
//! "The protocol") over WebSocket, in front of the relay core of
//! `ncdb-relay`. The server checks nothing itself: every push, pull and
//! subscription goes through [`ncdb_relay::Relay`], and the connection's
//! signed challenge through [`ncdb_relay::auth`].

use std::collections::BTreeSet;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;

use ncdb_relay::store::RelayStore;
use ncdb_relay::{Config, Relay};
use tokio::net::{TcpListener, ToSocketAddrs};

/// The largest message, in bytes, on every backend (RFC 0007, decision R4).
pub const MAX_MESSAGE: usize = 128 * 1024;

/// The most frames one push carries (RFC 0007, decision R4).
pub const MAX_PUSH: usize = 32;

/// The largest frame this server accepts.
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
    relay: Arc<Relay<S>>,
    relay_id: [u8; 16],
}

impl<S: RelayStore + 'static> Server<S> {
    /// A server on `addr` over the relay core `relay`, whose deployment has
    /// the stable id `relay_id`.
    pub async fn bind(
        addr: impl ToSocketAddrs,
        relay: Relay<S>,
        relay_id: [u8; 16],
    ) -> io::Result<Server<S>> {
        let listener = TcpListener::bind(addr).await?;
        Ok(Server {
            listener,
            relay: Arc::new(relay),
            relay_id,
        })
    }

    /// The address the server listens on.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.listener.local_addr()
    }

    /// Accepts connections until the listener fails.
    pub async fn run(self) -> io::Result<()> {
        let _ = (&self.relay, self.relay_id);
        loop {
            let (stream, _) = self.listener.accept().await?;
            tokio::spawn(async move {
                let _ = tokio_tungstenite::accept_async(stream).await;
            });
        }
    }
}
