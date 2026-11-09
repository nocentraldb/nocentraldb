//! How the operations of basic values, sets and links travel in a change,
//! as RFC 0004 Amendment 1 defines it: the operation number in key 4 of
//! RFC 0002's operation map, the arguments as a map in key 5.

use std::collections::{BTreeMap, BTreeSet};

use crate::basic::{BasicValue, Write};
use crate::cbor::{self, Key, Value};
use crate::change::{ChangeError, Operation};
use crate::clock::Hlc;
use crate::field::FieldType;
use crate::ids::{ChangeId, MemberId, ObjectId, OperationId};
use crate::link::{Link, Links};
use crate::log::OperationParkReason;
use crate::set::{Action, Edit, Set};

/// Operation number 1: set, on a basic value or a link.
pub const SET: u64 = 1;
/// Operation number 2: add, on a set or links.
pub const ADD: u64 = 2;
/// Operation number 3: remove, on a set or links.
pub const REMOVE: u64 = 3;

/// Argument 1: the value written, or the element added or removed.
const VALUE: u64 = 1;
/// Argument 2: the operations a set or an add replaces.
const REPLACES: u64 = 2;
/// Argument 3: the additions a remove observed.
const OBSERVED: u64 = 3;

/// The last argument key a member must understand; keys above it may be
/// ignored.
const LAST_MUST_UNDERSTAND_ARGUMENT: u64 = 63;

/// How deeply an operation map sits in a change, as [`cbor::MAX_DEPTH`]
/// counts: frame 1, header 2, payload 3, operation array 4.
const OPERATION_DEPTH: usize = 5;

/// What a field operation takes from its change rather than from its own
/// bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Origin {
    /// The change id and the operation's position in the payload.
    pub id: OperationId,
    /// The member that signed the change.
    pub author: MemberId,
    /// The payload's clock.
    pub stamp: Hlc,
}

/// A field type whose operations travel in a change.
pub trait Encoding: FieldType {
    /// The operation number and the arguments of `op`.
    fn arguments(&self, op: &Self::Operation) -> (u64, Value);

    /// Reads `operation`, an operation on a field of this type, or says
    /// why it is parked.
    fn decode(
        &self,
        operation: &Operation,
        origin: Origin,
    ) -> Result<Self::Operation, OperationParkReason>;
}

/// The operation map that writes `op` to `field` of `object` in
/// `collection`.
pub fn encode<T: Encoding>(
    field_type: &T,
    op: &T::Operation,
    object: ObjectId,
    collection: &str,
    field: &str,
) -> Operation {
    let (number, arguments) = field_type.arguments(op);
    Operation::new(
        object,
        collection.to_owned(),
        Some(field.to_owned()),
        number,
        Some(arguments),
    )
}

impl Operation {
    /// The canonical bytes of this operation map.
    pub fn encode(&self) -> Vec<u8> {
        cbor::encode(&self.to_value())
    }

    /// Decodes canonical bytes of one operation map, as it sits in a
    /// payload's operation array.
    pub fn decode(bytes: &[u8]) -> Result<Operation, ChangeError> {
        Operation::from_value(cbor::decode_at(bytes, OPERATION_DEPTH)?)
    }
}

impl Encoding for BasicValue {
    fn arguments(&self, op: &Write) -> (u64, Value) {
        write_arguments(op)
    }

    fn decode(&self, operation: &Operation, origin: Origin) -> Result<Write, OperationParkReason> {
        decode_write(operation, origin)
    }
}

impl Encoding for Link {
    fn arguments(&self, op: &Write) -> (u64, Value) {
        write_arguments(op)
    }

    fn decode(&self, operation: &Operation, origin: Origin) -> Result<Write, OperationParkReason> {
        decode_write(operation, origin)
    }
}

impl Encoding for Set {
    fn arguments(&self, op: &Edit) -> (u64, Value) {
        edit_arguments(op)
    }

    fn decode(&self, operation: &Operation, origin: Origin) -> Result<Edit, OperationParkReason> {
        decode_edit(operation, origin)
    }
}

impl Encoding for Links {
    fn arguments(&self, op: &Edit) -> (u64, Value) {
        edit_arguments(op)
    }

    fn decode(&self, operation: &Operation, origin: Origin) -> Result<Edit, OperationParkReason> {
        decode_edit(operation, origin)
    }
}

/// set: `{1: value, 2: replaces}`.
fn write_arguments(op: &Write) -> (u64, Value) {
    (
        SET,
        map([(VALUE, op.value.clone()), (REPLACES, list(&op.replaces))]),
    )
}

