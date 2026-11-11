//! Opening a member's database (RFC 0009, "Open and close"), and what a
//! database holds while it is open: the local view, the member's keys and
//! each space it writes into.
//!
//! A database lives in a directory the app names with
//! [`OpenOptions::path`], or in memory with [`OpenOptions::in_memory`].
//! Its keys live with it in memory, or in the key file the app names with
//! [`OpenOptions::key_file`] (`crate::keys`). Reopening a database
//! rebuilds what it keeps of each space from the space's log. A database
//! opened with a relay syncs with it; see `crate::sync`.
//!
//! **What this does not do yet.** The account key that signs the member's
//! certificate is drawn per database, until the accounts RFC designs
//! accounts (RFC 0003, decision 17).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use ed25519_dalek::SigningKey;
use ncdb_core::change::Kind;
use ncdb_core::checks::SealChecks;
use ncdb_core::clock::Clock;
use ncdb_core::ids::SpaceId as CoreSpaceId;
use ncdb_core::keys::MemberKey;
use ncdb_core::log::{Position, State};
use ncdb_core::membership::MemberKind;
use ncdb_core::model_lock::ModelLock;
use ncdb_core::receive::Checks;
use ncdb_core::view::{Model, View};

use crate::error::{Error, ErrorCode, storage};
use crate::keys::{self, Keys, SecretStore, Secrets};
use crate::space::Spaces;
use crate::subscribe::Watcher;
#[cfg(not(target_arch = "wasm32"))]
use crate::sync;
use crate::sync::SyncState;

/// The options of [`Db::open`]: the text of the app's `model.lock.json`,
/// where the database lives, and the relay to sync with.
#[derive(Clone)]
pub struct OpenOptions {
    lock: String,
    relay: Option<String>,
    in_memory: bool,
    path: Option<PathBuf>,
    key_file: Option<PathBuf>,
    agent: bool,
}

/// The name of the database's SQLite file in the directory of
/// [`OpenOptions::path`].
const DATABASE_FILE: &str = "nocentraldb.sqlite";

impl OpenOptions {
    /// Options to open a database from the text of `model.lock.json`.
    /// [`Db::open`] refuses them with `InvalidValue` until they name where
    /// the database lives, with [`path`](OpenOptions::path) or
    /// [`in_memory`](OpenOptions::in_memory) (RFC 0009, S27). Without [`relay`](OpenOptions::relay), the database
    /// stays local.
    pub fn new(lock: &str) -> OpenOptions {
        OpenOptions {
            lock: lock.to_owned(),
            relay: None,
            in_memory: false,
            path: None,
            key_file: None,
            agent: false,
        }
    }

    /// Marks the member as an agent in the certificate its account signs,
    /// so a space adds it as a writer when no role is given (RFC 0009,
    /// S20; RFC 0008, M14). Without it, the member is a device.
    pub fn agent(mut self) -> OpenOptions {
        self.agent = true;
        self
    }

    /// The directory the database lives in, which the SDK owns and creates
    /// when it does not exist (RFC 0009, S19).
    pub fn path(mut self, dir: impl Into<PathBuf>) -> OpenOptions {
        self.path = Some(dir.into());
        self
    }

    /// Keeps the member key and the account key in the file at `path`
    /// instead of the platform keystore, for a headless agent that has
    /// none (RFC 0009, S19). Only the file's permissions protect it:
    /// whoever reads the file, or a backup of it, holds the member key. On
    /// Unix the SDK creates it readable by its user alone. It takes a
    /// [`path`](OpenOptions::path) for the database.
    pub fn key_file(mut self, path: impl Into<PathBuf>) -> OpenOptions {
        self.key_file = Some(path.into());
        self
    }

    /// Keeps the database and its keys in memory, for tests: they are gone
    /// when the [`Db`] is dropped (RFC 0009, S19).
    pub fn in_memory(mut self) -> OpenOptions {
        self.in_memory = true;
        self
    }

