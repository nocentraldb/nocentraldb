//! Field values between the bridge's protocol and the SDK's [`Value`]
//! (RFC 0009, "Field operations per field type"): what a write's `values`
//! mean by the field's type in the lock, and how a read field answers.

use std::collections::BTreeMap;
use std::str::FromStr;

use ncdb_core::basic::{BasicValue, Kind};
use ncdb_core::cbor::{Key, Value};
use ncdb_core::set::Set;
use ncdb_core::view::{Field as ModelField, Model};

use super::{Failed, entries, invalid};
use crate::id::{ObjectId, SpaceId};
use crate::object::Object;
use crate::value::{Reference, Repr, Value as Field};

/// The values of a create or an update of `collection`, by field name, as
/// the field's type reads them. A whole number for a float field, or for an
/// element of a set of floats, is that float: canonical CBOR writes 2.0 as
/// the integer 2. An object id for a link, or for an element of many
/// links, arrives as its 32 hex digits and is that object id (NOC-353). A
/// reference arrives as `{ space, id }`, as a read gives it (NOC-354).
pub(super) fn fields(
    model: &Model,
    collection: &str,
    map: &BTreeMap<Key, Value>,
) -> Result<Vec<(String, Field)>, Failed> {
    map.iter()
        .map(|(key, value)| match key {
            Key::Text(name) => {
                if let Some(ModelField::Ref(_)) = model.field(collection, name) {
                    return Ok((name.clone(), as_reference(value)?));
                }
                let value = field(value)?;
                let value = match model.field(collection, name) {
                    Some(ModelField::Basic(BasicValue { kind: Kind::Float }))
                    | Some(ModelField::Set(Set { kind: Kind::Float })) => as_float(value),
                    Some(ModelField::Link(_) | ModelField::Links(_)) => as_link(value)?,
                    _ => value,
                };
                Ok((name.clone(), value))
            }
            Key::Unsigned(_) => Err(invalid(crate::texts::bridge::FIELD_NAME_IS_TEXT)),
        })
        .collect()
}

/// `value` with each integer in it as a float.
fn as_float(value: Field) -> Field {
    let floats = |items: Vec<Field>| items.into_iter().map(as_float).collect();
    Field(match value.0 {
        Repr::Integer(n) => Repr::Float(n as f64),
        Repr::Add(items) => Repr::Add(floats(items)),
        Repr::Remove(items) => Repr::Remove(floats(items)),
        other => other,
    })
}

/// `value` with each text in it as the object id it spells; a text that
/// spells none is `invalidValue`.
fn as_link(value: Field) -> Result<Field, Failed> {
    let ids = |items: Vec<Field>| {
        items
            .into_iter()
            .map(as_link)
            .collect::<Result<Vec<_>, _>>()
    };
    Ok(Field(match value.0 {
        Repr::Text(text) => Repr::Id(
            ObjectId::from_str(&text)
                .map_err(|_| invalid(crate::texts::bridge::NOT_AN_OBJECT_ID))?,
        ),
        Repr::Add(items) => Repr::Add(ids(items)?),
        Repr::Remove(items) => Repr::Remove(ids(items)?),
        other => other,
    }))
}

/// The reference `{ space, id }` names, or the clear `null` asks for; any
/// other value is `invalidValue`.
fn as_reference(value: &Value) -> Result<Field, Failed> {
    let not_one = || invalid(crate::texts::bridge::NOT_AREFERENCE);
    let (space, id) = match value {
        Value::Null => return Ok(Field::clear()),
        Value::Map(map) if map.len() == 2 => (
            map.get(&Key::Text("space".into())),
            map.get(&Key::Text("id".into())),
        ),
        _ => return Err(not_one()),
    };
    match (space, id) {
        (Some(Value::Text(space)), Some(Value::Text(id))) => Ok(Field::from(Reference::new(
            SpaceId::from_str(space).map_err(|_| not_one())?,
            ObjectId::from_str(id).map_err(|_| not_one())?,
        ))),
        _ => Err(not_one()),
    }
}

/// A value to write, from its protocol form.
fn field(value: &Value) -> Result<Field, Failed> {
    Ok(match value {
        Value::Text(text) => Field::from(text.as_str()),
        Value::Unsigned(n) => Field::from(
            i64::try_from(*n).map_err(|_| invalid(crate::texts::bridge::NUMBER_TOO_LARGE))?,
        ),
        Value::Negative(n) => {
            let n =
                i64::try_from(*n).map_err(|_| invalid(crate::texts::bridge::NUMBER_TOO_SMALL))?;
            Field::from(-1 - n)
        }
        Value::Float(f) => Field::from(*f),
        Value::Bool(b) => Field::from(*b),
        Value::Map(map) => {
            let one = |key: &str| map.get(&Key::Text(key.into()));
            match (map.len(), one("$add"), one("$remove")) {
                (1, Some(Value::Array(items)), None) => Field(Repr::Add(elements(items)?)),
                (1, None, Some(Value::Array(items))) => Field(Repr::Remove(elements(items)?)),
                _ => return Err(invalid(crate::texts::bridge::NOT_AN_ELEMENTS_OP)),
            }
        }
        // `null` clears a basic field or a link (RFC 0009, "Field operations
        // per field type"; Carsten, 2026-11-12, NOC-277); the SDK refuses it
        // for a set or many links.
        Value::Null => Field::clear(),
        _ => return Err(invalid(crate::texts::bridge::NOT_AFIELD_VALUE)),
    })
}

pub(super) fn elements(items: &[Value]) -> Result<Vec<Field>, Failed> {
    items.iter().map(field).collect()
}

/// The protocol form of a value read from a field.
fn value_of(field: &Field) -> Value {
    match &field.0 {
        Repr::Text(text) => Value::Text(text.clone()),
        Repr::Integer(n) if *n >= 0 => Value::Unsigned(*n as u64),
        Repr::Integer(n) => Value::Negative((-1 - *n) as u64),
        Repr::Float(f) => Value::Float(*f),
        Repr::Boolean(b) => Value::Bool(*b),
        Repr::Id(id) => Value::Text(id.to_string()),
        Repr::Ref(reference) => entries([
            ("space", Value::Text(reference.space.to_string())),
            ("id", Value::Text(reference.id.to_string())),
        ]),
        Repr::List(items) | Repr::Add(items) | Repr::Remove(items) => {
            Value::Array(items.iter().map(value_of).collect())
        }
        // A plugin field's data passes through as its own CBOR value; how
        // the TypeScript SDK shows it comes with plugin fields there.
        Repr::Data(data) => data.to_value().unwrap_or(Value::Null),
        // A read never holds a clear; an unset field reads null.
        Repr::Clear => Value::Null,
    }
}

/// An object as `get` answers it.
pub(super) fn read(object: &Object) -> Value {
    let fields = object
        .fields()
        .iter()
        .map(|(name, value)| {
            let value = value.as_ref().map_or(Value::Null, value_of);
            (Key::Text(name.clone()), value)
        })
        .collect();
    entries([
        ("$id", Value::Text(object.id().to_string())),
        ("$space", Value::Text(object.space().to_string())),
        ("$deleted", Value::Bool(object.deleted())),
        ("fields", Value::Map(fields)),
    ])
}