/// add: `{1: element, 2: replaces}`; remove: `{1: element, 3: observed}`.
fn edit_arguments(op: &Edit) -> (u64, Value) {
    let element = (VALUE, op.element.clone());
    match &op.action {
        Action::Add { replaces } => (ADD, map([element, (REPLACES, list(replaces))])),
        Action::Remove { observed } => (REMOVE, map([element, (OBSERVED, list(observed))])),
    }
}

fn map<const N: usize>(entries: [(u64, Value); N]) -> Value {
    Value::Map(
        entries
            .into_iter()
            .map(|(key, value)| (Key::Unsigned(key), value))
            .collect(),
    )
}

/// A list of operation ids. A set's order is the order of the ids'
/// canonical bytes: the change id has a fixed length, and a minimal
/// integer sorts like the number.
fn list(ids: &BTreeSet<OperationId>) -> Value {
    Value::Array(
        ids.iter()
            .map(|id| {
                Value::Array(vec![
                    Value::Bytes(id.change.0.to_vec()),
                    Value::Unsigned(id.position),
                ])
            })
            .collect(),
    )
}

fn decode_write(operation: &Operation, origin: Origin) -> Result<Write, OperationParkReason> {
    if operation.op != SET {
        return Err(OperationParkReason::UnknownOperation);
    }
    let mut arguments = arguments(operation, &[VALUE, REPLACES])?;
    Ok(Write {
        id: origin.id,
        author: origin.author,
        stamp: origin.stamp,
        value: arguments.take(VALUE),
        replaces: ids(arguments.take(REPLACES))?,
    })
}

fn decode_edit(operation: &Operation, origin: Origin) -> Result<Edit, OperationParkReason> {
    let (names, action): (u64, fn(BTreeSet<OperationId>) -> Action) = match operation.op {
        ADD => (REPLACES, |replaces| Action::Add { replaces }),
        REMOVE => (OBSERVED, |observed| Action::Remove { observed }),
        _ => return Err(OperationParkReason::UnknownOperation),
    };
    let mut arguments = arguments(operation, &[VALUE, names])?;
    Ok(Edit {
        id: origin.id,
        author: origin.author,
        stamp: origin.stamp,
        element: arguments.take(VALUE),
        action: action(ids(arguments.take(names))?),
    })
}

/// The arguments of an operation: exactly the keys `defined`, once keys
/// from 64 up are dropped.
struct Arguments(BTreeMap<u64, Value>);

impl Arguments {
    /// A key [`arguments`] checked is present.
    fn take(&mut self, key: u64) -> Value {
        self.0.remove(&key).expect("a defined argument is present")
    }
}

/// Reads the arguments of `operation`, whose keys `defined` are required.
///
/// An unknown "must understand" key parks the operation as an unknown
/// argument before its shape is checked: a later rule version may define
/// the key and the shape that goes with it, while a malformed operation
/// stays parked on every version.
fn arguments(operation: &Operation, defined: &[u64]) -> Result<Arguments, OperationParkReason> {
    let malformed = OperationParkReason::MalformedArguments;
    let Some(Value::Map(entries)) = &operation.value else {
        return Err(malformed);
    };
    // Keys 1 to 3 are this RFC's: one an operation does not define is
    // malformed, not unknown.
    let must_understand = OBSERVED + 1..=LAST_MUST_UNDERSTAND_ARGUMENT;
    let unknown = |key: &Key| matches!(key, Key::Unsigned(key) if must_understand.contains(key));
    if entries.keys().any(unknown) {
        return Err(OperationParkReason::UnknownArgument);
    }
    let mut arguments = BTreeMap::new();
    for (key, value) in entries {
        match key {
            Key::Unsigned(key) if defined.contains(key) => {
                arguments.insert(*key, value.clone());
            }
            Key::Unsigned(key) if *key > LAST_MUST_UNDERSTAND_ARGUMENT => {}
            _ => return Err(malformed),
        }
    }
    if arguments.len() != defined.len() {
        return Err(malformed);
    }
    Ok(Arguments(arguments))
}

/// A list of operation ids, read as a set: in any order, with repeats.
fn ids(list: Value) -> Result<BTreeSet<OperationId>, OperationParkReason> {
    let malformed = OperationParkReason::MalformedArguments;
    let Value::Array(items) = list else {
        return Err(malformed);
    };
    items
        .into_iter()
        .map(|item| match item {
            Value::Array(pair) => match <[Value; 2]>::try_from(pair) {
                Ok([Value::Bytes(change), Value::Unsigned(position)]) => {
                    let change = ChangeId(change.try_into().map_err(|_| malformed)?);
                    Ok(OperationId { change, position })
                }
                _ => Err(malformed),
            },
            _ => Err(malformed),
        })
        .collect()
}
