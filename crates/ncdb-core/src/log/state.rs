//! The states a member gives the changes of its log (RFC 0002, "What a
//! member does with a change it receives"), the relay's refusal recorded
//! beside them, and the counts the log keeps of both.

use std::collections::BTreeMap;

/// Where a member's check of a change stopped (RFC 0002, "What
/// a member does with a change it receives"). New states and reasons may
/// follow, so the enums are open to extension.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum State {
    /// Passed every check and folded. Operations the member's model does
    /// not know yet are parked on their own (RFC 0002, "The atomic unit"),
    /// listed here in order. A
    /// sealed change moves on to rejected, and is un-folded, when a later
    /// control change makes its membership check fail (RFC 0006,
    /// "Un-folding").
    Folded {
        /// The operations of the change that were not folded.
        parked: Vec<ParkedOperation>,
    },
    /// Kept and not folded, because this client does not understand it
    /// yet; folded after an upgrade.
    Parked(ParkReason),
    /// Kept and not folded, because it failed a check, or un-folded,
    /// because a later control change made it fail; checked again when a
    /// control change of the space passes (RFC 0002, "Rejected changes").
    Rejected(RejectReason),
}

/// Why a change is parked: steps 2 and 6.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum ParkReason {
    /// The suite id is unknown.
    UnknownSuite,
    /// The kind is unknown.
    UnknownKind,
    /// The header format is unknown.
    UnknownHeaderFormat,
    /// The header carries an unknown "must understand" key.
    UnknownHeaderKey,
    /// The payload format is unknown, or the payload is not a map this
    /// core can read, as a newer format may not be.
    UnknownPayloadFormat,
    /// The payload carries an unknown "must understand" key.
    UnknownPayloadKey,
    /// Payload key 64 names another model than the member's lock, or has
    /// the wrong shape: the change is parked whole (RFC 0002, "The
    /// writer's model").
    AnotherModel,
}

/// An operation of a folded change that was parked on its own (RFC 0002,
/// "The atomic unit").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParkedOperation {
    /// The operation's position in the change's operation array.
    pub position: u64,
    /// Why it was parked.
    pub reason: OperationParkReason,
}

/// Why an operation is parked on its own: step 7.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum OperationParkReason {
    /// The member's model does not know the collection.
    UnknownCollection,
    /// The member's model does not know the field.
    UnknownField,
    /// The member's model names the field, and no code implements its
    /// plugin type and rule (RFC 0011, "A type that is not registered").
    UnknownType,
    /// The operation carries an unknown "must understand" key.
    UnknownKey,
    /// The field's type does not define the operation's number (RFC 0004,
    /// "Operations a member does not understand").
    UnknownOperation,
    /// The operation's arguments carry a "must understand" key, 4 to 63,
    /// that the operation does not define (RFC 0004, "Operations a member
    /// does not understand").
    UnknownArgument,
    /// The operation's arguments are malformed (RFC 0004, "Operations a
    /// member does not understand").
    MalformedArguments,
}

/// Why a change is rejected: steps 3 to 5.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum RejectReason {
    /// Step 3: no membership entry the member holds names the author.
    UnknownAuthor,
    /// Step 3: the signature does not verify.
    BadSignature,
    /// Step 4: the author is not a member at the change's key generation.
    NotMember,
    /// Step 5: the member holds no key envelope for the key generation.
    NoKey,
    /// Step 5: the AEAD fails.
    Undecryptable,
}

/// The relay's refusal of a change the member holds: the relay's error
/// code (RFC 0007), such as `author_not_member`. It is recorded beside the
/// change and never changes its [`State`] (RFC 0002, "A change the relay
/// refuses after the member folded it"): no other member sees the
/// relay's answer, so a state that followed it could differ between
/// members that hold the same changes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelayRefusal(pub String);

/// How many of one author's changes in a log the relay has neither
/// confirmed nor refused, and how many it refused (NOC-147). A refused
/// change counts as refused alone, even when the relay also confirmed it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RelayCounts {
    /// The changes with neither a relay position nor a refusal.
    pub unconfirmed: u64,
    /// The changes the relay refused.
    pub refused: u64,
}

/// How many changes of a log are parked and rejected, per reason, and how
/// many operations of its folded changes are parked, per reason: what a
/// member reports per space (RFC 0009, "Sync and status"). The log keeps
/// them as states are set, so reading them costs no pass over the log.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StateCounts {
    /// The parked changes, per reason.
    pub parked: BTreeMap<ParkReason, u64>,
    /// The parked operations of folded changes, per reason.
    pub parked_operations: BTreeMap<OperationParkReason, u64>,
    /// The rejected changes, per reason.
    pub rejected: BTreeMap<RejectReason, u64>,
}

impl StateCounts {
    /// Adds `state` to the counts, or takes it away.
    pub(super) fn count(&mut self, state: &State, add: bool) {
        fn step<K: Ord>(map: &mut BTreeMap<K, u64>, key: K, add: bool) {
            if add {
                *map.entry(key).or_default() += 1;
            } else if let Some(count) = map.get_mut(&key) {
                *count -= 1;
                if *count == 0 {
                    map.remove(&key);
                }
            }
        }
        match state {
            State::Folded { parked } => {
                for operation in parked {
                    step(&mut self.parked_operations, operation.reason, add);
                }
            }
            State::Parked(reason) => step(&mut self.parked, *reason, add),
            State::Rejected(reason) => step(&mut self.rejected, *reason, add),
        }
    }
}
