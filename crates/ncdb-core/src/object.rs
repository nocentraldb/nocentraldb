//! What a copy knows of one object, as far as links need it to resolve
//! (RFC 0004, "Links"): whether any create of its id has arrived, its
//! collection, and whether it is deleted. Both are basic values, so two
//! creates of one id and a deletion converge by the rule of basic values.
//!
//! Object deletion belongs to a later RFC; this follows owner decision 4 of
//! RFC 0004: whether an object is deleted is a boolean basic value.

use crate::basic::{self, BasicValue, Kind, Write};
use crate::cbor::Value;
use crate::clock::Hlc;
use crate::field::FieldType;
use crate::ids::{MemberId, OperationId};

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

/// One object of a copy: its creates and its deletion.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Object {
    creates: basic::State,
    deleted: basic::State,
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

    /// Whether any create of the id has arrived.
    pub fn exists(&self) -> bool {
        self.creates != basic::State::default()
    }

    /// The object's collection, as a basic value: the create last in stamp
    /// order wins; creates that name another collection are conflicts.
    pub fn collection(&self) -> basic::Read {
        COLLECTION.read(&self.creates)
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
