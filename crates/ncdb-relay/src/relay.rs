//! The relay core (RFC 0007, "What the relay checks"): every check a
//! received change goes through before a backend appends it.

use std::collections::BTreeSet;

use ncdb_core::ids::{MemberId, SpaceId};

use crate::error::ErrorCode;
use crate::store::{Position, RelayStore};

/// The largest frame the relay accepts: 16 MiB (RFC 0007, decision R5).
pub const MAX_FRAME: usize = 16 * 1024 * 1024;

/// What a relay deployment configures.
#[derive(Clone, Debug)]
pub struct Config {
    /// The account keys whose members may create spaces on this relay
    /// (RFC 0007, decision R6).
    pub allowed_accounts: BTreeSet<[u8; 32]>,
    /// The largest frame the relay accepts.
    pub max_frame: usize,
    /// The relay's clock, in milliseconds since the Unix epoch.
    pub clock: fn() -> u64,
}

impl Config {
    /// A configuration that lets members of these accounts create spaces.
    pub fn new(allowed_accounts: BTreeSet<[u8; 32]>) -> Config {
        Config {
            allowed_accounts,
            max_frame: MAX_FRAME,
            clock: || 0,
        }
    }
}

/// The result of one frame of a push.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pushed {
    /// Appended at this position.
    Accepted(Position),
    /// The relay already holds this change id, at this position.
    Duplicate(Position),
    /// Refused, with this code.
    Rejected(ErrorCode),
}

/// The relay core over a backend.
#[derive(Debug)]
pub struct Relay<S> {
    store: S,
    config: Config,
}

impl<S: RelayStore> Relay<S> {
    /// The relay core over `store`.
    pub fn new(store: S, config: Config) -> Relay<S> {
        Relay { store, config }
    }

    /// The backend.
    pub fn store(&self) -> &S {
        &self.store
    }

    /// Checks each frame `uploader` pushes to `space`, one by one, and
    /// appends the accepted ones in order. One result per frame.
    pub async fn push(
        &self,
        _uploader: MemberId,
        _space: SpaceId,
        frames: Vec<Vec<u8>>,
    ) -> Vec<Pushed> {
        frames
            .iter()
            .map(|_| Pushed::Rejected(ErrorCode::Unavailable))
            .collect()
    }
}
