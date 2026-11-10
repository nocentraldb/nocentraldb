//! An object as a read returns it (RFC 0009, "Get by id"): its id, its
//! space, whether it is deleted, and its fields by name. Its `Debug` output
//! shows the ids and the names of the set fields, never a value.

use std::collections::BTreeMap;

use ncdb_core::cbor;
use ncdb_core::field::FieldType;
use ncdb_core::ids::{ObjectId as CoreObjectId, SpaceId as CoreSpaceId};
use ncdb_core::set;
use ncdb_core::view::{Field, FieldState, Row};

use crate::db::Inner;
use crate::error::{Error, ErrorCode, storage};
use crate::id::{ObjectId, SpaceId};
use crate::value::{Repr, Value};

/// One object read from the view.
#[derive(Clone, PartialEq)]
pub struct Object {
    id: ObjectId,
    space: SpaceId,
    deleted: bool,
    /// Every field of the collection, with its value when it is set.
    fields: BTreeMap<String, Option<Value>>,
}

impl std::fmt::Debug for Object {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let set: Vec<&String> = self
            .fields
            .iter()
            .filter_map(|(name, value)| value.as_ref().map(|_| name))
            .collect();
        f.debug_struct("Object")
            .field("id", &self.id)
            .field("space", &self.space)
            .field("deleted", &self.deleted)
            .field("set", &set)
            .finish()
    }
}

impl Object {
    /// The object id.
    pub fn id(&self) -> ObjectId {
        self.id
    }

    /// The space id.
    pub fn space(&self) -> SpaceId {
        self.space
    }

    /// Whether the object is deleted.
    pub fn deleted(&self) -> bool {
        self.deleted
    }

    /// The value of the field `name`, as `T`, or `None` when the field is
    /// unset. A field the collection does not have fails with `NotFound`,
    /// a value that is no `T` with `InvalidValue`.
    pub fn get<'a, T>(&'a self, name: &str) -> Result<Option<T>, Error>
    where
        T: TryFrom<&'a Value, Error = Error>,
    {
        match self.fields.get(name) {
            None => Err(Error::new(
                ErrorCode::NotFound,
                format!("the object's collection has no field {name}"),
            )),
            Some(None) => Ok(None),
            Some(Some(value)) => T::try_from(value).map(Some),
        }
    }

    /// Reads `object` of `collection` in `space` from the view, or `None`
    /// when the copy holds no create of it in that collection.
    pub(crate) fn read(
        inner: &Inner,
        space: CoreSpaceId,
        collection: &str,
        object: CoreObjectId,
    ) -> Result<Option<Object>, Error> {
        let Some(row) = inner.view.get(space, collection, object).map_err(storage)? else {
            return Ok(None);
        };
        let in_collection = row.object.collection().value.map(|entry| entry.value)
            == Some(cbor::Value::Text(collection.to_owned()));
        if !in_collection {
            return Ok(None);
        }
        let model = &inner.model.collections[collection];
        let fields = model
            .iter()
            .map(|(name, field)| (name.clone(), value(field, &row, name)))
            .collect();
        Ok(Some(Object {
            id: ObjectId(object),
            space: SpaceId(space),
            deleted: row.object.is_deleted(),
            fields,
        }))
    }
}

/// The read value of the field `name` of `row`: the winning write, when
/// the reader's schema allows it.
fn value(field: &Field, row: &Row, name: &str) -> Option<Value> {
    let state = row.fields.get(name)?;
    match (field, state) {
        (Field::Basic(basic), FieldState::Basic(state)) => {
            let entry = basic.read(state).value.filter(|entry| entry.known)?;
            Value::from_cbor(&entry.value, false)
        }
        (Field::Link(link), FieldState::Basic(state)) => {
            let entry = link.read(state).value.filter(|entry| entry.known)?;
            Value::from_cbor(&entry.value, true)
        }
        (Field::Set(set), FieldState::Set(state)) => Some(list(set.read(state), false)),
        (Field::Links(links), FieldState::Set(state)) => Some(list(links.read(state), true)),
        _ => None,
    }
}

/// The elements of a set or of many links, in the order of their canonical
/// bytes. An element the reader's schema does not allow is left out.
fn list(read: set::Read, ids: bool) -> Value {
    let elements = read
        .elements
        .iter()
        .filter(|element| element.known)
        .filter_map(|element| Value::from_cbor(&element.value, ids))
        .collect();
    Value(Repr::List(elements))
}
