//! A reference into another space, as RFC 0004 defines it ("References"): the
//! field type `ncdb/ref`, rule version 1. It is a basic value whose value
//! is the target's space id and object id, written `[space id, object id]`,
//! and it merges by the code of basic values. It has no merge rule of its
//! own.
//!
//! What is new is resolution, worked out when the app reads, never kept in
//! the state. A reference into the space of the object that holds it
//! resolves as a link does, from that space's copy alone, so every member
//! of the space reads the same (RFC 0018, "One function, two cases"). A
//! reference into another space resolves against the member's own copies,
//! which differ from member to member, so nothing stored depends on it.

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
/// byte strings, the space id and then the object id (RFC 0004, "The
/// value and its encoding").
pub fn value(target: Target) -> Value {
    Value::Array(vec![
        Value::Bytes(target.space.0.to_vec()),
        Value::Bytes(target.object.0.to_vec()),
    ])
}

/// The target `value` names, or `None` for a value that does not fit:
/// anything but an array of two 16-byte byte strings (RFC 0004, "The
/// value and its encoding").
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

/// What a member's copies hold of a reference's target. A cross-space
/// reference reads ok, not accessible or missing (RFC 0004, "How a
/// reference resolves"); an in-space one reads a link's statuses: ok,
/// deleted, missing, wrong collection or moved (RFC 0018, "One function,
/// two cases").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// The member holds a copy of the target's space, and it holds the
    /// target, not deleted, in a collection the field lists.
    Ok,
    /// The member holds no copy of the target's space (RFC 0004, "How a
    /// reference resolves").
    NotAccessible,
    /// The member holds a copy of the target's space, but not the target,
    /// or, for a cross-space reference, holds it deleted, or in a
    /// collection the field does not list (RFC 0004, "How a reference
    /// resolves"). A value that names no target reads missing too (RFC
    /// 0004, "The value and its encoding").
    Missing,
    /// An in-space reference only: the space holds the target, deleted.
    Deleted,
    /// An in-space reference only: the space holds the target in a
    /// collection the field does not list.
    WrongCollection,
    /// An in-space reference only: the space holds the trace of the
    /// target's move to this space (RFC 0016, "Links to the moved
    /// object"). It is not followed, as a link's is not.
    Moved(SpaceId),
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
    /// The space whose copy holds the target, when it is ok: the one the
    /// reference names, or the one the traces of its moves led to (RFC
    /// 0016, "References to the moved object").
    pub found: Option<SpaceId>,
}

/// What the app reads of a reference: the basic value read, resolved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resolved {
    /// The winning reference, or `None` when no write is live.
    pub value: Option<Resolution>,
    /// The losing live references, as basic values list them.
    pub conflicts: Vec<Resolution>,
}

/// How many traces of moves in a row a reference follows, so that a cycle
/// of moves ends (RFC 0016, "References to the moved object").
pub const MAX_TRACES: usize = 8;

/// The status of the target `value` names, in a field that lists
/// `collections` and is held by an object in `space`, and the space whose
/// copy holds the target when it is ok. A reference into `space` reads as a
/// link does, from that copy alone. One into another space follows a trace
/// to the space its object moved to, up to [`MAX_TRACES`] in a row, and
/// everything but ok is missing.
fn status(
    space: SpaceId,
    collections: &BTreeSet<String>,
    value: &Value,
    copies: &impl Copies,
) -> (Status, Option<SpaceId>) {
    let Some(target) = target(value) else {
        return (Status::Missing, None);
    };
    let object = link::value(target.object);
    if target.space == space {
        let Some(objects) = copies.copy(&space) else {
            return (Status::NotAccessible, None);
        };
        return match link::status(collections, &object, objects) {
            link::Status::Ok => (Status::Ok, Some(space)),
            link::Status::Deleted => (Status::Deleted, None),
            link::Status::Missing => (Status::Missing, None),
            link::Status::WrongCollection => (Status::WrongCollection, None),
            link::Status::Moved(to) => (Status::Moved(to), None),
        };
    }
    let mut at = target.space;
    for _ in 0..=MAX_TRACES {
        let Some(objects) = copies.copy(&at) else {
            return (Status::NotAccessible, None);
        };
        match link::status(collections, &object, objects) {
            link::Status::Ok => return (Status::Ok, Some(at)),
            link::Status::Moved(to) => at = to,
            _ => return (Status::Missing, None),
        }
    }
    (Status::Missing, None)
}

impl Reference {
    /// A new write of the reference to `target`, in any space, the
    /// writer's own included (RFC 0004, "The value and its encoding"), or a
    /// clear with `None`. It
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

    /// The read value of the field held by an object in `space`, with each
    /// target's status in the member's `copies`.
    pub fn resolve(&self, space: SpaceId, state: &basic::State, copies: &impl Copies) -> Resolved {
        let read = Values::References.read(state);
        let resolve = |entry: basic::Entry| {
            let (status, found) = status(space, &self.collections, &entry.value, copies);
            Resolution {
                status,
                found,
                entry,
            }
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
