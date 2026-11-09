//! Reading the CBOR maps of control bodies and certificates: unsigned keys
//! only, taken out one by one, with what is left over checked last.

use std::collections::{BTreeMap, BTreeSet};

use crate::cbor::{Key, Value};
use crate::change::ChangeError;
use crate::ids::ChangeId;

/// The entries of a map with unsigned keys.
pub(crate) struct Fields(pub(crate) BTreeMap<u64, Value>);

impl Fields {
    pub(crate) fn of(value: Value) -> Result<Fields, ChangeError> {
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

    pub(crate) fn optional(&mut self, key: u64) -> Option<(u64, Value)> {
        self.0.remove(&key).map(|value| (key, value))
    }

    pub(crate) fn required(&mut self, key: u64) -> Result<(u64, Value), ChangeError> {
        self.optional(key).ok_or(ChangeError::MissingKey(key))
    }

    /// Fails on a key that should not be there.
    pub(crate) fn absent(&self, key: u64) -> Result<(), ChangeError> {
        match self.0.contains_key(&key) {
            true => Err(ChangeError::InvalidValue(key)),
            false => Ok(()),
        }
    }
}

/// A map of unsigned keys, in key order.
pub(crate) fn map(entries: impl IntoIterator<Item = (u64, Value)>) -> Value {
    Value::Map(
        entries
            .into_iter()
            .map(|(key, value)| (Key::Unsigned(key), value))
            .collect(),
    )
}

pub(crate) fn unsigned((key, value): (u64, Value)) -> Result<u64, ChangeError> {
    match value {
        Value::Unsigned(n) => Ok(n),
        _ => Err(ChangeError::InvalidValue(key)),
    }
}

pub(crate) fn fixed<const N: usize>((key, value): (u64, Value)) -> Result<[u8; N], ChangeError> {
    match value {
        Value::Bytes(bytes) => bytes.try_into().map_err(|_| ChangeError::InvalidValue(key)),
        _ => Err(ChangeError::InvalidValue(key)),
    }
}

/// An array of change ids, sorted by bytes and without repeats.
pub(crate) fn change_ids((key, value): (u64, Value)) -> Result<BTreeSet<ChangeId>, ChangeError> {
    let Value::Array(items) = value else {
        return Err(ChangeError::InvalidValue(key));
    };
    let mut ids = BTreeSet::new();
    for item in items {
        let id = ChangeId(fixed((key, item))?);
        if ids.last().is_some_and(|last| *last >= id) {
            return Err(ChangeError::UnsortedDependencies);
        }
        ids.insert(id);
    }
    Ok(ids)
}

pub(crate) fn bytes(bytes: &[u8]) -> Value {
    Value::Bytes(bytes.to_vec())
}
