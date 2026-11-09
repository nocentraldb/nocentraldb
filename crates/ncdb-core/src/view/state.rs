//! The merge states the view keeps as opaque bytes (RFC 0006,
//! `fs_<collection>` and the objects), read back into the field types'
//! states. A field type's state is the set of operations it received, so
//! its canonical bytes, which [`FieldType::state_bytes`] writes, list them,
//! and reading them back inserts each one again.
//!
//! [`FieldType::state_bytes`]: crate::field::FieldType::state_bytes

use std::collections::{BTreeMap, BTreeSet};

use super::Error;
use crate::basic::{self, Write};
use crate::cbor::{self, Value};
use crate::clock::Hlc;
use crate::ids::{ChangeId, MemberId, OperationId};
use crate::object::{Create, Object, ObjectOperation};
use crate::set::{self, Action, Edit};

const CORRUPT: Error = Error::Corrupt("merge state");

/// The items of a state's top-level array.
fn items(bytes: &[u8]) -> Result<Vec<Value>, Error> {
    match cbor::decode(bytes).map_err(|_| CORRUPT)? {
        Value::Array(items) => Ok(items),
        _ => Err(CORRUPT),
    }
}

/// The fields of one item, which must have `n` of them.
fn fields<const N: usize>(item: Value) -> Result<[Value; N], Error> {
    match item {
        Value::Array(fields) => fields.try_into().map_err(|_| CORRUPT),
        _ => Err(CORRUPT),
    }
}

fn id(value: Value) -> Result<OperationId, Error> {
    let [change, position] = fields(value)?;
    match (change, position) {
        (Value::Bytes(change), Value::Unsigned(position)) => Ok(OperationId {
            change: ChangeId(change.try_into().map_err(|_| CORRUPT)?),
            position,
        }),
        _ => Err(CORRUPT),
    }
}

fn ids(value: Value) -> Result<BTreeSet<OperationId>, Error> {
    match value {
        Value::Array(items) => items.into_iter().map(id).collect(),
        _ => Err(CORRUPT),
    }
}

fn id_value(id: &OperationId) -> Value {
    Value::Array(vec![
        Value::Bytes(id.change.0.to_vec()),
        Value::Unsigned(id.position),
    ])
}

fn author(value: Value) -> Result<MemberId, Error> {
    match value {
        Value::Bytes(bytes) => Ok(MemberId(bytes.try_into().map_err(|_| CORRUPT)?)),
        _ => Err(CORRUPT),
    }
}

fn stamp(value: Value) -> Result<Hlc, Error> {
    match value {
        Value::Unsigned(n) => Ok(Hlc(n)),
        _ => Err(CORRUPT),
    }
}

/// A basic value's or a link's state from its canonical bytes.
pub(super) fn basic(bytes: &[u8]) -> Result<basic::State, Error> {
    let mut state = basic::State::default();
    for item in items(bytes)? {
        state.insert(&write(fields(item)?)?);
    }
    Ok(state)
}

/// A write from `[id, author, stamp, value, [replaced id…]]`.
fn write([id_, author_, stamp_, value, replaces]: [Value; 5]) -> Result<Write, Error> {
    Ok(Write {
        id: id(id_)?,
        author: author(author_)?,
        stamp: stamp(stamp_)?,
        value,
        replaces: ids(replaces)?,
    })
}

/// A set's or links' state from its canonical bytes.
pub(super) fn set(bytes: &[u8]) -> Result<set::State, Error> {
    let mut state = set::State::default();
    for item in items(bytes)? {
        let [id_, author_, stamp_, element, action, named] = fields(item)?;
        let named = ids(named)?;
        let action = match action {
            Value::Unsigned(0) => Action::Add { replaces: named },
            Value::Unsigned(1) => Action::Remove { observed: named },
            _ => return Err(CORRUPT),
        };
        state.insert(&Edit {
            id: id(id_)?,
            author: author(author_)?,
            stamp: stamp(stamp_)?,
            element,
            action,
        });
    }
    Ok(state)
}

/// The creates and deletes an object received, by operation id. The
/// object is a function of them, as a field's state is of its operations.
#[derive(Default)]
pub(super) struct ObjectState(BTreeMap<OperationId, ObjectOperation>);

impl ObjectState {
    /// Reads the bytes [`ObjectState::bytes`] wrote.
    pub(super) fn read(bytes: &[u8]) -> Result<ObjectState, Error> {
        let mut state = ObjectState::default();
        for item in items(bytes)? {
            let Value::Array(mut fields) = item else {
                return Err(CORRUPT);
            };
            let kind = (!fields.is_empty()).then(|| fields.remove(0));
            let operation = match (kind, fields.len()) {
                (Some(Value::Unsigned(0)), 4) => {
                    let [id_, author_, stamp_, collection] =
                        fields.try_into().map_err(|_| CORRUPT)?;
                    let Value::Text(collection) = collection else {
                        return Err(CORRUPT);
                    };
                    ObjectOperation::Create(Create {
                        id: id(id_)?,
                        author: author(author_)?,
                        stamp: stamp(stamp_)?,
                        collection,
                    })
                }
                (Some(Value::Unsigned(1)), 5) => {
                    ObjectOperation::Delete(write(fields.try_into().map_err(|_| CORRUPT)?)?)
                }
                _ => return Err(CORRUPT),
            };
            state.insert(operation);
        }
        Ok(state)
    }

    /// Keeps an operation, unless it holds one with its id.
    pub(super) fn insert(&mut self, operation: ObjectOperation) {
        let id = match &operation {
            ObjectOperation::Create(create) => create.id,
            ObjectOperation::Delete(write) => write.id,
        };
        self.0.entry(id).or_insert(operation);
    }

    /// The canonical CBOR of every operation, in operation id order: a
    /// create as `[0, id, author, stamp, collection]`, a delete as `[1, id,
    /// author, stamp, value, [replaced id…]]`. Not a wire format.
    pub(super) fn bytes(&self) -> Vec<u8> {
        let items = self
            .0
            .values()
            .map(|operation| match operation {
                ObjectOperation::Create(c) => Value::Array(vec![
                    Value::Unsigned(0),
                    id_value(&c.id),
                    Value::Bytes(c.author.0.to_vec()),
                    Value::Unsigned(c.stamp.0),
                    Value::Text(c.collection.clone()),
                ]),
                ObjectOperation::Delete(w) => Value::Array(vec![
                    Value::Unsigned(1),
                    id_value(&w.id),
                    Value::Bytes(w.author.0.to_vec()),
                    Value::Unsigned(w.stamp.0),
                    w.value.clone(),
                    Value::Array(w.replaces.iter().map(id_value).collect()),
                ]),
            })
            .collect();
        cbor::encode(&Value::Array(items))
    }

    /// The object these operations make.
    pub(super) fn object(&self) -> Object {
        let mut object = Object::default();
        for operation in self.0.values() {
            object.apply(operation);
        }
        object
    }
}
