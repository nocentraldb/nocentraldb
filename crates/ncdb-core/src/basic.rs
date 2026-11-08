//! Basic values: strings, numbers, booleans and a choice from a fixed list,
//! merged as RFC 0004 ("Basic values") defines it. The last writer wins on
//! read; the writes that lost stay in the state as conflicts until someone
//! writes having seen them.

use std::collections::{BTreeMap, BTreeSet};

use crate::cbor::{self, Value};
use crate::clock::Hlc;
use crate::field::{FieldType, Outcome};
use crate::ids::{MemberId, OperationId};

/// The kind of a basic value field, as the schema declares it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Any Unicode string, stored as written.
    Text,
    /// A 64-bit signed integer.
    Integer,
    /// An IEEE 754 binary64 float.
    Float,
    /// True or false.
    Boolean,
    /// One of the options the schema lists.
    Choice(BTreeSet<String>),
}

/// The basic value field type: a field of one [`Kind`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BasicValue {
    /// What the schema lets a member write.
    pub kind: Kind,
}

/// The one operation of a basic value, RFC 0004's set(value, replaces).
///
/// Its `Debug` output hides the value, as [`Value`]'s does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Write {
    /// The operation's id.
    pub id: OperationId,
    /// The member that signed the change.
    pub author: MemberId,
    /// The change's clock.
    pub stamp: Hlc,
    /// The value written; `Value::Null` clears the field.
    pub value: Value,
    /// The writes the author's copy held as the field's state when it made
    /// the write: the winner and every conflict.
    pub replaces: BTreeSet<OperationId>,
}

/// The state of one basic value field: every write it received.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct State {
    writes: BTreeMap<OperationId, Write>,
}

/// One live write, as the app reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// The write's operation id.
    pub id: OperationId,
    /// Who wrote it.
    pub author: MemberId,
    /// When, by its change's clock.
    pub stamp: Hlc,
    /// The value as written, also when the reader's schema does not allow
    /// it (an option a newer schema added).
    pub value: Value,
    /// Whether the reader's schema allows the value.
    pub known: bool,
}

/// What the app reads of a basic value field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Read {
    /// The winning write, or `None` when no write is live.
    pub value: Option<Entry>,
    /// The losing live writes whose value differs from the winner's, in
    /// stamp order.
    pub conflicts: Vec<Entry>,
}

/// A value the field's kind does not allow. Says nothing about the value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutsideSchema;

impl core::fmt::Display for OutsideSchema {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("the value is outside the field's schema")
    }
}

impl std::error::Error for OutsideSchema {}

impl BasicValue {
    /// A new write of `value`, which replaces what `state` holds: the
    /// winner and every conflict. Refuses a value the kind does not allow.
    pub fn set(
        &self,
        state: &State,
        id: OperationId,
        author: MemberId,
        stamp: Hlc,
        value: Value,
    ) -> Result<Write, OutsideSchema> {
        if !self.allows(&value) {
            return Err(OutsideSchema);
        }
        Ok(Write {
            id,
            author,
            stamp,
            value,
            replaces: state.live().iter().map(|w| w.id).collect(),
        })
    }

    /// Whether the kind allows `value`. `Value::Null`, the empty value,
    /// clears a field of any kind.
    fn allows(&self, value: &Value) -> bool {
        const MAX: u64 = i64::MAX as u64;
        match (&self.kind, value) {
            (_, Value::Null)
            | (Kind::Text, Value::Text(_))
            | (Kind::Float, Value::Float(_))
            | (Kind::Boolean, Value::Bool(_)) => true,
            (Kind::Integer, Value::Unsigned(n) | Value::Negative(n)) => *n <= MAX,
            (Kind::Choice(options), Value::Text(option)) => options.contains(option),
            _ => false,
        }
    }
}

impl Write {
    /// Stamp order (RFC 0004, "Tie-break"): the stamp, then the author
    /// member id, then the operation id. A total order, the same on every
    /// member.
    fn order(&self) -> (Hlc, MemberId, OperationId) {
        (self.stamp, self.author, self.id)
    }
}

impl State {
    /// R: the ids every received write replaces.
    fn replaced(&self) -> BTreeSet<OperationId> {
        self.writes
            .values()
            .flat_map(|w| w.replaces.iter().copied())
            .collect()
    }

    /// L: the writes no received write replaces, in stamp order.
    fn live(&self) -> Vec<&Write> {
        let replaced = self.replaced();
        let mut live: Vec<&Write> = self
            .writes
            .values()
            .filter(|w| !replaced.contains(&w.id))
            .collect();
        live.sort_by_key(|w| w.order());
        live
    }
}

impl BasicValue {
    fn entry(&self, write: &Write) -> Entry {
        Entry {
            id: write.id,
            author: write.author,
            stamp: write.stamp,
            value: write.value.clone(),
            known: self.allows(&write.value),
        }
    }
}

impl FieldType for BasicValue {
    type State = State;
    type Operation = Write;
    type Read = Read;

    /// Keeps every write it receives; the live writes and the replaced ids
    /// are computed from them, so the state is a function of the set of
    /// writes, whatever their order or repeats.
    fn apply(&self, state: &mut State, op: &Write) {
        state.writes.entry(op.id).or_insert_with(|| op.clone());
    }

    /// The canonical CBOR (RFC 0002's profile) of every write received, in
    /// operation id order: `[[id, author, stamp, value, [replaced id…]]…]`,
    /// with an id as `[change id, position]`. The bytes of the local view,
    /// not a wire format.
    fn state_bytes(&self, state: &State) -> Vec<u8> {
        let id = |id: &OperationId| {
            Value::Array(vec![
                Value::Bytes(id.change.0.to_vec()),
                Value::Unsigned(id.position),
            ])
        };
        let writes = state
            .writes
            .values()
            .map(|w| {
                Value::Array(vec![
                    id(&w.id),
                    Value::Bytes(w.author.0.to_vec()),
                    Value::Unsigned(w.stamp.0),
                    w.value.clone(),
                    Value::Array(w.replaces.iter().map(id).collect()),
                ])
            })
            .collect();
        cbor::encode(&Value::Array(writes))
    }

    fn read(&self, state: &State) -> Read {
        let mut live = state.live();
        let Some(winner) = live.pop() else {
            return Read {
                value: None,
                conflicts: Vec::new(),
            };
        };
        Read {
            value: Some(self.entry(winner)),
            conflicts: live
                .into_iter()
                .filter(|w| w.value != winner.value)
                .map(|w| self.entry(w))
                .collect(),
        }
    }

    /// Replaced by the received write with the lowest operation id that
    /// names it; otherwise visible when it is the winner or holds the
    /// winner's value, and lost when it is a listed conflict.
    fn outcome(&self, state: &State, id: OperationId) -> Option<Outcome> {
        state.writes.get(&id)?;
        if let Some(by) = state.writes.values().find(|w| w.replaces.contains(&id)) {
            return Some(Outcome::Replaced(by.id));
        }
        let winner = state.live().pop().map(|w| &w.value);
        Some(if winner == Some(&state.writes[&id].value) {
            Outcome::Visible
        } else {
            Outcome::Lost
        })
    }
}
