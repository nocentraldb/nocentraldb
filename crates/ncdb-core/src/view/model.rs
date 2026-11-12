//! The part of a member's model the local view needs (RFC 0006): each
//! collection with its fields and their field types, and the declared
//! indexes, each with its collection, its ordered fields with a direction
//! each, and how many leading fields a query fixes with equality.
//!
//! The view reads it only from the member's `model.lock.json` (RFC 0005),
//! through [`Model::from_lock`]. It is a working name of the core, not the
//! SDK's API.

use std::collections::{BTreeMap, BTreeSet};

use sha2::{Digest, Sha256};

use crate::basic::{self, BasicValue, Kind};
use crate::cbor::{self, Key, Value};
use crate::link::{Link, Links};
use crate::model_lock::Refusal;
pub use crate::model_lock::Scope;
use crate::reference::Reference;
use crate::set::{self, Set};

/// The model the view folds by: collections by name, each a map of field
/// names to field types, and the declared indexes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Model {
    /// The `model.id` of the lock, which the core writes into every sealed
    /// payload and compares with key 64 of every payload it receives.
    pub id: [u8; 16],
    /// The `model.version` of the lock.
    pub version: u64,
    /// The collections, by name, with their fields by name.
    pub collections: BTreeMap<String, BTreeMap<String, Field>>,
    /// The declared indexes.
    pub indexes: Vec<Index>,
    /// The fields of each collection that the lock names with a plugin
    /// type no code implements: their operations park as "unknown type"
    /// (RFC 0011, "A type that is not registered").
    pub unregistered: BTreeMap<String, BTreeSet<String>>,
}

/// A field and its field type (RFC 0004).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Field {
    /// A basic value.
    Basic(BasicValue),
    /// A link to one object.
    Link(Link),
    /// A set of basic values.
    Set(Set),
    /// Links to many objects.
    Links(Links),
    /// A reference to one object, in any space (RFC 0004, "References").
    Ref(Reference),
}

/// The merge state of one field, as its field type keeps it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FieldState {
    /// The state of a basic value, a link or a reference.
    Basic(basic::State),
    /// The state of a set or of links.
    Set(set::State),
}

/// A declared index (RFC 0006, "Declared indexes").
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Index {
    /// The index's name, unique in its collection: two collections may
    /// each have an index of the same name, as the lock scopes them.
    pub name: String,
    /// The collection it orders.
    pub collection: String,
    /// The fields it orders by, each with its direction. Each is a field
    /// with a single sortable value: a basic value, a link or a reference.
    pub keys: Vec<(String, Direction)>,
    /// How many leading keys a query fixes with equality, after the space
    /// id when the index runs in one space.
    pub equality: usize,
    /// Whether the index spans every space of the member, or runs in one
    /// and puts the space id first (RFC 0006, "Declared indexes").
    pub scope: Scope,
}

/// The direction of one key of an index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    /// Smallest first; an unset value first.
    Ascending,
    /// Largest first; an unset value last.
    Descending,
}

/// Why a model cannot be folded into a view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Invalid {
    /// The lock breaks the rules RFC 0005 gives a lock on its own.
    Lock(Vec<Refusal>),
    /// Two indexes of one collection share a name.
    DuplicateIndex {
        /// The collection.
        collection: String,
        /// The index's name.
        index: String,
    },
    /// The index names a collection the model does not have.
    UnknownCollection(String),
    /// The index orders by a field the collection does not have, or by one
    /// without a single sortable value.
    UnsortableKey(String),
    /// The index fixes more keys with equality than it has.
    Equality(String),
}

impl Field {
    /// Whether the field has a column in `c_<collection>`: a single value
    /// that sorts (RFC 0006, "Where the view lives").
    pub fn has_column(&self) -> bool {
        matches!(self, Field::Basic(_) | Field::Link(_) | Field::Ref(_))
    }

    /// The state of the field nobody has written.
    pub fn empty(&self) -> FieldState {
        match self {
            Field::Basic(_) | Field::Link(_) | Field::Ref(_) => {
                FieldState::Basic(basic::State::default())
            }
            Field::Set(_) | Field::Links(_) => FieldState::Set(set::State::default()),
        }
    }
}

