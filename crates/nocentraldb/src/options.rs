//! The options of [`Db::open`](crate::Db::open) (RFC 0009, "Opening a
//! database"): where the database lives, the relay, the member's kind, and
//! pairing or recovery; split out of `db.rs` by concern.

use std::path::PathBuf;

use ncdb_plugin::Plugin;

use crate::error::{Error, ErrorCode};
use crate::keys::Keys;

/// The options of [`Db::open`](crate::Db::open): the text of the app's `model.lock.json`,
/// where the database lives, and the relay to sync with.
#[derive(Clone)]
pub struct OpenOptions {
    pub(crate) lock: String,
    pub(crate) relay: Option<String>,
    pub(crate) in_memory: bool,
    pub(crate) path: Option<PathBuf>,
    pub(crate) key_file: Option<PathBuf>,
    pub(crate) agent: bool,
    /// Whether a new database waits to be paired with a device of an
    /// account (RFC 0010).
    pub(crate) pairing: bool,
    /// The words of the recovery code a new database recovers its account
    /// from (RFC 0010, "The recovery code").
    pub(crate) recover: Option<zeroize::Zeroizing<String>>,
    /// The plugins whose field types the core folds (RFC 0011).
    pub(crate) plugins: Vec<Plugin>,
    /// Whether the database joins an account another device created, as a
    /// pairing one does (RFC 0010): it creates no account space of its own.
    pub(crate) joins_account: bool,
    /// Whether the database is a sub-agent's (NOC-318).
    pub(crate) sub_agent: bool,
    /// The name of a database on OPFS, in a dedicated worker (NOC-175):
    /// crate-internal until the browser export names it (NOC-176).
    #[cfg(target_arch = "wasm32")]
    pub(crate) browser: Option<String>,
    /// The app's function that fetches an admission (RFC 0015).
    pub(crate) admission: Option<crate::admission::Fetch>,
}

impl OpenOptions {
    /// Options to open a database from the text of `model.lock.json`.
    /// [`Db::open`](crate::Db::open) refuses them with `InvalidValue` until they name where
    /// the database lives, with [`path`](OpenOptions::path) or
    /// [`in_memory`](OpenOptions::in_memory) (RFC 0009, "Open and close",
    /// **Where the database lives.**). Without [`relay`](OpenOptions::relay),
    /// the database stays local.
    pub fn new(lock: &str) -> OpenOptions {
        OpenOptions {
            lock: lock.to_owned(),
            relay: None,
            in_memory: false,
            path: None,
            key_file: None,
            agent: false,
            pairing: false,
            recover: None,
            plugins: Vec::new(),
            joins_account: false,
            sub_agent: false,
            #[cfg(target_arch = "wasm32")]
            browser: None,
            admission: None,
        }
    }

    /// The app's function that fetches an admission from its backend
    /// (RFC 0015, "In the SDK"): it gets the account's identity, the
    /// recovery key in hexadecimal that
    /// [`Account::identity`](crate::Account::identity) gives, and answers
    /// the admission's text, `ncdb-admit-1:...`, which the backend signed
    /// after it authenticated its user. When the relay refuses a new space
    /// with `not_allowed`, the SDK calls it, presents the answer to the
    /// relay, and pushes the space again, once; the next refusal calls it
    /// again. An error, or a text that is no admission, leaves the space
    /// refused as without the function, and the SDK never logs either. The
    /// developer handles a string, never a key.
    pub fn admission<F, Fut, E>(mut self, fetch: F) -> OpenOptions
    where
        F: Fn(String) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<String, E>> + 'static,
    {
        self.admission = Some(crate::admission::boxed(fetch));
        self
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
        // It joins an account another device created: a device of that
        // account adds it to the account space (RFC 0010).
        self.joins_account = true;
        self
    }

    /// Recovers an account on a new device from its recovery code (RFC
    /// 0009, "Accounts and devices"; RFC 0010, "The recovery code"): the
    /// words as the person typed them, separated by white space, in any
    /// letter case. The new database draws a new account key, with the
    /// next statement number, and certifies its member with it; the
    /// account's fingerprint stays the same, and the database is a member
    /// of nothing until other members add it. [`Db::open`](crate::Db::open) fails with
    /// `InvalidValue`, and creates nothing, when they are not twelve words
    /// of the list or their checksum fails, or when the database exists.
    pub fn recover(mut self, words: &str) -> OpenOptions {
        self.recover = Some(zeroize::Zeroizing::new(words.to_owned()));
        self
    }

    /// The recovery code the options recover from, read before anything
    /// is created: words that are no code are `InvalidValue`, naming a
    /// word's position, never the word.
    pub(crate) fn recovery_code(&self) -> Result<Option<ncdb_core::recovery::RecoveryCode>, Error> {
        let Some(words) = &self.recover else {
            return Ok(None);
        };
        ncdb_core::recovery::RecoveryCode::parse(words)
            .map(Some)
            .map_err(|why| {
                Error::new(
                    ErrorCode::InvalidValue,
                    crate::texts::error::recovery_code_unreadable(why),
                )
            })
    }