    /// The one relay the database syncs with (RFC 0009, decision S16), by
    /// its URL. Only plain WebSocket, `ws://`, is built yet; [`Db::open`]
    /// refuses any other URL with `InvalidValue`.
    pub fn relay(mut self, url: &str) -> OpenOptions {
        self.relay = Some(url.to_owned());
        self
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

/// What an open database holds.
pub(crate) struct Inner {
    pub(crate) view: View,
    pub(crate) model: Arc<Model>,
    pub(crate) model_id: [u8; 16],
    pub(crate) member: Secrets,
    pub(crate) account: SigningKey,
    /// What the account certifies the member as: a device or an agent.
    pub(crate) kind: MemberKind,
    /// Each space the member created or was added to.
    pub(crate) spaces: BTreeMap<CoreSpaceId, SpaceState>,
    pub(crate) watchers: Vec<Watcher>,
    /// What the database keeps of its relay, when it syncs with one.
    pub(crate) sync: Option<SyncState>,
    /// The followers of the sync status.
    pub(crate) status: crate::sync::Notify,
}

/// What the database keeps of one space besides its log: the checks,
/// which hold the space keys its envelopes gave the member, and the
/// member's clock in the space. The rest of what a write needs comes from
/// the log (`crate::next`).
pub(crate) struct SpaceState {
    pub(crate) checks: SealChecks,
    pub(crate) clock: Clock,
}

impl SpaceState {
    /// A space the database holds no change of yet.
    pub(crate) fn new(space: CoreSpaceId, member: MemberKey) -> SpaceState {
        SpaceState {
            checks: SealChecks::new(space, member),
            clock: Clock::new(),
        }
    }

    /// A space whose log `view` holds, as a reopened database finds it:
    /// its checks learn every folded control change of the log, and with
    /// them the membership and the space keys the envelopes give the
    /// member. The clock observes the log on the next write (`crate::next`).
    pub(crate) fn from_log(view: &View, space: CoreSpaceId, member: MemberKey) -> SpaceState {
        let mut state = SpaceState::new(space, member);
        if let Some(log) = view.log(space) {
            for (_, change) in log.after(Position::START) {
                let folded = matches!(log.state(change.id()), Some(State::Folded { .. }));
                if folded && change.header().kind == Kind::CONTROL {
                    state.checks.add_control(change);
                }
            }
        }
        state
    }
}

/// Where a database lives, as its options name it.
enum Place {
    Memory,
    Path {
        dir: PathBuf,
        key_file: Option<PathBuf>,
    },
}

impl OpenOptions {
    /// The place the options name, or `InvalidValue` when they name none,
    /// or memory together with a path or a key file (RFC 0009, S27).
    fn place(&self) -> Result<Place, Error> {
        let invalid = |message: &str| Err(Error::new(ErrorCode::InvalidValue, message));
        match (&self.path, self.in_memory) {
            (Some(_), true) => invalid("the options name both a path and in_memory(); name one"),
            (None, true) if self.key_file.is_some() => invalid(
                "the options name both a key file and in_memory(); keys in memory need no file",
            ),
            (None, true) => Ok(Place::Memory),
            (None, false) => {
                invalid("the options name no place for the database: call path(dir) or in_memory()")
            }
            (Some(dir), false) => Ok(Place::Path {
                dir: dir.clone(),
                key_file: self.key_file.clone(),
            }),
        }
    }
}

/// What gives the keystore a database at a path keeps its keys in: the
/// platform's, or a test's.
type Store<'a> = dyn Fn() -> Result<Box<dyn SecretStore>, keyring_core::Error> + 'a;

/// Opens the view and the keys of the database in `dir`, creating the
/// directory, the database and the keys on first open. The keys come from
/// the key file when the app names one, and from the keystore otherwise;
/// without a keystore, nothing is created.
fn open_at(
    dir: PathBuf,
    key_file: Option<PathBuf>,
    store: &Store<'_>,
    lock: &ModelLock,
) -> Result<(View, Keys), Error> {
    let store = match key_file {
        Some(_) => None,
        None => Some(store().map_err(keys::store::unavailable)?),
    };
    std::fs::create_dir_all(&dir)
        .map_err(|error| keys::io("the database directory cannot be created", &error))?;
    let file = dir.join(DATABASE_FILE);
    let existed = exists(&file)?;
    let keys = match (&key_file, &store) {
        (Some(key_file), _) => keys::from_file(key_file, existed)?,
        (None, Some(store)) => keys::from_store(store.as_ref(), &dir, existed)?,
        (None, None) => unreachable!("a database without a key file has a keystore"),
    };
    let checks = |space| SealChecks::new(space, keys.member.key());
    let view = View::open_with(&file, lock, checks).map_err(view_error)?;
    Ok((view, keys))
}

/// Whether the database file exists.
fn exists(file: &Path) -> Result<bool, Error> {
    file.try_exists()
        .map_err(|error| keys::io("the database file cannot be found", &error))
}

/// A view that does not open: a lock it cannot fold, one that is not a
/// successor of the lock the database was last opened with (RFC 0005,
/// D11), naming each refused change, or a database that holds no lock to
/// check it against and was folded by another model, is `ModelRefused`;
/// any other failure a failure of storage.
fn view_error(error: ncdb_core::view::Error) -> Error {
    match error {
        ncdb_core::view::Error::Invalid(invalid) => Error::new(
            ErrorCode::ModelRefused,
            format!("the lock is refused: {invalid:?}"),
        ),
        error @ ncdb_core::view::Error::Refused(_) => {
            Error::new(ErrorCode::ModelRefused, error.to_string())
        }
        ncdb_core::view::Error::Model => Error::new(
            ErrorCode::ModelRefused,
            "the database was folded by another model than the lock's, \
             and holds no lock to check this one against",
        ),
        error => storage(error),
    }
}

impl Db {
    /// Opens the member's database from the app's `model.lock.json`, at
    /// the place the options name, and creates the member key on first
    /// open. Options that name no place, or both memory and a path or a
    /// key file, are refused with `InvalidValue`; a lock that cannot be
    /// read, or breaks the rules RFC 0005 gives a lock, with
    /// `ModelRefused`. An existing database checks the lock against the one
    /// it was last opened with (RFC 0005, D11): a lock of another
    /// `model.id`, or a change that is not additive, is `ModelRefused`,
    /// naming each refused change, and nothing changes; an additive change
    /// folds the database's log again by the new model before `open`
    /// returns, so that what the old model parked is read. A relay URL that is not `ws://`, with
    /// `InvalidValue`. An existing database whose keys are gone fails with
    /// `StorageFailed`. With a relay, the database starts syncing in the
    /// background.
    pub async fn open(options: OpenOptions) -> Result<Db, Error> {
        Db::open_with(options, None, &keys::store::platform)
    }

    /// Opens the database with keys from `store` in place of the
    /// platform's keystore, as the keystore's tests need it.
    #[cfg(all(test, not(target_arch = "wasm32")))]
    pub(crate) fn open_with_store(options: OpenOptions, store: &Store<'_>) -> Result<Db, Error> {
        Db::open_with(options, None, store)
    }

    /// Opens the database of the member with these secrets, of this
    /// account, as the sync tests need it.
    #[cfg(test)]
    pub(crate) async fn open_as(
        options: OpenOptions,
        signing: [u8; 32],
        agreement: [u8; 32],
        account: SigningKey,
    ) -> Result<Db, Error> {
        let keys = Keys {
            member: Secrets::new(signing, agreement),
            account,
        };
        Db::open_with(options, Some(keys), &keys::store::platform)
    }

    fn open_with(
        options: OpenOptions,
        given: Option<Keys>,
        store: &Store<'_>,
    ) -> Result<Db, Error> {
        let place = options.place()?;
        #[cfg(target_arch = "wasm32")]
        if options.relay.is_some() {
            return Err(Error::new(
                ErrorCode::InvalidValue,
                "syncing with a relay is not built on wasm32 yet",
            ));
        }
        if let Some(url) = &options.relay
            && !url.starts_with("ws://")
        {
            return Err(Error::new(
                ErrorCode::InvalidValue,
                "the relay URL must start with ws://; TLS is not built yet",
            ));
        }
        let lock = ModelLock::from_json(&options.lock).map_err(|error| {
            Error::new(
                ErrorCode::ModelRefused,
                format!("the lock is refused: {error}"),
            )
        })?;
        let (view, keys) = match place {
            Place::Memory => {
                let view = View::open_in_memory(&lock).map_err(view_error)?;
                (view, given.unwrap_or_else(Keys::generate))
            }
            Place::Path { dir, key_file } => open_at(dir, key_file, store, &lock)?,
        };
        let model = Arc::new(view.model().clone());
        let spaces = view
            .spaces()
            .map(|space| (space, SpaceState::from_log(&view, space, keys.member.key())))
            .collect();
        let inner = Inner {
            view,
            model,
            model_id: lock.model.id.0,
            member: keys.member,
            account: keys.account,
            kind: match options.agent {
                true => MemberKind::Agent,
                false => MemberKind::Device,
            },
            spaces,
            watchers: Vec::new(),
            sync: None,
            status: crate::sync::Notify::default(),
        };
        let inner = Arc::new(Mutex::new(inner));
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(url) = options.relay {
            sync::start(&inner, url);
        }
        Ok(Db { inner })
    }

    /// The spaces of the member.
    pub fn spaces(&self) -> Spaces<'_> {
        Spaces { db: self }
    }

    /// The invitation of this database's member: its public identity,
    /// which an admin of a space adds with [`Members::add`](crate::Members::add)
    /// (RFC 0009, "Invitations and members").
    pub async fn invitation(&self) -> crate::Invitation {
        crate::Invitation::of(&self.lock())
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