impl Model {
    /// The field type of a field.
    pub fn field(&self, collection: &str, field: &str) -> Option<&Field> {
        self.collections.get(collection)?.get(field)
    }

    /// Whether the lock names `field` of `collection` with a plugin type
    /// no code implements.
    pub fn unregistered(&self, collection: &str, field: &str) -> bool {
        self.unregistered
            .get(collection)
            .is_some_and(|fields| fields.contains(field))
    }

    /// The declared index of `collection` with this name.
    pub fn index(&self, collection: &str, name: &str) -> Option<&Index> {
        self.indexes
            .iter()
            .find(|index| index.collection == collection && index.name == name)
    }

    /// Checks that every index has a name no other index of its collection
    /// has, names a collection, orders by fields with a column, and fixes
    /// at most its keys with equality.
    pub fn validate(&self) -> Result<(), Invalid> {
        for (n, index) in self.indexes.iter().enumerate() {
            let name = || index.name.clone();
            let twin = |i: &Index| i.collection == index.collection && i.name == index.name;
            if self.indexes[..n].iter().any(twin) {
                return Err(Invalid::DuplicateIndex {
                    collection: index.collection.clone(),
                    index: name(),
                });
            }
            let Some(fields) = self.collections.get(&index.collection) else {
                return Err(Invalid::UnknownCollection(name()));
            };
            let sortable =
                |(key, _): &(String, Direction)| fields.get(key).is_some_and(Field::has_column);
            if !index.keys.iter().all(sortable) {
                return Err(Invalid::UnsortableKey(name()));
            }
            if index.equality > index.keys.len() {
                return Err(Invalid::Equality(name()));
            }
        }
        Ok(())
    }

    /// The SHA-256 of the model's canonical CBOR: equal models, equal
    /// hashes. The view stores it in `meta`, and a page cursor carries it.
    pub fn hash(&self) -> [u8; 32] {
        let text = |s: &str| Value::Text(s.to_owned());
        let collections = self
            .collections
            .iter()
            .map(|(name, fields)| {
                let fields = fields
                    .iter()
                    .map(|(name, field)| (Key::Text(name.clone()), field.describe()))
                    .collect();
                (Key::Text(name.clone()), Value::Map(fields))
            })
            .collect();
        let indexes = self
            .indexes
            .iter()
            .map(|index| {
                let keys = index
                    .keys
                    .iter()
                    .map(|(key, direction)| {
                        let descending = *direction == Direction::Descending;
                        Value::Array(vec![text(key), Value::Bool(descending)])
                    })
                    .collect();
                Value::Array(vec![
                    text(&index.name),
                    text(&index.collection),
                    Value::Array(keys),
                    Value::Unsigned(index.equality as u64),
                    Value::Bool(index.scope == Scope::Space),
                ])
            })
            .collect();
        let model = Value::Array(vec![Value::Map(collections), Value::Array(indexes)]);
        Sha256::digest(cbor::encode(&model)).into()
    }
}

impl Field {
    /// The field type as a CBOR value, for the model's hash.
    fn describe(&self) -> Value {
        let collections = |c: &std::collections::BTreeSet<String>| {
            Value::Array(c.iter().map(|c| Value::Text(c.clone())).collect())
        };
        match self {
            Field::Basic(t) => Value::Array(vec![Value::Unsigned(0), describe(&t.kind)]),
            Field::Link(t) => Value::Array(vec![Value::Unsigned(1), collections(&t.collections)]),
            Field::Set(t) => Value::Array(vec![Value::Unsigned(2), describe(&t.kind)]),
            Field::Links(t) => Value::Array(vec![Value::Unsigned(3), collections(&t.collections)]),
            Field::Ref(t) => Value::Array(vec![Value::Unsigned(4), collections(&t.collections)]),
        }
    }
}

/// A kind of basic value as a CBOR value, for the model's hash.
fn describe(kind: &Kind) -> Value {
    match kind {
        Kind::Text => Value::Unsigned(0),
        Kind::Integer => Value::Unsigned(1),
        Kind::Float => Value::Unsigned(2),
        Kind::Boolean => Value::Unsigned(3),
        Kind::Choice(options) => {
            Value::Array(options.iter().map(|o| Value::Text(o.clone())).collect())
        }
    }
}
