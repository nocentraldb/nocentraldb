//! A reference into another space, as RFC 0004 Amendment 4 defines it: the
//! field type `ncdb/ref`, rule version 1. It is a basic value whose value
//! is the target's space id and object id, written `[space id, object id]`,
//! and it merges by the code of basic values. It has no merge rule of its
//! own.

use crate::basic::{self, Values, Write};
use crate::cbor::Value;
use crate::field::{FieldType, Outcome};
use crate::ids::{ObjectId, OperationId, SpaceId};
pub use crate::link::Target;

use std::collections::BTreeSet;

/// The reference field type: one target, in any space, in one of
/// `collections`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reference {
    /// The collections the schema allows the target in (`targets`).
    pub collections: BTreeSet<String>,
}

/// `target` as the value a reference holds: a CBOR array of two 16-byte
/// byte strings, the space id and then the object id (decision 16).
pub fn value(target: Target) -> Value {
    Value::Array(vec![
        Value::Bytes(target.space.0.to_vec()),
        Value::Bytes(target.object.0.to_vec()),
    ])
}

/// The target `value` names, or `None` for a value that does not fit:
/// anything but an array of two 16-byte byte strings (decision 17).
pub fn target(value: &Value) -> Option<Target> {
    let Value::Array(items) = value else {
        return None;
    };
    let id = |item: &Value| match item {
        Value::Bytes(bytes) => <[u8; 16]>::try_from(bytes.as_slice()).ok(),
        _ => None,
    };
    match items.as_slice() {
        [space, object] => Some(Target {
            space: SpaceId(id(space)?),
            object: ObjectId(id(object)?),
        }),
        _ => None,
    }
}

impl FieldType for Reference {
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
        Values::References.read(state)
    }

    fn outcome(&self, state: &basic::State, id: OperationId) -> Option<Outcome> {
        state.outcome(id)
    }
}
