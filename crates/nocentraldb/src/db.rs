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

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use ncdb_core::change::Kind;
use ncdb_core::checks::SealChecks;
use ncdb_core::clock::Clock;
use ncdb_core::ids::SpaceId as CoreSpaceId;
use ncdb_core::keys::{MemberKey, PersonKey};
use ncdb_core::log::{Position, State};
use ncdb_core::membership::MemberKind;
use ncdb_core::model_lock::ModelLock;
use ncdb_core::plugins::Registry;
use ncdb_core::receive::Checks;
use ncdb_core::view::{Model, View};
use ncdb_plugin::Plugin;

use crate::durability::Storage;
use crate::error::{Error, ErrorCode, storage};
use crate::keys::{self, Keys, SecretStore, Secrets};
use crate::space::Spaces;
use crate::subscribe::Watcher;
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
    /// Whether a new database waits to be paired with a device of an
    /// account (RFC 0010).
    pairing: bool,
    /// The plugins whose field types the core folds (RFC 0011).
    plugins: Vec<Plugin>,
    /// The name of a database on OPFS, in a dedicated worker (NOC-175):
    /// crate-internal until the browser export names it (NOC-176).
    #[cfg(target_arch = "wasm32")]
    browser: Option<String>,
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
            pairing: false,
            plugins: Vec::new(),
            #[cfg(target_arch = "wasm32")]
            browser: None,
        }
    }

    /// Opens a new database for pairing (RFC 0009, "Accounts and devices";
    /// RFC 0010, "Adding a device or an agent to an account"): it draws its
    /// member key and no account, and its invitation carries no
    /// certificate yet, which only [`Account::pair`](crate::Account::pair)
    /// on a device of the account accepts. [`Account::complete_pairing`](crate::Account::complete_pairing)
    /// then makes it a device or, with [`agent`](OpenOptions::agent), an
    /// agent of that account. A database that exists keeps the account it
    /// has.
    pub fn pairing(mut self) -> OpenOptions {
        self.pairing = true;
        self
    }

    /// The keys of a new database: a member key and no account when it
    /// waits to be paired, a new account and its recovery code otherwise.
    fn new_keys(&self) -> Keys {
        match self.pairing {
            true => Keys::pairing(),
            false => Keys::generate(),
        }
    }

    /// Opens the database named `name` on OPFS, in a dedicated worker
    /// (RFC 0006, "The browser"; NOC-231): crate-internal until the
    /// browser export names it (NOC-176).
    #[cfg(target_arch = "wasm32")]
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn browser(mut self, name: &str) -> OpenOptions {
        self.browser = Some(name.to_owned());
        self
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

    /// Hands `plugin`'s field types to the core, as a plugin's crate
    /// exports them from `plugin()`; call it once per plugin (RFC 0011,
    /// "Handing plugins to the core"). A plugin field of the lock folds
    /// through its code; one whose type and rule no plugin given here
    /// implements parks, and a write to it is refused with `UnknownType`.
    /// A plugin the lock does not name is ignored. [`Db::open`] refuses two
    /// plugins of one namespace, or a plugin of the namespace `ncdb`, with
    /// `InvalidValue`.
    pub fn plugin(mut self, plugin: Plugin) -> OpenOptions {
        self.plugins.push(plugin);
        self
    }

    /// The one relay the database syncs with (RFC 0009, decision S16), by
    /// its URL: `wss://`, WebSocket over TLS verified against the
    /// platform's trust store, or `ws://` on a local network. [`Db::open`]
    /// refuses any other URL with `InvalidValue`. In a browser, the relay
    /// URL is `wss://`, except for localhost while developing: a page
    /// served over `https://` cannot open `ws://` (RFC 0009, NOC-245).
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
    /// What the database holds of its account: the account key, an
    /// agent's certificate, or nothing while it waits to be paired.
    pub(crate) account: keys::Account,
    /// The recovery code, until the person confirmed they stored it.
    pub(crate) code: Option<ncdb_core::recovery::RecoveryCode>,
    /// Where the key record lives, to keep it again when it changes.
    pub(crate) home: keys::Home,
    /// What the account certifies the member as: a device or an agent.
    pub(crate) kind: MemberKind,
    /// Where the copy is stored, for the durability report.
    pub(crate) storage: crate::durability::Storage,
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
    /// The person keys of the space the member holds, by person-key id,
    /// each from a record whose signature, wrap and commitment held
    /// (`crate::sync::person_keys`). In memory only: a reopened database
    /// fetches them again from the relay.
    pub(crate) person_keys: BTreeMap<[u8; 16], PersonKey>,
    /// Whether the relay's records of the space were fetched in the
    /// current connection. Read by the session.
    pub(crate) person_keys_fetched: bool,
    /// The account keys of the writers whose records failed their checks
    /// in the last fetch, though they signed them
    /// (`crate::sync::person_keys`). In memory only, as the person keys.
    pub(crate) bad_person_key_records: BTreeSet<[u8; 32]>,
    /// The ids of the person keys the view opens the space's personal
    /// fields with (`crate::personal::refresh`). In memory only.
    pub(crate) opened: BTreeSet<[u8; 16]>,
}

