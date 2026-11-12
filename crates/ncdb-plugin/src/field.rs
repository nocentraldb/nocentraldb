//! What a plugin field type implements (RFC 0011, "The field type a plugin
//! implements"): RFC 0004's contract, a state, its operations, apply,
//! read and outcome, with two more functions the core needs, `decode_state`
//! and `write`, and `configure`, which reads a field's `config`.

use std::collections::BTreeMap;
use std::fmt;

use crate::clock::Hlc;
use crate::data::Data;
use crate::ids::{MemberId, OperationId};

/// What an operation takes from its change rather than from its own
/// bytes: its id, its author and its stamp.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Origin {
    /// The change id and the operation's position in the payload.
    pub id: OperationId,
    /// The member that signed the change.
    pub author: MemberId,
    /// The payload's clock.
    pub stamp: Hlc,
}

/// Where a received operation went (RFC 0004, "What a field type
/// defines"). Every received operation has exactly one outcome, the same
/// on every copy that holds the same operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// It is visible in the read value.
    Visible,
    /// The operation with this id, whose author had seen it, replaced it.
    Replaced(OperationId),
    /// It lost, and the app can read that it lost.
    Lost,
}

/// Why a plugin parks an operation it cannot read (RFC 0004, "Operations a
/// member does not understand"). The core parks an operation it can judge
/// without the plugin itself: a number below 64, arguments that are no map
/// with integer keys, or a field whose type has no code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ParkReason {
    /// The type defines no operation with this number.
    UnknownOperation,
    /// The arguments carry a key from 1 to 63 that the operation does not
    /// define.
    UnknownArgument,
    /// A required argument is missing, or has the wrong shape.
    MalformedArguments,
}

/// A field's `config` that the plugin cannot read: the field then parks as
/// an unknown type (RFC 0011, "A type that is not registered"). Its text
/// says why, and names no content.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigError(String);

impl ConfigError {
    /// A refusal that says why.
    pub fn new(why: impl Into<String>) -> ConfigError {
        ConfigError(why.into())
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ConfigError {}

/// A write intent the plugin refuses: the SDK refuses the write before
/// anything is written. Its text says why, and names no content.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InvalidIntent(String);

impl InvalidIntent {
    /// A refusal that says why.
    pub fn new(why: impl Into<String>) -> InvalidIntent {
        InvalidIntent(why.into())
    }
}

impl fmt::Display for InvalidIntent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for InvalidIntent {}

/// One operation as a plugin writes it: its number, 64 or more, and its
/// arguments by key.
pub type Written = (u64, BTreeMap<u64, Data>);

/// A plugin field type: the merge rule of one kind of field (RFC 0004,
/// "The plugin contract"; RFC 0011). A value of it is registered with
/// [`Plugin::field_type`](crate::Plugin::field_type), and the core calls
/// [`configure`](FieldType::configure) once for each field of that type,
/// then the other functions on the value it returns.
///
/// Every function must be pure: no clock, no randomness, no order of a
/// hash map, the same result natively and on wasm32 (RFC 0004, C3). The
/// core cannot check this; `ncdb-harness` does, in the plugin's own CI.
/// `Debug` of the state and the operation must not show content.
pub trait FieldType: Sized + Send + Sync + 'static {
    /// The state of one field. `Default` is the state of a field nobody
    /// has written.
    type State: Default;
    /// One operation on the field, as [`decode`](FieldType::decode) reads
    /// it.
    type Operation;

    /// The type configured for one field, from the field's rule, which the
    /// registry already matched, and its `config` in the lock, or a
    /// refusal of a `config` it cannot read.
    fn configure(&self, rule: u64, config: Option<&Data>) -> Result<Self, ConfigError>;

    /// The canonical bytes of `state`: equal states, equal bytes.
    fn state_bytes(&self, state: &Self::State) -> Vec<u8>;

    /// The state of bytes [`state_bytes`](FieldType::state_bytes) wrote,
    /// or `None` for bytes it did not write. It must round-trip.
    fn decode_state(&self, bytes: &[u8]) -> Option<Self::State>;

    /// One operation of the field, from its number, 64 or more, and its
    /// arguments, keys from 64 up already dropped; or why it is parked.
    fn decode(
        &self,
        number: u64,
        arguments: &BTreeMap<u64, Data>,
        origin: Origin,
    ) -> Result<Self::Operation, ParkReason>;

    /// Merges one received operation into `state`, in whatever order the
    /// operations arrive. Applying an operation the state holds changes
    /// nothing.
    fn apply(&self, state: &mut Self::State, operation: &Self::Operation);

    /// What the app reads.
    fn read(&self, state: &Self::State) -> Data;

    /// Where the operation `id` went, or `None` when `state` never received
    /// it.
    fn outcome(&self, state: &Self::State, id: OperationId) -> Option<Outcome>;

    /// The operations that carry out `intent`, what the app writes, against
    /// the state the member's copy holds; each with a number of 64 or more.
    fn write(&self, state: &Self::State, intent: &Data) -> Result<Vec<Written>, InvalidIntent>;
}
