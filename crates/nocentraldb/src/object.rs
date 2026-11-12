//! An object as a read returns it (RFC 0009, "Get by id"): its id, its
//! space, whether it is deleted, and its fields by name. Its `Debug` output
//! shows the ids and the names of the set fields, never a value.

use std::collections::BTreeMap;

use ncdb_core::cbor;
use ncdb_core::field::FieldType;
use ncdb_core::ids::{ObjectId as CoreObjectId, SpaceId as CoreSpaceId};
use ncdb_core::set;
use ncdb_core::view::{Field, FieldState, Row};

use ncdb_core::personal::{self, Place, Sealed};

use crate::db::{Inner, SpaceState};
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
    /// The person id of `$person`, when the object is a person (RFC 0005,
    /// "Identifying the person").
    person: Option<[u8; 16]>,
    /// The names of its personal fields whose person is erased.
    erased: Vec<String>,
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
    /// Every field of the collection, with its value when it is set; for
    /// the TypeScript SDK's bridge (NOC-176).
    pub(crate) fn fields(&self) -> &BTreeMap<String, Option<Value>> {
        &self.fields
    }

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

    /// The names of the object's personal fields whose person is erased,
    /// provisionally or for good: each reads as unset (RFC 0009, "How a
    /// personal field reads"; `$erased` in TypeScript). A field whose key
    /// this device does not hold yet reads as unset too, but is not named.
    pub fn erased(&self) -> &[String] {
        &self.erased
    }

    /// The person id of the object, when it is a person.
    pub(crate) fn person_id(&self) -> Option<[u8; 16]> {
        self.person
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
        let state = inner.spaces.get(&space);
        let mut erased = Vec::new();
        let fields = model
            .iter()
            .filter(|(name, _)| !name.starts_with('$'))
            .map(|(name, field)| {
                if inner.model.person(collection, name).is_none() {
                    return (name.clone(), value(field, &row, name));
                }
                let place = Place {
                    space,
                    object,
                    person_key: [0; 16],
                    collection,
                    field: name,
                };
                let read = personal_value(field, &row, place, state);
                if let Personal::Erased = read {
                    erased.push(name.clone());
                }
                let value = match read {
                    Personal::Value(value) => value,
                    Personal::Erased => None,
                };
                (name.clone(), value)
            })
            .collect();
        let person = crate::personal::held_person(&inner.view, space, collection, object)?
            .map(|person| person.person);
        Ok(Some(Object {
            id: ObjectId(object),
            space: SpaceId(space),
            deleted: row.object.is_deleted(),
            fields,
            person,
            erased,
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

/// What a personal field reads.
enum Personal {
    /// Its value, or `None`: unset, cleared, its key not held yet, or a
    /// value that does not open.
    Value(Option<Value>),
    /// Its person is erased.
    Erased,
}

/// The read value of the personal field at `place` of `row`: the winning
/// write opened with the person key it names, which `state` holds (RFC
/// 0009, "How a personal field reads").
fn personal_value(
    field: &Field,
    row: &Row,
    mut place: Place<'_>,
    state: Option<&SpaceState>,
) -> Personal {
    let (Field::Basic(basic), Some(FieldState::Basic(written))) =
        (field, row.fields.get(place.field))
    else {
        return Personal::Value(None);
    };
    let Some(sealed) = basic
        .read(written)
        .value
        .and_then(|entry| Sealed::from_value(&entry.value))
    else {
        return Personal::Value(None);
    };
    let Some(state) = state else {
        return Personal::Value(None);
    };
    let resolved = state.checks.resolved();
    if resolved
        .person_key(sealed.person_key)
        .is_some_and(|key| key.erased())
    {
        return Personal::Erased;
    }
    let Some(key) = state.person_keys.get(&sealed.person_key) else {
        return Personal::Value(None);
    };
    place.person_key = sealed.person_key;
    let value = personal::open(key, &place, &sealed.bytes)
        .ok()
        .filter(|value| basic.kind.allows(value))
        .and_then(|value| Value::from_cbor(&value, false));
    Personal::Value(value)
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
