//! A relay for tests: [`TestRelay`] runs this crate's [`Server`] on the
//! SQLite backend in memory, on a free port of the loopback, so that an app
//! or an engine that embeds the Rust SDK can test against a real relay
//! (NOC-507): a write on one member reaches another after sync, and a relay
//! that does not list the space creator's account refuses its genesis with
//! `not_allowed`.
//!
//! ```no_run
//! use ncdb_relay_server::local::{Options, TestRelay};
//!
//! # fn identity_of_my_account() -> String { String::new() }
//! // The identity the SDK reports with `db.account().identity()`.
//! let identity = identity_of_my_account();
//! let relay = TestRelay::start(Options::new().allow_identity(&identity))?;
//! // Open the databases with `OpenOptions::relay(relay.url())`.
//! # Ok::<(), std::io::Error>(())
//! ```
//!
//! The relay runs on a Tokio runtime of its own, on a thread of its own, so
//! a test needs no runtime, and one inside a runtime may start and drop it
//! too. Dropping it stops the server: it closes the port and every
//! connection, and its spaces are gone.
//!
//! **What this does not do.** It is for tests, not for serving members: it
//! keeps nothing after it is dropped, and like every server of this crate it
//! serves plain `ws://` on the loopback only. Each relay draws a new relay
//! id, which [`TestRelay::relay_id`] returns for an admission a test signs.

use std::collections::BTreeSet;
use std::io;
use std::net::SocketAddr;
use std::sync::mpsc;
use std::thread::JoinHandle;

use ncdb_relay::Relay;
use ncdb_relay::sqlite::SqliteStore;
use tokio::sync::oneshot;

use crate::{Server, config};

/// What a [`TestRelay`] admits.
#[derive(Clone, Debug, Default)]
pub struct Options {
    identities: Vec<String>,
    admission_keys: BTreeSet<[u8; 32]>,
}

impl Options {
    /// A relay on which no member may create a space: it refuses every
    /// genesis with `not_allowed`, unless an admission admits its account.
    pub fn new() -> Options {
        Options::default()
    }

    /// Members of the account with this identity may create spaces: 64
    /// hex digits, as the SDK's `db.account().identity()` reports it (RFC
    /// 0015, "The account's identity"). May be given more than once.
    pub fn allow_identity(mut self, identity: &str) -> Options {
        self.identities.push(identity.to_owned());
        self
    }

    /// The relay takes admissions this Ed25519 public key signed (RFC
    /// 0015, "Admission tokens"), for its [`TestRelay::relay_id`]. May be
    /// given more than once.
    pub fn admission_key(mut self, key: [u8; 32]) -> Options {
        self.admission_keys.insert(key);
        self
    }
}

/// A relay server on SQLite in memory and a free loopback port, which stops
/// when dropped.
pub struct TestRelay {
    addr: SocketAddr,
    relay_id: [u8; 16],
    stop: Option<oneshot::Sender<()>>,
    thread: Option<JoinHandle<()>>,
}

impl TestRelay {
    /// Starts a relay with `options` and returns once it listens. An
    /// identity that is not 64 hex digits is `InvalidInput`, and nothing
    /// starts.
    pub fn start(options: Options) -> io::Result<TestRelay> {
        let mut allowed = BTreeSet::new();
        for identity in &options.identities {
            allowed.insert(hex32(identity).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "an allowed identity is not 64 hex digits",
                )
            })?);
        }
        let mut config = config(allowed);
        config.admission_keys = options.admission_keys;
        let mut relay_id = [0u8; 16];
        getrandom::fill(&mut relay_id).map_err(|e| io::Error::other(e.to_string()))?;
        let store = SqliteStore::in_memory().map_err(io::Error::other)?;

        let (started, listening) = mpsc::channel();
        let (stop, stopped) = oneshot::channel();
        let thread = std::thread::Builder::new()
            .name("ncdb-test-relay".into())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(2)
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(error) => {
                        let _ = started.send(Err(error));
                        return;
                    }
                };
                runtime.block_on(async move {
                    let bound = Server::bind("127.0.0.1:0", Relay::new(store, config), relay_id)
                        .await
                        .and_then(|server| Ok((server.local_addr()?, server)));
                    let server = match bound {
                        Ok((addr, server)) => {
                            let _ = started.send(Ok(addr));
                            server
                        }
                        Err(error) => {
                            let _ = started.send(Err(error));
                            return;
                        }
                    };
                    tokio::select! {
                        _ = server.run() => {}
                        _ = stopped => {}
                    }
                });
                // Dropping the runtime here, on its own thread, ends every
                // connection's task.
            })?;
        let addr = listening
            .recv()
            .map_err(|_| io::Error::other("the test relay's thread ended before it listened"))??;
        Ok(TestRelay {
            addr,
            relay_id,
            stop: Some(stop),
            thread: Some(thread),
        })
    }

    /// The URL members connect to: `ws://` and the loopback address it
    /// listens on.
    pub fn url(&self) -> String {
        format!("ws://{}", self.addr)
    }

    /// The relay's id, which an admission for this relay names (RFC 0015,
    /// "Admission tokens"). Each relay draws its own.
    pub fn relay_id(&self) -> [u8; 16] {
        self.relay_id
    }
}

impl Drop for TestRelay {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// 32 bytes written as 64 hex digits.
fn hex32(hex: &str) -> Option<[u8; 32]> {
    if hex.len() != 64 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let mut bytes = [0u8; 32];
    for (i, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).ok()?;
    }
    Some(bytes)
}
