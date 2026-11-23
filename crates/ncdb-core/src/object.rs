//! What a copy knows of one object, as far as links need it to resolve
//! (RFC 0004, "Links"): whether any create of its id has arrived, its
//! collection, whether it is deleted, and where it moved. All three are
//! basic values, so two creates of one id, a deletion and two moves at
//! once converge by the rule of basic values.
//!
//! Whether an object is deleted is a boolean basic value (RFC 0004,
//! "Creating and deleting an object"); where it moved is a basic value of
//! space ids, independent of the deletion (RFC 0016, "Formats"). Creates
//! (operation 0), deletes (operation 4) and moves (operation 6) reach it
//! as [`ObjectOperation`]s, which `field_op` decodes and the fold of
//! `receive` applies. Who may delete, and what a deleted object shows,
//! belong to a later RFC.

use crate::basic::{self, BasicValue, Kind, Values, Write};
use crate::cbor::Value;
use crate::clock::Hlc;
use crate::field::FieldType;
use crate::ids::{MemberId, OperationId, SpaceId};

/// A create of an object id (RFC 0002's operation 0), as far as RFC 0004
/// gives it a meaning: it puts the object into a collection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Create {
    /// The operation's id.
    pub id: OperationId,
    /// The member that signed the change.
    pub author: MemberId,
    /// The change's clock.
    pub stamp: Hlc,
    /// The collection the create puts the object into.
    pub collection: String,
}

/// An operation on an object rather than on one of its fields (RFC 0004,
/// "Creating and deleting an object").
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ObjectOperation {
    /// Operation 0: a create of the object's id.
    Create(Create),
    /// Operation 4: a write of whether the object is deleted.
    Delete(Write),
    /// Operation 6: a write of the space the object moved to, or `null`
    /// to take a move back (RFC 0016, "Formats").
    Move(Write),
}

/// One object of a copy: its creates, its deletion and its move.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Object {
    creates: basic::State,
    deleted: basic::State,
    moved: basic::State,
}

/// The collection as a basic value: any name.
const COLLECTION: BasicValue = BasicValue { kind: Kind::Text };
/// Whether the object is deleted, as a basic value.
const DELETED: BasicValue = BasicValue {
    kind: Kind::Boolean,
};

impl Object {
    /// Merges one create of the object's id. A create replaces nothing, and
    /// neither deletes nor restores.
    pub fn create(&mut self, create: &Create) {
        let write = Write {
            id: create.id,
            author: create.author,
            stamp: create.stamp,
            value: Value::Text(create.collection.clone()),
            replaces: Default::default(),
        };
        COLLECTION.apply(&mut self.creates, &write);
    }

    /// Merges one write of whether the object is deleted: `true` deletes,
    /// `false` restores.
    pub fn set_deleted(&mut self, write: &Write) {
        DELETED.apply(&mut self.deleted, write);
    }

    /// Merges one write of where the object moved: a space id moves it,
    /// `null` takes the move back (RFC 0016, "Formats").
    pub fn set_moved(&mut self, write: &Write) {
        self.moved.insert(write);
    }

    /// Merges one operation on the object.
    pub fn apply(&mut self, operation: &ObjectOperation) {
        match operation {
            ObjectOperation::Create(create) => self.create(create),
            ObjectOperation::Delete(write) => self.set_deleted(write),
            ObjectOperation::Move(write) => self.set_moved(write),
        }
    }

    /// Whether any create of the id has arrived.
    pub fn exists(&self) -> bool {
        self.creates != basic::State::default()
    }

    /// The object's collection, as a basic value: the create last in stamp
    /// order wins; creates that name another collection are conflicts.
    pub fn collection(&self) -> basic::Read {
        COLLECTION.read(&self.creates)
    }

    /// The state of the object's deletion, as a basic value of booleans: a
    /// member that deletes or restores the object replaces what it holds.
    pub fn deletion(&self) -> &basic::State {
        &self.deleted
    }

    /// The object's move, as a basic value of space ids: the winning
    /// write is where it went, and a losing concurrent move stays a
    /// conflict (RFC 0016, "Two moves at once").
    pub fn moving(&self) -> basic::Read {
        Values::ObjectIds.read(&self.moved)
    }

    /// The space the object moved to: the winning write of its move, when
    /// that names a 16-byte space id. `null`, a take-back, and a value of
    /// any other kind read as not moved.
    pub fn moved_to(&self) -> Option<SpaceId> {
        match self.moving().value?.value {
            Value::Bytes(bytes) => bytes.try_into().ok().map(SpaceId),
            _ => None,
        }
    }

    /// Whether the object is deleted: the winning write of its deletion is
    /// `true`. Anything else, a value of the wrong kind included, reads as
    /// not deleted.
    pub fn is_deleted(&self) -> bool {
        DELETED
            .read(&self.deleted)
            .value
            .is_some_and(|w| w.value == Value::Bool(true))
    }
}
