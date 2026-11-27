//! An agent's anchored edits (RFC 0013, "An agent: anchored edits"): the
//! agent reads a rich-text field as Markdown with a version, and edits it
//! by quoting the Markdown it read and giving the Markdown that replaces
//! it. The core keeps the version, the document's state at the read, in a
//! device-local cache with a time limit ([`Versions`]), no part of the
//! view or the log. An edit:
//!
//! 1. takes the document as it was at the version, plus the edits this
//!    agent's lease has written since;
//! 2. renders it as Markdown and finds each quote there, exactly once, or
//!    is refused with [`Error::AnchorNotFound`] or
//!    [`Error::AnchorAmbiguous`]; both depend only on what the agent read;
//! 3. reads the replacement as Markdown and writes the smallest change,
//!    text by characters, marks by ranges, blocks by type, under the
//!    lease's client id, directly on the Yrs document, so that what the
//!    Markdown does not show stays untouched;
//! 4. returns that change as one Yjs update ([`Written`]), which the
//!    caller writes as an operation 5 and folds into the live document,
//!    where Yjs merges it with everything written since the read.
//!
//! Three rules keep an edit focused: a quote's ends fall on text, never
//! inside markup ([`lookup`]); a block's type changes only when the
//! replacement changes it ([`diff`]); and an agent's edits for one field go
//! through one lease, in order, so that its clock continues: an edit takes
//! the lease mutably and moves its clock on.
//!
//! Errors name the edit by its index and, when a quote occurs more than
//! once, the number of matches; never the quote or any other text of the
//! document (RFC 0003, "Keys never reach logs, errors or telemetry").

use std::collections::BTreeMap;
use std::fmt;

use super::State;
use crate::ids::{ObjectId, SpaceId};
use crate::redact::Size;
use crate::view::Lease;

/// The field a read version belongs to.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FieldKey {
    /// The space of the object.
    pub space: SpaceId,
    /// The object.
    pub object: ObjectId,
    /// The object's collection.
    pub collection: String,
    /// The rich-text field.
    pub field: String,
}

/// An opaque name for a read version, as the agent hands it back.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version(pub [u8; 16]);

impl fmt::Debug for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Version(..)")
    }
}

/// What an agent reads: the document as Markdown and its version.
#[derive(Clone, PartialEq, Eq)]
pub struct Reading {
    /// The document as Markdown, as [`super::read::markdown`].
    pub markdown: String,
    /// The version the edits name.
    pub version: Version,
}

impl fmt::Debug for Reading {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Reading")
            .field("markdown", &Size(self.markdown.len(), "bytes <redacted>"))
            .field("version", &self.version)
            .finish()
    }
}

/// One anchored edit.
#[derive(Clone, PartialEq, Eq)]
pub enum Anchored {
    /// Replaces the Markdown `quote` with the Markdown `replacement`.
    Replace { quote: String, replacement: String },
    /// Adds the blocks of `markdown` after the block that holds the end of
    /// `quote`, at the top of the document.
    InsertAfter { quote: String, markdown: String },
    /// Adds the blocks of `markdown` at the end of the document.
    Append { markdown: String },
}

impl fmt::Debug for Anchored {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Anchored::Replace { .. } => "Replace(<redacted>)",
            Anchored::InsertAfter { .. } => "InsertAfter(<redacted>)",
            Anchored::Append { .. } => "Append(<redacted>)",
        })
    }
}

/// The change an edit made: one Yjs update, encoding v1, with the client
/// ids it adds items under and their nonces, as operation 5 carries them.
#[derive(Clone, PartialEq, Eq)]
pub struct Written {
    /// The update.
    pub update: Vec<u8>,
    /// The lease's client id and nonce, when the update adds items.
    pub clients: BTreeMap<u64, [u8; 8]>,
}

impl fmt::Debug for Written {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Written")
            .field("update", &Size(self.update.len(), "bytes"))
            .field("clients", &self.clients.len())
            .finish()
    }
}

/// Why an edit is refused. Nothing is written then.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// The quote of edit `edit` does not occur in the version with both
    /// ends on text.
    AnchorNotFound { edit: usize },
    /// The quote of edit `edit` occurs `matches` times.
    AnchorAmbiguous { edit: usize, matches: usize },
    /// The cache no longer holds the version, or never did: the agent
    /// reads again.
    VersionUnknown,
    /// The lease is for another object than the version.
    LeaseMismatch,
    /// The version holds fewer of the lease's items than the lease has
    /// written, so a new item could reuse an id.
    LeaseBehind,
    /// Edit `edit` touches a part of the document, or writes a part, that
    /// the core does not edit from Markdown yet.
    Unsupported { edit: usize },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::AnchorNotFound { edit } => write!(f, "edit {edit}: the quote is not found"),
            Error::AnchorAmbiguous { edit, matches } => {
                write!(f, "edit {edit}: the quote occurs {matches} times")
            }
            Error::VersionUnknown => f.write_str("the version is no longer held"),
            Error::LeaseMismatch => f.write_str("the lease is for another object"),
            Error::LeaseBehind => f.write_str("the version is behind the lease"),
            Error::Unsupported { edit } => {
                write!(f, "edit {edit}: the core does not make this edit")
            }
        }
    }
}

impl std::error::Error for Error {}

/// A read version the cache holds.
struct Entry {
    field: FieldKey,
    document: Vec<u8>,
    read_at: u64,
}

/// The device-local cache of read versions, with a time limit: a version
/// read at `t` is held while the clock the caller passes is below `t` plus
/// the limit. It also keeps the updates each lease wrote through it, for
/// as long as it holds a version of their field.
pub struct Versions {
    limit: u64,
    entries: BTreeMap<Version, Entry>,
    written: BTreeMap<(FieldKey, u64), Vec<Vec<u8>>>,
}

impl fmt::Debug for Versions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Versions")
            .field("limit", &self.limit)
            .field("entries", &self.entries.len())
            .finish()
    }
}

impl Versions {
    /// A cache that holds a version for `limit` milliseconds after its
    /// read.
    pub fn new(limit: u64) -> Versions {
        Versions {
            limit,
            entries: BTreeMap::new(),
            written: BTreeMap::new(),
        }
    }

    /// Reads `state`, the state of `field`, at `now` (milliseconds): its
    /// Markdown, and a version the cache holds until the limit passes.
    pub fn read(&mut self, field: FieldKey, state: &State, now: u64) -> Reading {
        self.expire(now);
        let mut version = [0u8; 16];
        getrandom::fill(&mut version).expect("the platform's secure random generator failed");
        let version = Version(version);
        self.entries.insert(
            version,
            Entry {
                field,
                document: state.document().to_vec(),
                read_at: now,
            },
        );
        Reading {
            markdown: super::read::markdown(state),
            version,
        }
    }

    /// Makes `edits`, in order, at `version` through `lease`, at `now`.
    pub fn edit(
        &mut self,
        version: &Version,
        lease: &mut Lease,
        edits: &[Anchored],
        now: u64,
    ) -> Result<Written, Error> {
        let _ = (version, lease, edits, now);
        Err(Error::Unsupported { edit: 0 })
    }

    /// Drops the versions whose time has passed, and the updates of fields
    /// it holds no version of.
    fn expire(&mut self, now: u64) {
        let limit = self.limit;
        self.entries
            .retain(|_, entry| now < entry.read_at.saturating_add(limit));
        let fields: std::collections::BTreeSet<&FieldKey> =
            self.entries.values().map(|entry| &entry.field).collect();
        self.written.retain(|(field, _), _| fields.contains(field));
    }
}
