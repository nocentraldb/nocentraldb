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
//! let relay = TestRelay::start(Options::new().admit_any())?;
//! // Open each database with `OpenOptions::relay(&relay.url())` and
//! // `OpenOptions::admission(relay.admissions())`: the SDK fetches an
//! // admission for its account when the relay refuses a new space.
//! # Ok::<(), std::io::Error>(())
//! ```
//!
//! A test that knows its account's identity before it opens the database
//! lists it with [`Options::allow_identity`] instead, as a relay in
//! production lists its accounts.
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
//! [`Options::admit_any`] exists only here: the relay itself stays as in
//! production, and admits any account only through admissions this test
//! relay signs, which an app's database asks for through
//! `OpenOptions::admission`.

use std::collections::BTreeSet;
use std::io;
use std::net::SocketAddr;
use std::sync::{Arc, mpsc};
use std::thread::JoinHandle;

use ed25519_dalek::SigningKey;
use ncdb_core::admission::Admission;
use ncdb_relay::Relay;
use ncdb_relay::sqlite::SqliteStore;
use tokio::sync::oneshot;

use crate::{Server, config};

/// What a [`TestRelay`] admits.
#[derive(Clone, Debug, Default)]
pub struct Options {
    identities: Vec<String>,
    admission_keys: BTreeSet<[u8; 32]>,
    admit_any: bool,
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

    /// Any account may create spaces, through an admission the test relay
    /// signs with an admission key of its own, which
    /// [`TestRelay::admissions`] hands to the SDK. An app's test then needs
    /// no identity before it opens its database.
    pub fn admit_any(mut self) -> Options {
        self.admit_any = true;
        self
    }
}

/// How long an admission of [`TestRelay::admissions`] holds: a day.
const ADMISSION_FOR: u64 = 24 * 60 * 60;

/// How many spaces an admission of [`TestRelay::admissions`] lets an
/// account create.
const ADMITTED_SPACES: u64 = 1_000_000;

/// A relay server on SQLite in memory and a free loopback port, which stops
/// when dropped.
pub struct TestRelay {
    addr: SocketAddr,
    relay_id: [u8; 16],
    admission_key: Option<Arc<SigningKey>>,
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
        let admission_key = if options.admit_any {
            let mut secret = [0u8; 32];
            getrandom::fill(&mut secret).map_err(|e| io::Error::other(e.to_string()))?;
            let key = SigningKey::from_bytes(&secret);
            config.admission_keys.insert(key.verifying_key().to_bytes());
            Some(Arc::new(key))
        } else {
            None
        };
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
            admission_key,
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

    /// The function an app's database fetches its admissions with, for
    /// `OpenOptions::admission`: it answers an admission's text for the
    /// identity it gets, for this relay, a day and a million spaces. It
    /// answers an error when the relay was started without
    /// [`Options::admit_any`], or for an identity that is not 64 hex
    /// digits.
    pub fn admissions(
        &self,
    ) -> impl Fn(String) -> std::future::Ready<io::Result<String>> + Send + Sync + 'static {
        let key = self.admission_key.clone();
        let relay_id = self.relay_id;
        move |identity| std::future::ready(admission(key.as_deref(), relay_id, &identity))
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

/// An admission's text for `identity` on the relay `relay_id`, signed with
/// `key`.
fn admission(key: Option<&SigningKey>, relay_id: [u8; 16], identity: &str) -> io::Result<String> {
    let key = key.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::Unsupported,
            "the test relay was started without admit_any",
        )
    })?;
    let identity = hex32(identity).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "the identity is not 64 hex digits",
        )
    })?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| io::Error::other("the clock is before 1970"))?
        .as_secs();
    let admission = Admission::sign(
        key,
        relay_id.to_vec(),
        identity,
        now + ADMISSION_FOR,
        ADMITTED_SPACES,
    );
    Ok(admission.to_string())
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
