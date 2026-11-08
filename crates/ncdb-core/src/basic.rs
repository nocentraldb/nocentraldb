//! Basic values: strings, numbers, booleans and a choice from a fixed list,
//! merged as RFC 0004 ("Basic values") defines it. The last writer wins on
//! read; the writes that lost stay in the state as conflicts until someone
//! writes having seen them.

use std::collections::{BTreeMap, BTreeSet};

use crate::cbor::Value;
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
        let _ = (state, id, author, stamp, value);
        todo!()
    }
}

impl FieldType for BasicValue {
    type State = State;
    type Operation = Write;
    type Read = Read;

    fn apply(&self, state: &mut State, op: &Write) {
        let _ = (state, op);
        todo!()
    }

    fn state_bytes(&self, state: &State) -> Vec<u8> {
        let _ = state;
        todo!()
    }

    fn read(&self, state: &State) -> Read {
        let _ = state;
        todo!()
    }

    fn outcome(&self, state: &State, id: OperationId) -> Option<Outcome> {
        let _ = (state, id);
        todo!()
    }
}
