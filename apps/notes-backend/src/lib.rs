//! An example backend of a notes and projects app with open signup, as the
//! guide on spaces and invitations (`docs/spaces-and-invitations.md`)
//! describes it: it signs admissions for its users (RFC 0015, "Admission
//! tokens") and keeps a directory that maps a user to their account's
//! identity and their devices' invitations, with the invites that wait for
//! a user who has no account yet.
//!
//! It holds the app's admission key and the directory, and never a key
//! that reads content: it cannot add a member to a space, because only an
//! admin's device holds the space key.

pub mod admission;
pub mod directory;
mod http;

use std::collections::BTreeMap;
use std::io;
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ed25519_dalek::SigningKey;

use crate::directory::Directory;

/// What the backend is started with.
pub struct Config {
    /// The relay its admissions admit to: the relay's `NCDB_RELAY_ID`.
    pub relay_id: [u8; 16],
    /// The app's admission key; the operator puts its public half in the
    /// relay's `NCDB_ADMISSION_KEYS`.
    pub admission_key: SigningKey,
    /// How many spaces an admission lets a user create: the personal
    /// space, the account space, and every project and shared note.
    pub spaces: u64,
    /// How long an admission holds. The SDK fetches a new one when the
    /// relay refuses a new space after it lapsed, so the backend withdraws
    /// a user by no longer signing for them.
    pub lifetime: Duration,
    /// The address to listen on.
    pub listen: String,
}

impl Config {
    /// The settings for `relay_id` with `admission_key`: 100 spaces, 30
    /// days, a free port of the loopback.
    pub fn new(relay_id: [u8; 16], admission_key: SigningKey) -> Config {
        Config {
            relay_id,
            admission_key,
            spaces: 100,
            lifetime: Duration::from_secs(30 * 24 * 60 * 60),
            listen: "127.0.0.1:0".to_owned(),
        }
    }
}

/// What the connections share.
pub(crate) struct State {
    config: Config,
    directory: Mutex<Directory>,
    /// The app's sessions: a token to the user's e-mail address.
    sessions: Mutex<BTreeMap<String, String>>,
}

/// The backend, serving HTTP until it is dropped.
pub struct Backend {
    address: String,
    stop: Arc<AtomicBool>,
}

impl Backend {
    /// Starts the backend on a thread of its own.
    pub fn start(config: Config) -> io::Result<Backend> {
        let listener = TcpListener::bind(&config.listen)?;
        let address = listener.local_addr()?.to_string();
        let stop = Arc::new(AtomicBool::new(false));
        let state = Arc::new(State {
            config,
            directory: Mutex::default(),
            sessions: Mutex::default(),
        });
        let stopped = stop.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                if stopped.load(Ordering::SeqCst) {
                    break;
                }
                let Ok(stream) = stream else { continue };
                let state = state.clone();
                std::thread::spawn(move || {
                    let _ = http::serve(stream, &state);
                });
            }
        });
        Ok(Backend { address, stop })
    }

    /// The address it listens on, such as `127.0.0.1:8080`.
    pub fn address(&self) -> String {
        self.address.clone()
    }

    /// Serves until the process ends.
    pub fn wait(self) -> ! {
        loop {
            std::thread::park();
        }
    }
}

impl Drop for Backend {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // Wakes the accepting thread, which then sees the flag.
        let _ = TcpStream::connect(&self.address);
    }
}
