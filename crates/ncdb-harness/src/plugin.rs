//! A plugin's field type in the harness (RFC 0011, "Checking a plugin:
//! ncdb-harness"): [`PluginState`] holds one field's state as the bytes
//! the view would store, and applies each operation as the core does:
//! decode the state, decode and apply the operation, encode the state
//! again. An operation the plugin parks leaves the state as it is and
//! reads as [`Outcome::Parked`].
//!
//! The harness numbers operations and members; the adapter gives each
//! operation the [`Origin`] a change would: an operation id whose change
//! id starts with the harness's number, a member id of the member's
//! number, and a stamp that grows with every operation made, so a later
//! operation's stamp is above every one its author had seen.

use std::collections::BTreeMap;
use std::fmt;

use ncdb_plugin::{
    ChangeId, Data, FieldType, Hlc, InvalidIntent, MemberId, OperationId, Origin,
    Outcome as PluginOutcome, ParkReason,
};

use crate::convergence::{Member, Mergeable, Op, OpId, Outcome};

/// One operation of a plugin's field type: its number and its arguments,
/// as a change carries them.
#[derive(Clone, PartialEq)]
pub struct PluginOp {
    /// The operation's number, 64 or more for a plugin's own.
    pub number: u64,
    /// The arguments by key.
    pub arguments: BTreeMap<u64, Data>,
}

impl fmt::Debug for PluginOp {
    /// The number and the arguments' canonical bytes as hex, so a
    /// counterexample can be read back.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{} ", self.number)?;
        for (key, value) in &self.arguments {
            write!(f, "{key}=")?;
            match value.encode() {
                Ok(bytes) => bytes.iter().try_for_each(|b| write!(f, "{b:02x}"))?,
                Err(_) => f.write_str("<out of range>")?,
            }
            f.write_str(" ")?;
        }
        Ok(())
    }
}

/// One field of the plugin type `T`, as `T::default()` configures it, its
/// state kept as the bytes the view would store.
pub struct PluginState<T: FieldType + Default> {
    field: T,
    bytes: Vec<u8>,
}

impl<T: FieldType + Default> PluginState<T> {
    /// The state as the plugin decodes it from the bytes, or `None` for
    /// bytes it does not read.
    fn state(&self) -> Option<T::State> {
        self.field.decode_state(&self.bytes)
    }

    /// The operations the plugin's `write` makes of `intent` against this
    /// state, for a generator that writes through the plugin, as an app
    /// does.
    pub fn write(&self, intent: &Data) -> Result<Vec<PluginOp>, InvalidIntent> {
        let state = self.state().unwrap_or_default();
        Ok(self
            .field
            .write(&state, intent)?
            .into_iter()
            .map(|(number, arguments)| PluginOp { number, arguments })
            .collect())
    }

    /// The state's bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

impl<T: FieldType + Default> Default for PluginState<T> {
    fn default() -> Self {
        let field = T::default();
        let bytes = field.state_bytes(&T::State::default());
        PluginState { field, bytes }
    }
}

impl<T: FieldType + Default> Clone for PluginState<T> {
    fn clone(&self) -> Self {
        PluginState {
            field: T::default(),
            bytes: self.bytes.clone(),
        }
    }
}

impl<T: FieldType + Default> fmt::Debug for PluginState<T> {
    /// The state's bytes as hex.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.bytes.iter().try_for_each(|b| write!(f, "{b:02x}"))
    }
}

/// The id a change would give the harness's operation `id`: its number in
/// the first four bytes of the change id.
pub fn operation_id(id: OpId) -> OperationId {
    let mut change = [0u8; 32];
    change[..4].copy_from_slice(&id.to_be_bytes());
    OperationId {
        change: ChangeId(change),
        position: 0,
    }
}

/// The harness's number of an operation id that [`operation_id`] made.
fn op_id(id: OperationId) -> OpId {
    let bytes: [u8; 4] = id.change.0[..4].try_into().expect("four bytes");
    OpId::from_be_bytes(bytes)
}

/// The origin a change would give the harness's operation `id` by
/// `author`.
pub fn origin(id: OpId, author: Member) -> Origin {
    Origin {
        id: operation_id(id),
        author: MemberId([author; 32]),
        stamp: Hlc(u64::from(id)),
    }
}

/// What a plugin's read gives, as its canonical bytes, so copies compare
/// by them and a counterexample shows them.
#[derive(Clone, PartialEq)]
pub struct ReadBytes(pub Option<Vec<u8>>);

impl fmt::Debug for ReadBytes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            Some(bytes) => bytes.iter().try_for_each(|b| write!(f, "{b:02x}")),
            None => f.write_str("<no read>"),
        }
    }
}

impl<T: FieldType + Default> Mergeable for PluginState<T> {
    type Change = PluginOp;
    type Read = ReadBytes;

    fn apply(&mut self, op: &Op<PluginOp>) {
        let Some(mut state) = self.state() else {
            return;
        };
        let decoded: Result<T::Operation, ParkReason> =
            self.field
                .decode(op.body.number, &op.body.arguments, origin(op.id, op.author));
        if let Ok(operation) = decoded {
            self.field.apply(&mut state, &operation);
            self.bytes = self.field.state_bytes(&state);
        }
    }

    fn state_bytes(&self) -> Vec<u8> {
        self.bytes.clone()
    }

    fn read(&self) -> ReadBytes {
        ReadBytes(
            self.state()
                .and_then(|state| self.field.read(&state).encode().ok()),
        )
    }

    fn outcome(&self, id: OpId) -> Outcome {
        let outcome = self
            .state()
            .and_then(|state| self.field.outcome(&state, operation_id(id)));
        match outcome {
            Some(PluginOutcome::Visible) => Outcome::Visible,
            Some(PluginOutcome::Replaced(by)) => Outcome::Replaced(op_id(by)),
            Some(PluginOutcome::Lost) => Outcome::Lost,
            None => Outcome::Parked,
        }
    }

    fn round_trips(&self) -> bool {
        self.state()
            .is_some_and(|state| self.field.state_bytes(&state) == self.bytes)
    }
}
