//! Opening a member's database (RFC 0009, "Open and close"), and what a
//! database holds while it is open: the local view, the member's keys and
//! each space it writes into.
//!
//! **What this does not do yet.** The database lives in memory and its
//! keys with it: nothing persists after the [`Db`] is dropped. RFC 0009
//! gives no place on disk natively, and the platform keystore that RFC 0003
//! puts the member key in is a later ticket. The account key that signs the
//! member's certificate is drawn on open too, until the accounts RFC
//! designs accounts. There is no relay: the database stays local.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};

use ed25519_dalek::SigningKey;
use ncdb_core::checks::SealChecks;
use ncdb_core::clock::Clock;
use ncdb_core::ids::{ChangeId, SpaceId as CoreSpaceId};
use ncdb_core::keys::{MemberKey, SpaceKey};
use ncdb_core::model_lock::ModelLock;
use ncdb_core::view::{Model, View};
use zeroize::Zeroizing;

use crate::error::{Error, ErrorCode, storage};
use crate::space::Spaces;

/// The options of [`Db::open`]: the text of the app's `model.lock.json`.
#[derive(Clone)]
pub struct OpenOptions {
    lock: String,
}

impl OpenOptions {
    /// Options to open a database from the text of `model.lock.json`.
    pub fn new(lock: &str) -> OpenOptions {
        OpenOptions {
            lock: lock.to_owned(),
        }
    }
}

impl std::fmt::Debug for OpenOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenOptions").finish_non_exhaustive()
    }
}

/// One member's copy of all its spaces on this device.
#[derive(Clone)]
pub struct Db {
    pub(crate) inner: Arc<Mutex<Inner>>,
}

impl std::fmt::Debug for Db {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Db").finish_non_exhaustive()
    }
}

/// The secret halves of the member key. The core's key cannot be copied,
/// and the checks of each space hold one, so the SDK keeps the secrets and
/// makes a key from them when it needs one.
pub(crate) struct Secrets {
    signing: Zeroizing<[u8; 32]>,
    agreement: Zeroizing<[u8; 32]>,
}

impl Secrets {
    fn generate() -> Secrets {
        Secrets {
            signing: random(),
            agreement: random(),
        }
    }

    /// The member key.
    pub(crate) fn key(&self) -> MemberKey {
        MemberKey::from_secrets(*self.signing, *self.agreement)
    }
}

/// 32 bytes from the platform's secure generator.
///
/// # Panics
///
/// When the platform has no secure generator; a key from a weaker source
/// is no key.
pub(crate) fn random() -> Zeroizing<[u8; 32]> {
    let mut bytes = Zeroizing::new([0; 32]);
    getrandom::fill(bytes.as_mut()).expect("the platform's secure random generator failed");
    bytes
}

/// What an open database holds.
pub(crate) struct Inner {
    pub(crate) view: View,
    pub(crate) model: Arc<Model>,
    pub(crate) model_id: [u8; 16],
    pub(crate) member: Secrets,
    pub(crate) account: SigningKey,
    pub(crate) spaces: BTreeMap<CoreSpaceId, SpaceState>,
}

/// What the member needs to write into one space: its checks, the space
/// key it writes under, its next author sequence, its clock and its heads.
pub(crate) struct SpaceState {
    pub(crate) checks: SealChecks,
    pub(crate) key: SpaceKey,
    pub(crate) generation: u64,
    pub(crate) sequence: u64,
    pub(crate) clock: Clock,
    pub(crate) heads: ChangeId,
}

impl Db {
    /// Opens the member's database from the app's `model.lock.json`, and
    /// creates the member key. A lock that cannot be read, or breaks the
    /// rules RFC 0005 gives a lock, is refused with `ModelRefused`.
    pub async fn open(options: OpenOptions) -> Result<Db, Error> {
        let lock = ModelLock::from_json(&options.lock).map_err(|error| {
            Error::new(
                ErrorCode::ModelRefused,
                format!("the lock is refused: {error}"),
            )
        })?;
        let view = View::open_in_memory(&lock).map_err(|error| match error {
            ncdb_core::view::Error::Invalid(invalid) => Error::new(
                ErrorCode::ModelRefused,
                format!("the lock is refused: {invalid:?}"),
            ),
            error => storage(error),
        })?;
        let model = Arc::new(view.model().clone());
        let inner = Inner {
            view,
            model,
            model_id: lock.model.id.0,
            member: Secrets::generate(),
            account: SigningKey::from_bytes(&random()),
            spaces: BTreeMap::new(),
        };
        Ok(Db {
            inner: Arc::new(Mutex::new(inner)),
        })
    }

    /// The spaces of the member.
    pub fn spaces(&self) -> Spaces<'_> {
        Spaces { db: self }
    }

    pub(crate) fn lock(&self) -> MutexGuard<'_, Inner> {
        lock(&self.inner)
    }
}

/// The database's state. A panic while it was held leaves it as the last
/// SQLite transaction left it, so the lock is taken back.
pub(crate) fn lock(inner: &Mutex<Inner>) -> MutexGuard<'_, Inner> {
    inner
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
