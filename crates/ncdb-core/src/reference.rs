//! A reference into another space, as RFC 0004 defines it ("References"): the
//! field type `ncdb/ref`, rule version 1. It is a basic value whose value
//! is the target's space id and object id, written `[space id, object id]`,
//! and it merges by the code of basic values. It has no merge rule of its
//! own.
//!
//! What is new is resolution: whether the member's own copies hold the
//! target, worked out when the app reads, never kept in the state. It
//! differs from member to member, so nothing stored depends on it.

use crate::basic::{self, Values, Write};
use crate::cbor::Value;
use crate::clock::Hlc;
use crate::field::{FieldType, Outcome};
use crate::ids::{MemberId, ObjectId, OperationId, SpaceId};
pub use crate::link::Target;
use crate::link::{self, Objects};

use std::collections::{BTreeMap, BTreeSet};

/// The reference field type: one target, in any space, in one of
/// `collections`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reference {
    /// The collections the schema allows the target in (`targets`).
    pub collections: BTreeSet<String>,
}

/// `target` as the value a reference holds: a CBOR array of two 16-byte
/// byte strings, the space id and then the object id (decision 16).
pub fn value(target: Target) -> Value {
    Value::Array(vec![
        Value::Bytes(target.space.0.to_vec()),
        Value::Bytes(target.object.0.to_vec()),
    ])
}

/// The target `value` names, or `None` for a value that does not fit:
/// anything but an array of two 16-byte byte strings (decision 17).
pub fn target(value: &Value) -> Option<Target> {
    let Value::Array(items) = value else {
        return None;
    };
    let id = |item: &Value| match item {
        Value::Bytes(bytes) => <[u8; 16]>::try_from(bytes.as_slice()).ok(),
        _ => None,
    };
    match items.as_slice() {
        [space, object] => Some(Target {
            space: SpaceId(id(space)?),
            object: ObjectId(id(object)?),
        }),
        _ => None,
    }
}

/// What a member's copies hold of a reference's target (RFC 0004,
/// "How a reference resolves"). RFC 0005's "References into another space"
/// names the states for the app
/// `"ok"`, `"notAccessible"` and `"missing"`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// The member holds a copy of the target's space, and it holds the
    /// target, not deleted, in a collection the field lists.
    Ok,
    /// The member holds no copy of the target's space (decision 20).
    NotAccessible,
    /// The member holds a copy of the target's space, but not the target,
    /// or holds it deleted, or in a collection the field does not list
    /// (decision 18). A value that names no target reads missing too
    /// (decision 17).
    Missing,
}

/// The copies of the spaces a member holds, which references resolve
/// against.
pub trait Copies {
    /// The objects of one space's copy.
    type Objects: Objects;
    /// The member's copy of `space`, if it holds one.
    fn copy(&self, space: &SpaceId) -> Option<&Self::Objects>;
}

impl<O: Objects> Copies for BTreeMap<SpaceId, O> {
    type Objects = O;

    fn copy(&self, space: &SpaceId) -> Option<&O> {
        self.get(space)
    }
}

/// One write of a reference with its target's status.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resolution {
    /// The write, as basic values read it.
    pub entry: basic::Entry,
    /// What the member's copies hold of its target.
    pub status: Status,
}

/// What the app reads of a reference: the basic value read, resolved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resolved {
    /// The winning reference, or `None` when no write is live.
    pub value: Option<Resolution>,
    /// The losing live references, as basic values list them.
    pub conflicts: Vec<Resolution>,
}

/// The status of the target `value` names, in a field that lists
/// `collections`. Within the target's space it reads as a link does, and
/// everything but ok is missing.
fn status(collections: &BTreeSet<String>, value: &Value, copies: &impl Copies) -> Status {
    let Some(target) = target(value) else {
        return Status::Missing;
    };
    let Some(objects) = copies.copy(&target.space) else {
        return Status::NotAccessible;
    };
    match link::status(collections, &link::value(target.object), objects) {
        link::Status::Ok => Status::Ok,
        _ => Status::Missing,
    }
}

impl Reference {
    /// A new write of the reference to `target`, in any space, the
    /// writer's own included (decision 19), or a clear with `None`. It
    /// replaces what `state` holds: the winner and every conflict.
    pub fn set(
        &self,
        state: &basic::State,
        id: OperationId,
        author: MemberId,
        stamp: Hlc,
        target: Option<Target>,
    ) -> Write {
        let value = target.map_or(Value::Null, value);
        Values::References
            .set(state, id, author, stamp, value)
            .expect("a reference or the empty value is a reference's value")
    }

    /// The read value, with each target's status in the member's `copies`.
    pub fn resolve(&self, state: &basic::State, copies: &impl Copies) -> Resolved {
        let read = Values::References.read(state);
        let resolve = |entry: basic::Entry| Resolution {
            status: status(&self.collections, &entry.value, copies),
            entry,
        };
        Resolved {
            value: read.value.map(resolve),
            conflicts: read.conflicts.into_iter().map(resolve).collect(),
        }
    }
}

impl FieldType for Reference {
    type State = basic::State;
    type Operation = Write;
    type Read = basic::Read;

    fn apply(&self, state: &mut basic::State, op: &Write) {
        state.insert(op);
    }

    fn state_bytes(&self, state: &basic::State) -> Vec<u8> {
        state.bytes()
    }

    fn read(&self, state: &basic::State) -> basic::Read {
        Values::References.read(state)
    }

    fn outcome(&self, state: &basic::State, id: OperationId) -> Option<Outcome> {
        state.outcome(id)
    }
}