impl SpaceState {
    /// Drops every person key the space's state names as destroyed: a
    /// shred is final when accepted, and a provisional one once an accepted
    /// erasure decision finalizes it, so the member destroys its copy then
    /// (RFC 0008, "A shred is final when accepted"). A provisional shred
    /// alone keeps the key. A destroyed key is never held again, since its
    /// records are not accepted (`crate::sync::person_keys`).
    pub(crate) fn drop_shredded(&mut self) {
        let resolved = self.checks.resolved();
        self.person_keys
            .retain(|id, _| resolved.person_key(*id).is_none_or(|key| !key.destroyed()));
    }

    /// A space the database holds no change of yet.
    pub(crate) fn new(space: CoreSpaceId, member: MemberKey) -> SpaceState {
        SpaceState {
            checks: SealChecks::new(space, member),
            clock: Clock::new(),
            person_keys: BTreeMap::new(),
            person_keys_fetched: false,
            bad_person_key_records: BTreeSet::new(),
            opened: BTreeSet::new(),
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
    /// A database at a name on OPFS, which `Db::open` prepares first.
    #[cfg(target_arch = "wasm32")]
    Browser {
        name: String,
    },
}

impl OpenOptions {
    /// The place the options name, or `InvalidValue` when they name none,
    /// or memory together with a path or a key file (RFC 0009, S27).
    fn place(&self) -> Result<Place, Error> {
        let invalid = |message: &str| Err(Error::new(ErrorCode::InvalidValue, message));
        #[cfg(target_arch = "wasm32")]
        if let Some(name) = &self.browser {
            if self.path.is_some() || self.in_memory || self.key_file.is_some() {
                return invalid(
                    "the options name a browser database together with a path, a key file or in_memory(); name one place",
                );
            }
            return Ok(Place::Browser { name: name.clone() });
        }
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

/// A browser database `Db::open` prepared; none exists natively.
#[cfg(target_arch = "wasm32")]
type Placed = crate::browser::Placed;
/// A browser database `Db::open` prepared; none exists natively.
#[cfg(not(target_arch = "wasm32"))]
enum Placed {}

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
    registry: &Registry,
    new: impl FnOnce() -> Keys,
) -> Result<(View, Keys, keys::Home), Error> {
    let store = match key_file {
        Some(_) => None,
        None => Some(store().map_err(keys::store::unavailable)?),
    };
    std::fs::create_dir_all(&dir)
        .map_err(|error| keys::io("the database directory cannot be created", &error))?;
    let file = dir.join(DATABASE_FILE);
    let existed = exists(&file)?;
    let (keys, home) = match (key_file, store) {
        (Some(key_file), _) => (
            keys::from_file(&key_file, existed, new)?,
            keys::Home::File(key_file),
        ),
        (None, Some(store)) => {
            let keys = keys::from_store(store.as_ref(), &dir, existed, new)?;
            let user = keys::store::user(&dir)?;
            let store = std::sync::Arc::from(store);
            (keys, keys::Home::Store { store, user })
        }
        (None, None) => unreachable!("a database without a key file has a keystore"),
    };
    // The view keeps the checks while a rebuild runs (NOC-156).
    let member = keys.member.clone();
    let checks = move |space| SealChecks::new(space, member.key());
    let view = View::open_with_plugins(&file, lock, registry, checks).map_err(view_error)?;
    Ok((view, keys, home))
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
    /// returns, so that what the old model parked is read. A relay URL that is neither `wss://` nor
    /// `ws://` fails with `InvalidValue`. An existing database whose keys are gone fails with
    /// `StorageFailed`. With a relay, the database starts syncing in the
    /// background.
    pub async fn open(options: OpenOptions) -> Result<Db, Error> {
        #[cfg(target_arch = "wasm32")]
        if let Place::Browser { name } = options.place()? {
            let placed = crate::browser::place(&name, || options.new_keys()).await?;
            return Db::open_placed(options, None, &keys::store::platform, Some(placed));
        }
        Db::open_with(options, None, &keys::store::platform)
    }

    /// Opens the database with keys from `store` in place of the
    /// platform's keystore, as the keystore's tests need it.
    #[cfg(all(test, not(target_arch = "wasm32")))]
    pub(crate) fn open_with_store(options: OpenOptions, store: &Store<'_>) -> Result<Db, Error> {
        Db::open_with(options, None, store)
    }

    /// Opens the database of the member with these secrets, of this
    /// account, as the sync tests need it. Its recovery code derives from
    /// the account key, so that databases opened with one account key are
    /// one account.
    #[cfg(test)]
    pub(crate) async fn open_as(
        options: OpenOptions,
        signing: [u8; 32],
        agreement: [u8; 32],
        account: ed25519_dalek::SigningKey,
    ) -> Result<Db, Error> {
        let keys = Keys::of_account(Secrets::new(signing, agreement), account);
        Db::open_with(options, Some(keys), &keys::store::platform)
    }

    fn open_with(
        options: OpenOptions,
        given: Option<Keys>,
        store: &Store<'_>,
    ) -> Result<Db, Error> {
        Db::open_placed(options, given, store, None)
    }

    /// Opens the database at the place the options name; a browser
    /// database comes `placed` already, since preparing it waits.
    fn open_placed(
        options: OpenOptions,
        given: Option<Keys>,
        store: &Store<'_>,
        placed: Option<Placed>,
    ) -> Result<Db, Error> {
        let place = options.place()?;
        if let Some(url) = &options.relay
            && !url.starts_with("wss://")
            && !url.starts_with("ws://")
        {
            return Err(Error::new(
                ErrorCode::InvalidValue,
                "the relay URL must start with wss:// or ws://",
            ));
        }
        let lock = ModelLock::from_json(&options.lock).map_err(|error| {
            Error::new(
                ErrorCode::ModelRefused,
                format!("the lock is refused: {error}"),
            )
        })?;
        let registry = Registry::new(options.plugins.iter().cloned())
            .map_err(|error| Error::new(ErrorCode::InvalidValue, error.to_string()))?;
        let _ = &placed;
        let new = || options.new_keys();
        let (mut view, keys, storage, home) = match place {
            Place::Memory => {
                let view =
                    View::open_in_memory_with_plugins(&lock, &registry).map_err(view_error)?;
                let keys = given.unwrap_or_else(new);
                (view, keys, Storage::Temporary, keys::Home::Memory)
            }
            Place::Path { dir, key_file } => {
                let (view, keys, home) = open_at(dir, key_file, store, &lock, &registry, new)?;
                (view, keys, Storage::Persistent, home)
            }
            #[cfg(target_arch = "wasm32")]
            Place::Browser { .. } => {
                let placed = placed.expect("Db::open prepares a browser database first");
                let member = placed.keys.member.clone();
                let checks = move |space| SealChecks::new(space, member.key());
                let view =
                    View::open_with_plugins(Path::new(&placed.path), &lock, &registry, checks)
                        .map_err(view_error)?;
                (
                    view,
                    placed.keys,
                    placed.storage,
                    keys::Home::Browser(placed.name),
                )
            }
        };
        let model = Arc::new(view.model().clone());
        let spaces: BTreeMap<_, _> = view
            .spaces()
            .map(|space| (space, SpaceState::from_log(&view, space, keys.member.key())))
            .collect();
        // A shred held before this device kept when (NOC-218) starts its
        // age now.
        for (space, state) in &spaces {
            crate::status::note_shreds(&mut view, *space, state.checks.resolved())
                .map_err(view_error)?;
        }
        let inner = Inner {
            view,
            model,
            model_id: lock.model.id.0,
            member: keys.member,
            account: keys.account,
            code: keys.code,
            home,
            kind: match options.agent {
                true => MemberKind::Agent,
                false => MemberKind::Device,
            },
            storage,
            spaces,
            watchers: Vec::new(),
            sync: None,
            status: crate::sync::Notify::default(),
        };
        let inner = Arc::new(Mutex::new(inner));
        // A rebuild of the view runs in the background (RFC 0009, step 4),
        // in a thread, which wasm32 does not start yet.
        #[cfg(not(target_arch = "wasm32"))]
        crate::view_report::start(&inner);
        if let Some(url) = options.relay {
            sync::start(&inner, url);
        }
        Ok(Db { inner })
    }

    /// What this copy would lose if it were deleted now (RFC 0006, "What
    /// the SDK reports"): crate-internal until NOC-176 names it.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn durability(&self) -> crate::durability::Durability {
        let inner = lock(&self.inner);
        crate::durability::durability(&inner, inner.storage)
    }

    /// The account of this database (RFC 0009, "Accounts and devices"):
    /// its recovery code until the person stored it, and pairing.
    pub fn account(&self) -> crate::Account<'_> {
        crate::Account { db: self }
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