    /// The keys of a new database: those that recover the account of
    /// `recovered`, a member key and no account when it waits to be
    /// paired, a new account and its recovery code otherwise.
    pub(crate) fn new_keys(&self, recovered: Option<&ncdb_core::recovery::RecoveryCode>) -> Keys {
        match (recovered, self.pairing || self.sub_agent) {
            (Some(code), _) => Keys::recovered(code, crate::clock::now_millis() / 1000),
            (None, true) => Keys::pairing(),
            (None, false) => Keys::generate(),
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

    /// Opens the database of a sub-agent (RFC 0009, "Sub-agents and ad-hoc
    /// spaces"; RFC 0003, "Sub-agents get an ad-hoc space"): it draws its
    /// member key and holds no account, no recovery code and no account
    /// space. Its invitation is version 3 and carries no certificate; an
    /// agent's [`Members::add`](crate::Members::add) signs one with the
    /// agent's own member key, which ties it to the agent's account. It
    /// creates no space of its own: its parent adds it to the spaces it
    /// works in. [`Db::open`](crate::Db::open) refuses it with `InvalidValue` together with
    /// [`agent`](OpenOptions::agent), [`pairing`](OpenOptions::pairing) or
    /// [`recover`](OpenOptions::recover).
    pub fn sub_agent(mut self) -> OpenOptions {
        self.sub_agent = true;
        self
    }

    /// Marks the member as an agent in the certificate its account signs,
    /// so a space adds it as a writer when no role is given (RFC 0009,
    /// "Accounts and devices"; RFC 0008, "Roles and rights"). Without it,
    /// the member is a device.
    pub fn agent(mut self) -> OpenOptions {
        self.agent = true;
        self
    }

    /// The directory the database lives in, which the SDK owns and creates
    /// when it does not exist (RFC 0009, "The Rust SDK", **Where the
    /// database lives.**).
    pub fn path(mut self, dir: impl Into<PathBuf>) -> OpenOptions {
        self.path = Some(dir.into());
        self
    }

    /// Keeps the member key and the account key in the file at `path`
    /// instead of the platform keystore, for a headless agent that has
    /// none (RFC 0009, "The Rust SDK", **Where the database lives.**).
    /// Only the file's permissions protect it:
    /// whoever reads the file, or a backup of it, holds the member key. On
    /// Unix the SDK creates it readable by its user alone. It takes a
    /// [`path`](OpenOptions::path) for the database.
    pub fn key_file(mut self, path: impl Into<PathBuf>) -> OpenOptions {
        self.key_file = Some(path.into());
        self
    }

    /// Keeps the database and its keys in memory, for tests: they are gone
    /// when the [`Db`](crate::Db) is dropped (RFC 0009, "The Rust SDK", **Where the
    /// database lives.**).
    pub fn in_memory(mut self) -> OpenOptions {
        self.in_memory = true;
        self
    }

    /// Hands `plugin`'s field types to the core, as a plugin's crate
    /// exports them from `plugin()`; call it once per plugin (RFC 0011,
    /// "Handing plugins to the core"). A plugin field of the lock folds
    /// through its code; one whose type and rule no plugin given here
    /// implements parks, and a write to it is refused with `UnknownType`.
    /// A plugin the lock does not name is ignored. [`Db::open`](crate::Db::open) refuses two
    /// plugins of one namespace, or a plugin of the namespace `ncdb`, with
    /// `InvalidValue`.
    pub fn plugin(mut self, plugin: Plugin) -> OpenOptions {
        self.plugins.push(plugin);
        self
    }

    /// The one relay the database syncs with (RFC 0009, "Sync and status",
    /// **The relay connection.**), by
    /// its URL: `wss://`, WebSocket over TLS verified against the
    /// platform's trust store, or `ws://` on a local network. [`Db::open`](crate::Db::open)
    /// refuses any other URL with `InvalidValue`. In a browser, the relay
    /// URL is `wss://`, except for localhost while developing: a page
    /// served over `https://` cannot open `ws://` (RFC 0009, NOC-245).
    /// A token for the relay belongs in the query string, never in the
    /// path, since the SDK shows a relay URL with its path (NOC-495).
    pub fn relay(mut self, url: &str) -> OpenOptions {
        self.relay = Some(url.to_owned());
        self
    }

    /// The database joins an account another device created and creates
    /// no account space of its own (RFC 0010, "The account space"). Until
    /// pairing (NOC-267) opens a database this way, only the tests do.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn joins_account(mut self) -> OpenOptions {
        self.joins_account = true;
        self
    }
}

impl std::fmt::Debug for OpenOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenOptions").finish_non_exhaustive()
    }
}

/// Where a database lives, as its options name it.
pub(crate) enum Place {
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
    /// or memory together with a path or a key file (RFC 0009, "Open and
    /// close", **Where the database lives.**).
    pub(crate) fn place(&self) -> Result<Place, Error> {
        let invalid = |message: &str| Err(Error::new(ErrorCode::InvalidValue, message));
        #[cfg(target_arch = "wasm32")]
        if let Some(name) = &self.browser {
            if self.path.is_some() || self.in_memory || self.key_file.is_some() {
                return invalid(crate::texts::error::OPTIONS_BROWSER_AND_PATH);
            }
            return Ok(Place::Browser { name: name.clone() });
        }
        match (&self.path, self.in_memory) {
            (Some(_), true) => invalid(crate::texts::error::OPTIONS_PATH_AND_MEMORY),
            (None, true) if self.key_file.is_some() => {
                invalid(crate::texts::error::OPTIONS_KEY_FILE_AND_MEMORY)
            }
            (None, true) => Ok(Place::Memory),
            (None, false) => invalid(crate::texts::error::OPTIONS_NO_PLACE),
            (Some(dir), false) => Ok(Place::Path {
                dir: dir.clone(),
                key_file: self.key_file.clone(),
            }),
        }
    }
}
