//! The part of a member's model the local view needs (RFC 0006): each
//! collection with its fields and their field types, and the declared
//! indexes, each with its collection, its ordered fields with a direction
//! each, and how many leading fields a query fixes with equality.
//!
//! The core has no `model.lock.json` reader yet (RFC 0005); until it has,
//! a caller builds this value itself. It is a working name of the core,
//! not the SDK's API.

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};

use crate::basic::{self, BasicValue, Kind};
use crate::cbor::{self, Key, Value};
use crate::link::{Link, Links};
use crate::set::{self, Set};

/// The model the view folds by: collections by name, each a map of field
/// names to field types, and the declared indexes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Model {
    /// The collections, by name, with their fields by name.
    pub collections: BTreeMap<String, BTreeMap<String, Field>>,
    /// The declared indexes.
    pub indexes: Vec<Index>,
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
}

/// The merge state of one field, as its field type keeps it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FieldState {
    /// The state of a basic value or a link.
    Basic(basic::State),
    /// The state of a set or of links.
    Set(set::State),
}

/// A declared index (RFC 0006, "Declared indexes").
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Index {
    /// The index's name, unique in the model.
    pub name: String,
    /// The collection it orders.
    pub collection: String,
    /// The fields it orders by, each with its direction. Each is a field
    /// with a single sortable value: a basic value or a link.
    pub keys: Vec<(String, Direction)>,
    /// How many leading keys a query fixes with equality.
    pub equality: usize,
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
    /// Two indexes share this name.
    DuplicateIndex(String),
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
        matches!(self, Field::Basic(_) | Field::Link(_))
    }

    /// The state of the field nobody has written.
    pub fn empty(&self) -> FieldState {
        match self {
            Field::Basic(_) | Field::Link(_) => FieldState::Basic(basic::State::default()),
            Field::Set(_) | Field::Links(_) => FieldState::Set(set::State::default()),
        }
    }
}

impl Model {
    /// The field type of a field.
    pub fn field(&self, collection: &str, field: &str) -> Option<&Field> {
        self.collections.get(collection)?.get(field)
    }

    /// The declared index with this name.
    pub fn index(&self, name: &str) -> Option<&Index> {
        self.indexes.iter().find(|index| index.name == name)
    }

    /// Checks that every index names a collection, orders by fields with a
    /// column, and fixes at most its keys with equality.
    pub fn validate(&self) -> Result<(), Invalid> {
        for (n, index) in self.indexes.iter().enumerate() {
            let name = || index.name.clone();
            if self.indexes[..n].iter().any(|i| i.name == index.name) {
                return Err(Invalid::DuplicateIndex(name()));
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
