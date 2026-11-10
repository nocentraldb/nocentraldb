//! Links to other objects of the same space, as RFC 0004 ("Links") defines
//! them. A link (one target) is a basic value whose value is an object id;
//! links (many targets) are a set of object ids. Both merge by the code of
//! basic values and sets, over object ids (16 bytes) instead of a `Kind`. Neither has merge rules of
//! its own. What is new is resolution: whether the copy holds the target,
//! worked out when the app reads, never kept in the state.

use std::collections::{BTreeMap, BTreeSet};

use crate::basic::{self, Entry, Values, Write};
use crate::cbor::Value;
use crate::clock::Hlc;
use crate::field::{FieldType, Outcome};
use crate::ids::{MemberId, ObjectId, OperationId, SpaceId};
use crate::object::Object;
use crate::set::{self, Edit, Element};

/// The object a member links to, with the space it lives in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Target {
    /// The target's space.
    pub space: SpaceId,
    /// The target's object id.
    pub object: ObjectId,
}

/// A link into another space, which the writing side refuses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OtherSpace;

impl core::fmt::Display for OtherSpace {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("a link cannot point into another space")
    }
}

impl std::error::Error for OtherSpace {}

/// Whether the copy can read a link's target (RFC 0004, "Resolution").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// The copy holds the target, not deleted, in an allowed collection.
    Ok,
    /// The copy holds the target and it is deleted.
    Deleted,
    /// The copy holds no object with this id.
    Missing,
    /// The copy holds the target, in a collection the field does not allow.
    WrongCollection,
}

/// The objects of one space's copy that links resolve against.
pub trait Objects {
    /// The object with this id, if the copy holds any change of it.
    fn object(&self, id: &ObjectId) -> Option<&Object>;
}

impl Objects for BTreeMap<ObjectId, Object> {
    fn object(&self, id: &ObjectId) -> Option<&Object> {
        self.get(id)
    }
}

/// An object id as the value a link holds.
pub fn value(object: ObjectId) -> Value {
    Value::Bytes(object.0.to_vec())
}

/// The target's object id, when it lives in `space`.
fn in_space(space: SpaceId, target: Target) -> Result<ObjectId, OtherSpace> {
    if target.space == space {
        Ok(target.object)
    } else {
        Err(OtherSpace)
    }
}

/// The status of the target `value` names, in a field that allows
/// `collections`. A value that is no object id names no object the copy can
/// hold. A deleted target reads deleted whatever its collection.
pub(crate) fn status(
    collections: &BTreeSet<String>,
    value: &Value,
    objects: &impl Objects,
) -> Status {
    let object = match value {
        Value::Bytes(bytes) => <[u8; 16]>::try_from(bytes.as_slice())
            .ok()
            .and_then(|id| objects.object(&ObjectId(id))),
        _ => None,
    };
    let Some(object) = object.filter(|o| o.exists()) else {
        return Status::Missing;
    };
    if object.is_deleted() {
        return Status::Deleted;
    }
    match object.collection().value.map(|w| w.value) {
        Some(Value::Text(collection)) if collections.contains(&collection) => Status::Ok,
        _ => Status::WrongCollection,
    }
}

/// The link field type: one target, in one of `collections`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link {
    /// The collections the schema allows the target in.
    pub collections: BTreeSet<String>,
}

/// One write of a link with its target's status.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resolution {
    /// The write, as basic values read it.
    pub entry: Entry,
    /// Whether the copy can read its target.
    pub status: Status,
}

/// What the app reads of a link: the basic value read, resolved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resolved {
    /// The winning target, or `None` when no write is live.
    pub value: Option<Resolution>,
    /// The losing live targets, as basic values list them.
    pub conflicts: Vec<Resolution>,
}

impl Link {
    /// A new write of the link to `target`, or a clear with `None`, from a
    /// member writing in `space`. Refuses a target in another space.
    pub fn set(
        &self,
        state: &basic::State,
        id: OperationId,
        author: MemberId,
        stamp: Hlc,
        space: SpaceId,
        target: Option<Target>,
    ) -> Result<Write, OtherSpace> {
        let value = match target {
            Some(target) => value(in_space(space, target)?),
            None => Value::Null,
        };
        Ok(Values::ObjectIds
            .set(state, id, author, stamp, value)
            .expect("an object id or the empty value is a link's value"))
    }

    /// The read value, with each target's status in `objects`.
    pub fn resolve(&self, state: &basic::State, objects: &impl Objects) -> Resolved {
        let read = Values::ObjectIds.read(state);
        let resolve = |entry: Entry| Resolution {
            status: status(&self.collections, &entry.value, objects),
            entry,
        };
        Resolved {
            value: read.value.map(resolve),
            conflicts: read.conflicts.into_iter().map(resolve).collect(),
        }
    }
}

impl FieldType for Link {
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
        Values::ObjectIds.read(state)
    }

    fn outcome(&self, state: &basic::State, id: OperationId) -> Option<Outcome> {
        state.outcome(id)
    }
}

/// The links field type: many targets, each in one of `collections`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Links {
    /// The collections the schema allows the targets in.
    pub collections: BTreeSet<String>,
}

/// One target of a links field with its status.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Linked {
    /// The target, as sets read an element.
    pub element: Element,
    /// Whether the copy can read it.
    pub status: Status,
}

impl Links {
    /// A new addition of `target`, from a member writing in `space`.
    /// Refuses a target in another space.
    pub fn add(
        &self,
        state: &set::State,
        id: OperationId,
        author: MemberId,
        stamp: Hlc,
        space: SpaceId,
        target: Target,
    ) -> Result<Edit, OtherSpace> {
        let element = value(in_space(space, target)?);
        Ok(
            set::add(Values::ObjectIds, state, id, author, stamp, element)
                .expect("an object id is an element of links"),
        )
    }

    /// A new removal of `target`, wherever it points: removing a link that
    /// should never have been written is allowed.
    pub fn remove(
        &self,
        state: &set::State,
        id: OperationId,
        author: MemberId,
        stamp: Hlc,
        target: ObjectId,
    ) -> Edit {
        set::remove(Values::ObjectIds, state, id, author, stamp, value(target))
            .expect("an object id is an element of links")
    }

    /// The targets, each with its status in `objects`.
    pub fn resolve(&self, state: &set::State, objects: &impl Objects) -> Vec<Linked> {
        set::read(Values::ObjectIds, state)
            .elements
            .into_iter()
            .map(|element| Linked {
                status: status(&self.collections, &element.value, objects),
                element,
            })
            .collect()
    }
}

impl FieldType for Links {
    type State = set::State;
    type Operation = Edit;
    type Read = set::Read;

    fn apply(&self, state: &mut set::State, op: &Edit) {
        state.insert(op);
    }

    fn state_bytes(&self, state: &set::State) -> Vec<u8> {
        state.bytes()
    }

    fn read(&self, state: &set::State) -> set::Read {
        set::read(Values::ObjectIds, state)
    }

    fn outcome(&self, state: &set::State, id: OperationId) -> Option<Outcome> {
        state.outcome(id)
    }
}
