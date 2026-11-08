//! What a field type defines, as RFC 0004 ("The plugin contract", "What a
//! field type defines") names it: how an operation is applied, the canonical
//! bytes of the state, what the app reads, and where each operation went.
//!
//! Internal to the core for now: the names here are working names, not the
//! SDK's public API, which the owner decides (RFC 0004, decision 7).

use crate::ids::OperationId;

/// Where a received operation went. Every received operation has exactly
/// one outcome, the same on every copy that holds the same operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// It is visible in the read value.
    Visible,
    /// The operation with this id, whose author had seen it, replaced it.
    Replaced(OperationId),
    /// It lost, and the app can read that it lost.
    Lost,
}

/// A field type: the merge rule of one kind of field. A value of it is the
/// field as the schema declares it; the state is kept apart, one per field
/// of an object.
pub trait FieldType {
    /// The state of one field. `Default` is the state of a field nobody has
    /// written.
    type State: Default + Clone;
    /// One operation on the field, with its id, author and stamp.
    type Operation;
    /// What the app, views and computed fields see.
    type Read;

    /// Merges one received operation into `state`, in whatever order the
    /// operations arrive. Applying an operation the state holds changes
    /// nothing.
    fn apply(&self, state: &mut Self::State, op: &Self::Operation);
    /// The canonical encoding of `state`: equal states, equal bytes.
    fn state_bytes(&self, state: &Self::State) -> Vec<u8>;
    /// The read value.
    fn read(&self, state: &Self::State) -> Self::Read;
    /// Where the operation `id` went, or `None` when `state` never received
    /// it.
    fn outcome(&self, state: &Self::State, id: OperationId) -> Option<Outcome>;
}
