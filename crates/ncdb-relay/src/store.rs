//! The backend interface of RFC 0007 ("The backend interface"): what every
//! relay backend implements. A backend only stores. It never checks a
//! signature, a membership rule or a frame; the relay core ([`crate::Relay`])
//! calls it only after every check has passed.
//!
//! The rules every backend keeps, which [`crate::conformance`] tests:
//!
//! - **Dense positions**, without a gap and without a position used twice:
//!   an append is conditional on the head.
//! - **One change id, one position**: the check for a duplicate and the
//!   append are one atomic step.
//! - **An append and its effects are atomic.**
//! - **Exact bytes**: a frame comes back byte for byte.
//! - **Read your writes**, also through another handle on the same storage.
//! - **A new epoch after any loss.**

use std::fmt;
use std::future::Future;

use ncdb_core::change::Kind;
use ncdb_core::ids::{ChangeId, MemberId, SpaceId};

/// A relay position: 1, 2, 3, ... per space, in the order the relay
/// accepted the changes (RFC 0002). 0 is the head of an empty log.
pub type Position = u64;

/// The relay epoch: a random id of the relay's current history. Positions
/// are valid only within one epoch.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Epoch(pub [u8; 16]);

/// The id of a person key in one space: 16 random bytes (RFC 0003).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PersonKeyId(pub [u8; 16]);

/// A change the core appends: what the relay knows of it besides its frame.
#[derive(Clone, PartialEq, Eq)]
pub struct NewEntry {
    /// The change id, computed from the frame.
    pub change: ChangeId,
    /// The relay's clock when it appended the change, in milliseconds since
    /// the Unix epoch. A diagnostic only (RFC 0002).
    pub accepted_at: u64,
    /// The author's member id, from the header.
    pub author: MemberId,
    /// Sealed or control, from the header.
    pub kind: Kind,
    /// The frame, exactly as the member sent it.
    pub frame: Vec<u8>,
}

/// An entry of a space's log.
#[derive(Clone, PartialEq, Eq)]
pub struct Entry {
    /// Its position in the space's log.
    pub position: Position,
    /// The change id.
    pub change: ChangeId,
    /// When the relay appended it.
    pub accepted_at: u64,
    /// The author's member id.
    pub author: MemberId,
    /// Sealed or control.
    pub kind: Kind,
    /// The frame, byte for byte as it was appended.
    pub frame: Vec<u8>,
}

/// One member's row in a space's membership table: whether it is a current
/// member, and its state as the core encodes it (its public keys and
/// certificate, its grants with their from generations, its revocations with
/// their cutoffs). The backend stores the state as opaque bytes;
/// [`crate::record`] reads and writes them.
#[derive(Clone, PartialEq, Eq)]
pub struct MemberRecord {
    /// The member.
    pub member: MemberId,
    /// Whether it is a current member: [`RelayStore::members_of`] lists only
    /// the spaces it currently belongs to.
    pub current: bool,
    /// The encoded state.
    pub state: Vec<u8>,
}

/// What an append changes besides the log, in the same atomic step.
#[derive(Clone, PartialEq, Eq)]
pub enum Effect {
    /// Writes a member's row of the space's membership table, replacing the
    /// row it had.
    Member(MemberRecord),
}

/// A person-key record (RFC 0003, decision 14): opaque wrapped bytes and the
/// fields outside the wrapping.
#[derive(Clone, PartialEq, Eq)]
pub struct PersonKeyRecord {
    /// The person key it wraps.
    pub id: PersonKeyId,
    /// The generation the record is wrapped under.
    pub generation: u64,
    /// The member that wrote it.
    pub writer: MemberId,
    /// The wrapped key, which the relay cannot open.
    pub wrapped: Vec<u8>,
    /// The writer's signature over the record.
    pub signature: Vec<u8>,
}

/// What an append did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppendOutcome {
    /// The entries now sit at the positions after the expected head, up to
    /// this new head.
    Appended {
        /// The space's head after the append.
        head: Position,
    },
    /// Another append got there first: the space's head is this, not the
    /// expected one. Nothing was appended.
    HeadMoved(Position),
    /// A change id of the entries is already in the space's log, at this
    /// position. Nothing was appended.
    Duplicate {
        /// The change already held.
        change: ChangeId,
        /// Its position.
        position: Position,
    },
}

/// A backend that failed or timed out. Nothing was appended. Its detail
/// names what failed, never a frame or a key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoreError(pub String);

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "the relay backend failed: {}", self.0)
    }
}

impl std::error::Error for StoreError {}

