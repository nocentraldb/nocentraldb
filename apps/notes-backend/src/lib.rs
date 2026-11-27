//! An example backend of a notes and projects app with open signup, as
//! [the guide on spaces and invitations](../../docs/spaces-and-invitations.md)
//! describes it: it signs admissions for its users (RFC 0015, "Admission
//! tokens") and keeps a directory that maps a user to their account's
//! identity and their devices' invitations, with the invitations that wait
//! for a user who has no account yet.

pub mod admission;

use std::io;

use ed25519_dalek::SigningKey;

/// What the backend is started with.
pub struct Config {
    /// The relay its admissions admit to: `NCDB_RELAY_ID` of the relay.
    pub relay_id: [u8; 16],
    /// The app's admission key; its public half is in the relay's
    /// `NCDB_ADMISSION_KEYS`.
    pub admission_key: SigningKey,
}

impl Config {
    /// The settings for `relay_id` with `admission_key`.
    pub fn new(relay_id: [u8; 16], admission_key: SigningKey) -> Config {
        Config {
            relay_id,
            admission_key,
        }
    }
}

/// The backend, serving HTTP on the loopback.
pub struct Backend;

impl Backend {
    /// Starts the backend.
    pub fn start(_config: Config) -> io::Result<Backend> {
        Err(io::Error::other("not built yet"))
    }

    /// The address it listens on.
    pub fn address(&self) -> String {
        String::new()
    }
}
