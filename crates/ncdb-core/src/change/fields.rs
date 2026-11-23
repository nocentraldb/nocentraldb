//! Reading the maps of a change: the entries of a map with unsigned keys,
//! taken out one by one, and the payload decoded as one tree, which says
//! why bytes are no payload.

use std::collections::{BTreeMap, BTreeSet};

use super::{ChangeError, Extensions, Operation, PAYLOAD_DEPTH, Payload};
use crate::cbor::{self, Key, Value};
use crate::clock::Hlc;
use crate::ids::ChangeId;

/// The entries of a map with unsigned keys, taken out one by one; what is
/// left are the unknown keys.
pub(super) struct Fields(pub(super) BTreeMap<u64, Value>);

impl Fields {
    pub(super) fn decode(bytes: &[u8], depth: usize) -> Result<Fields, ChangeError> {
        Fields::of(cbor::decode_at(bytes, depth)?)
    }

    pub(super) fn of(value: Value) -> Result<Fields, ChangeError> {
        let Value::Map(entries) = value else {
            return Err(ChangeError::NotAMap);
        };
        entries
            .into_iter()
            .map(|(key, value)| match key {
                Key::Unsigned(key) => Ok((key, value)),
                Key::Text(_) => Err(ChangeError::TextKey),
            })
            .collect::<Result<_, _>>()
            .map(Fields)
    }

    pub(super) fn optional(&mut self, key: u64) -> Option<(u64, Value)> {
        self.0.remove(&key).map(|value| (key, value))
    }

    pub(super) fn required(&mut self, key: u64) -> Result<(u64, Value), ChangeError> {
        self.optional(key).ok_or(ChangeError::MissingKey(key))
    }

    /// The unknown keys. A map whose entries were all taken out keeps
    /// its node; a new empty one allocates nothing, which counts for each
    /// operation of a large change (NOC-422).
    pub(super) fn rest(self) -> Extensions {
        if self.0.is_empty() {
            return Extensions::new();
        }
        Extensions(self.0)
    }
}

pub(super) fn unsigned((key, value): (u64, Value)) -> Result<u64, ChangeError> {
    match value {
        Value::Unsigned(n) => Ok(n),
        _ => Err(ChangeError::InvalidValue(key)),
    }
}

pub(super) fn bytes((key, value): (u64, Value)) -> Result<Vec<u8>, ChangeError> {
    match value {
        Value::Bytes(bytes) => Ok(bytes),
        _ => Err(ChangeError::InvalidValue(key)),
    }
}

pub(super) fn fixed<const N: usize>(entry: (u64, Value)) -> Result<[u8; N], ChangeError> {
    let key = entry.0;
    bytes(entry)?
        .try_into()
        .map_err(|_| ChangeError::InvalidValue(key))
}

pub(super) fn text((key, value): (u64, Value)) -> Result<String, ChangeError> {
    match value {
        Value::Text(text) => Ok(text),
        _ => Err(ChangeError::InvalidValue(key)),
    }
}

/// A map of known entries and unknown keys, in key order.
pub(super) fn map(known: Vec<(u64, Value)>, extensions: &Extensions) -> Value {
    let unknown = extensions.iter().map(|(key, value)| (key, value.clone()));
    Value::Map(
        known
            .into_iter()
            .chain(unknown)
            .map(|(key, value)| (Key::Unsigned(key), value))
            .collect(),
    )
}

/// The dependencies, which the encoding holds sorted by bytes and without
/// repeats.
pub(super) fn dependencies((key, value): (u64, Value)) -> Result<BTreeSet<ChangeId>, ChangeError> {
    let Value::Array(items) = value else {
        return Err(ChangeError::InvalidValue(key));
    };
    let mut dependencies = BTreeSet::new();
    for item in items {
        let id = ChangeId(fixed((key, item))?);
        if dependencies.last().is_some_and(|last| *last >= id) {
            return Err(ChangeError::UnsortedDependencies);
        }
        dependencies.insert(id);
    }
    Ok(dependencies)
}

impl Payload {
    /// [`Payload::decode`] through one tree of the whole payload: what
    /// says, for bytes the streaming decode does not take, why they are
    /// no payload, exactly as it always has.
    pub(super) fn decode_whole(bytes: &[u8]) -> Result<Payload, ChangeError> {
        let mut fields = Fields::decode(bytes, PAYLOAD_DEPTH)?;
        let format = unsigned(fields.required(1)?)?;
        let clock = Hlc(unsigned(fields.required(2)?)?);
        let dependencies = dependencies(fields.required(3)?)?;
        let Value::Array(operations) = fields.required(4)?.1 else {
            return Err(ChangeError::InvalidValue(4));
        };
        let operations = operations
            .into_iter()
            .map(Operation::from_value)
            .collect::<Result<_, _>>()?;
        let padding = fields.optional(5).map(self::bytes).transpose()?;
        Ok(Payload {
            format,
            clock,
            dependencies,
            operations,
            padding,
            extensions: fields.rest(),
        })
    }

    /// The payload of the map's entries `fields`, all but key 4, and of
    /// its `operations`, decoded from key 4's array.
    pub(super) fn of(
        fields: BTreeMap<u64, Value>,
        operations: Vec<Operation>,
    ) -> Result<Payload, ChangeError> {
        let mut fields = Fields(fields);
        let format = unsigned(fields.required(1)?)?;
        let clock = Hlc(unsigned(fields.required(2)?)?);
        let dependencies = dependencies(fields.required(3)?)?;
        let padding = fields.optional(5).map(self::bytes).transpose()?;
        Ok(Payload {
            format,
            clock,
            dependencies,
            operations,
            padding,
            extensions: fields.rest(),
        })
    }
}