/// What every relay backend implements: SQLite, Postgres, DynamoDB with S3.
/// The relay core calls it only after every check has passed.
///
/// The methods are asynchronous, as RFC 0007 draws them, and their futures
/// are `Send`, so a server can run them on any thread.
pub trait RelayStore: Send + Sync {
    /// Appends `entries` at positions head + 1, head + 2, ... if and only if
    /// the space's head is still `expected_head`, and applies `effects` in
    /// the same atomic step. An append at head 0 to a space the relay does
    /// not hold creates it. A change id already in the space's log is never
    /// appended twice.
    fn append(
        &self,
        space: SpaceId,
        expected_head: Position,
        entries: Vec<NewEntry>,
        effects: Vec<Effect>,
    ) -> impl Future<Output = Result<AppendOutcome, StoreError>> + Send;

    /// The entries after `after`, in position order, at most `limit`.
    fn read(
        &self,
        space: SpaceId,
        after: Position,
        limit: u32,
    ) -> impl Future<Output = Result<Vec<Entry>, StoreError>> + Send;

    /// The head of the space's log, or none if the relay holds no such space.
    fn head(
        &self,
        space: SpaceId,
    ) -> impl Future<Output = Result<Option<Position>, StoreError>> + Send;

    /// The position of a change id in the space's log, if it is there.
    fn find(
        &self,
        space: SpaceId,
        change: ChangeId,
    ) -> impl Future<Output = Result<Option<Position>, StoreError>> + Send;

    /// A member's row of the space's membership table. Written only through
    /// [`Effect::Member`].
    fn member(
        &self,
        space: SpaceId,
        member: MemberId,
    ) -> impl Future<Output = Result<Option<MemberRecord>, StoreError>> + Send;

    /// The spaces the member currently belongs to, in id order.
    fn members_of(
        &self,
        member: MemberId,
    ) -> impl Future<Output = Result<Vec<SpaceId>, StoreError>> + Send;

    /// The space's person-key records, in person-key id order.
    fn person_keys(
        &self,
        space: SpaceId,
    ) -> impl Future<Output = Result<Vec<PersonKeyRecord>, StoreError>> + Send;

    /// Stores records the core has matched to their person-key entries,
    /// each replacing the record of its person-key id. There is no delete
    /// request: a record goes only through [`RelayStore::delete_person_keys`]
    /// once its shred entry is stable (RFC 0008, M20).
    fn put_person_keys(
        &self,
        space: SpaceId,
        records: Vec<PersonKeyRecord>,
    ) -> impl Future<Output = Result<(), StoreError>> + Send;

    /// Deletes the person-key records of shred entries that became stable.
    fn delete_person_keys(
        &self,
        space: SpaceId,
        ids: Vec<PersonKeyId>,
    ) -> impl Future<Output = Result<(), StoreError>> + Send;

    /// The relay epoch.
    fn epoch(&self) -> impl Future<Output = Result<Epoch, StoreError>> + Send;

    /// Draws a new relay epoch, which a restore must do before the relay
    /// serves any request.
    fn new_epoch(&self) -> impl Future<Output = Result<Epoch, StoreError>> + Send;
}

/// Debug output shows ids, positions, sizes and kinds: never a frame, a
/// member state or a wrapped key (GENESIS.md, "Never write content, keys or
/// personal fields into logs").
mod debug {
    use super::*;

    fn hex(f: &mut fmt::Formatter<'_>, bytes: &[u8]) -> fmt::Result {
        bytes.iter().try_for_each(|b| write!(f, "{b:02x}"))
    }

    impl fmt::Debug for Epoch {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("Epoch(")?;
            hex(f, &self.0)?;
            f.write_str(")")
        }
    }

    impl fmt::Debug for PersonKeyId {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("PersonKeyId(")?;
            hex(f, &self.0)?;
            f.write_str(")")
        }
    }

    impl fmt::Debug for NewEntry {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("NewEntry")
                .field("change", &self.change)
                .field("accepted_at", &self.accepted_at)
                .field("author", &self.author)
                .field("kind", &self.kind)
                .field("frame_len", &self.frame.len())
                .finish()
        }
    }

    impl fmt::Debug for Entry {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("Entry")
                .field("position", &self.position)
                .field("change", &self.change)
                .field("accepted_at", &self.accepted_at)
                .field("author", &self.author)
                .field("kind", &self.kind)
                .field("frame_len", &self.frame.len())
                .finish()
        }
    }

    impl fmt::Debug for MemberRecord {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("MemberRecord")
                .field("member", &self.member)
                .field("current", &self.current)
                .field("state_len", &self.state.len())
                .finish()
        }
    }

    impl fmt::Debug for Effect {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Effect::Member(record) => f.debug_tuple("Member").field(record).finish(),
            }
        }
    }

    impl fmt::Debug for PersonKeyRecord {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("PersonKeyRecord")
                .field("id", &self.id)
                .field("generation", &self.generation)
                .field("writer", &self.writer)
                .field("wrapped_len", &self.wrapped.len())
                .finish_non_exhaustive()
        }
    }
}
