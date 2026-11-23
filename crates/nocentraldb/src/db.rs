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
//! accounts (RFC 0003, "The account key", **Until RFC 0010 is built.**).

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
use ncdb_core::view::{Model, View};

use crate::durability::Storage;
use crate::error::{Error, ErrorCode, storage};
use crate::keys::{self, Keys, SecretStore, Secrets};
use crate::space::Spaces;
use crate::subscribe::Watcher;
use crate::sync;
use crate::sync::SyncState;

pub use crate::options::OpenOptions;
use crate::options::Place;

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
    /// `AccountRecovered`, when this open recovered the account.
    pub(crate) recovered: Option<crate::members::Notice>,
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
    /// Whether the database joins an account another device created: it
    /// then creates no account space (`crate::account`).
    pub(crate) joins_account: bool,
    /// The app's function that fetches an admission (RFC 0015).
    pub(crate) admission: Option<crate::admission::Fetch>,
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
    /// member, all at once, so the checks resolve them once (NOC-438). The
    /// clock observes the log on the next write (`crate::next`).
    pub(crate) fn from_log(view: &View, space: CoreSpaceId, member: MemberKey) -> SpaceState {
        let mut state = SpaceState::new(space, member);
        if let Some(log) = view.log(space) {
            let controls = log.after(Position::START).filter(|(_, change)| {
                change.header().kind == Kind::CONTROL
                    && matches!(log.state(change.id()), Some(State::Folded { .. }))
            });
            state
                .checks
                .add_controls(controls.map(|(_, change)| change));
        }
        state
    }
}

/// The name of the database's SQLite file in the directory of
/// [`OpenOptions::path`].
const DATABASE_FILE: &str = "nocentraldb.sqlite";

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
        .map_err(|error| keys::io(crate::texts::part::DATABASE_DIRECTORY_UNCREATABLE, &error))?;
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
        .map_err(|error| keys::io(crate::texts::part::DATABASE_FILE_NOT_FOUND, &error))
}

/// A view that does not open: a lock it cannot fold, one that is not a
/// successor of the lock the database was last opened with (RFC 0005,
/// D11), naming each refused change, a declared index whose lookup would
/// not run through it (RFC 0006, "The check"), or a database that holds no
/// lock to check it against and was folded by another model, is
/// `ModelRefused`; any other failure a failure of storage.
fn view_error(error: ncdb_core::view::Error) -> Error {
    match error {
        ncdb_core::view::Error::Invalid(invalid) => Error::new(
            ErrorCode::ModelRefused,
            crate::texts::error::lock_invalid(invalid),
        ),
        error @ (ncdb_core::view::Error::Refused(_) | ncdb_core::view::Error::Plan(_)) => {
            Error::new(ErrorCode::ModelRefused, error.to_string())
        }
        ncdb_core::view::Error::Model => Error::new(
            ErrorCode::ModelRefused,
            crate::texts::error::FOLDED_BY_ANOTHER_MODEL,
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
    /// it was last opened with (RFC 0005, "Two checks: at build and on the
    /// device"): a lock of another
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
            let code = options.recovery_code()?;
            let drawn = std::cell::Cell::new(false);
            let mut placed = crate::browser::place(&name, || {
                drawn.set(true);
                options.new_keys(code.as_ref())
            })
            .await?;
            placed.drawn = drawn.get();
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
        if let Some(crate::relay::Relay::Fixed(url)) = &options.relay
            && !crate::relay::is_relay_url(url)
        {
            return Err(Error::new(
                ErrorCode::InvalidValue,
                crate::texts::error::RELAY_URL_SCHEME,
            ));
        }
        let lock = ModelLock::from_json(&options.lock).map_err(|error| {
            Error::new(
                ErrorCode::ModelRefused,
                crate::texts::error::lock_refused(error),
            )
        })?;
        // The view folds the account space by the model the SDK ships,
        // beside the app's lock (RFC 0010, "The account space").
        let registry = Registry::new(options.plugins.iter().cloned())
            .map_err(|error| Error::new(ErrorCode::InvalidValue, error.to_string()))?
            .with_reserved_model(crate::account::lock());
        let _ = &placed;
        #[cfg(target_arch = "wasm32")]
        let placed_drawn = placed.as_ref().is_some_and(|placed| placed.drawn);
        if options.sub_agent && (options.agent || options.pairing || options.recover.is_some()) {
            return Err(Error::new(
                ErrorCode::InvalidValue,
                crate::texts::error::OPTIONS_SUB_AGENT,
            ));
        }
        let code = options.recovery_code()?;
        let drawn = std::cell::Cell::new(false);
        let new = || {
            drawn.set(true);
            options.new_keys(code.as_ref())
        };
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
        // An open with a recovery code recovers onto a new database only;
        // the keys it drew are dropped with the database that exists.
        #[cfg(target_arch = "wasm32")]
        let drawn = std::cell::Cell::new(drawn.get() || placed_drawn);
        let recovered = match (&code, drawn.get()) {
            (Some(_), false) => {
                return Err(Error::new(
                    ErrorCode::InvalidValue,
                    crate::texts::error::RECOVERY_ONTO_EXISTING,
                ));
            }
            (Some(_), true) => Some(crate::members::Notice::ACCOUNT_RECOVERED),
            (None, _) => None,
        };
        let model = Arc::new(crate::account::app_model(view.model()));
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
            recovered,
            home,
            kind: match (options.sub_agent, options.agent) {
                (true, _) => MemberKind::SubAgent,
                (false, true) => MemberKind::Agent,
                (false, false) => MemberKind::Device,
            },
            storage,
            spaces,
            watchers: Vec::new(),
            sync: None,
            status: crate::sync::Notify::default(),
            joins_account: options.joins_account,
            admission: options.admission.clone(),
        };
        let mut inner = inner;
        crate::account::reconcile(&mut inner)?;
        let inner = Arc::new(Mutex::new(inner));
        // A rebuild of the view runs in the background (RFC 0009, step 4),
        // in a thread, which wasm32 does not start yet.
        #[cfg(not(target_arch = "wasm32"))]
        crate::view_report::start(&inner);
        if let Some(relay) = options.relay {
            sync::start(&inner, relay);
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

#[cfg(test)]
mod tests {
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test as test;

    use super::*;

    /// A declared index whose lookup would not run through it is a
    /// problem of the model, not of storage (NOC-343).
    #[test]
    fn an_index_that_would_not_run_is_model_refused() {
        let error = view_error(ncdb_core::view::Error::Plan("tasks.byPriority".into()));

        assert_eq!(error.code(), ErrorCode::ModelRefused);
        assert!(error.to_string().contains("tasks.byPriority"), "{error}");
    }
}
