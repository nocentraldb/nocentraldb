//! The writer's model, payload key 64 (RFC 0002, "The writer's model"):
//! the `model.id` and `model.version` of the lock that wrote a sealed
//! change. The core writes it into every sealed payload, and a member reads
//! it at step 6, "Understand the payload", to park a change of another
//! model whole (RFC 0005, "Data from a newer model").
//!
//! Key 64 is the first key of the "may ignore" range. The payload keeps it
//! among the keys it encodes again unchanged, so a payload decodes and
//! encodes to the same bytes whatever key 64 holds.

use std::collections::BTreeMap;

use crate::cbor::{self, Key, Value};
use crate::change::Payload;

/// Payload key 64.
pub const KEY: u64 = 64;

/// The model that wrote a sealed change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WriterModel {
    /// Key 1: the writer's `model.id`, its 32 hex digits as raw bytes.
    pub id: [u8; 16],
    /// Key 2: the writer's `model.version`.
    pub version: u64,
}

/// Key 64 holds a value that is not a map of a 16-byte model id and an
/// unsigned model version: it does not show that the member's model wrote
/// the change, which is parked as "another model".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WrongShape;

impl WriterModel {
    /// The value of key 64.
    pub fn to_value(&self) -> Value {
        Value::Map(BTreeMap::from([
            (Key::Unsigned(1), Value::Bytes(self.id.to_vec())),
            (Key::Unsigned(2), Value::Unsigned(self.version)),
        ]))
    }

    /// The canonical bytes of the value of key 64.
    pub fn encode(&self) -> Vec<u8> {
        cbor::encode(&self.to_value())
    }

    /// Reads the value of key 64. Keys of the map other than 1 and 2 are
    /// ignored, so a later field of the model can be a new key of it.
    pub fn from_value(value: &Value) -> Result<WriterModel, WrongShape> {
        let Value::Map(entries) = value else {
            return Err(WrongShape);
        };
        let Some(Value::Bytes(id)) = entries.get(&Key::Unsigned(1)) else {
            return Err(WrongShape);
        };
        let Some(Value::Unsigned(version)) = entries.get(&Key::Unsigned(2)) else {
            return Err(WrongShape);
        };
        Ok(WriterModel {
            id: id.as_slice().try_into().map_err(|_| WrongShape)?,
            version: *version,
        })
    }
}

impl Payload {
    /// The writer's model the payload names in key 64: `None` without key
    /// 64, an error for a value of the wrong shape.
    pub fn writer_model(&self) -> Option<Result<WriterModel, WrongShape>> {
        self.extensions()
            .iter()
            .find(|(key, _)| *key == KEY)
            .map(|(_, value)| WriterModel::from_value(value))
    }

    /// Names the writer's model in key 64.
    pub fn set_writer_model(&mut self, model: WriterModel) {
        self.insert_extension(KEY, model.to_value());
    }
}
