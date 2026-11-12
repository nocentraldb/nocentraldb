//! `Data`, what a plugin reads and writes and what crosses both SDKs (RFC
//! 0011, "The data a plugin reads and writes"): the subset of CBOR that
//! RFC 0002's profile allows, with text keys in maps. It encodes to, and
//! decodes from, RFC 0002's canonical bytes only, so a plugin can keep its
//! state as `Data` without writing CBOR by hand.

use std::collections::BTreeMap;
use std::fmt;

use crate::cbor::{self, Key, Value};
use crate::redact::{Hidden, Size};

/// A value of the CBOR subset RFC 0002 allows. Its `Debug` output names
/// only kinds and sizes, never content (RFC 0011, "What a plugin can
/// reach").
#[derive(Clone, PartialEq)]
pub enum Data {
    /// `null`.
    Null,
    /// `false` or `true`.
    Bool(bool),
    /// An integer from -2^64 to 2^64 - 1, CBOR's range. A value outside it
    /// does not encode.
    Integer(i128),
    /// A floating-point number. Every NaN encodes as the same value.
    Float(f64),
    /// A text, valid UTF-8.
    Text(String),
    /// A byte string.
    Bytes(Vec<u8>),
    /// An array.
    Array(Vec<Data>),
    /// A map with text keys, in the order of the keys' canonical bytes.
    Map(BTreeMap<String, Data>),
}

/// Why bytes or a value are not `Data`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DataError {
    /// An integer outside -2^64 to 2^64 - 1.
    OutOfRange,
    /// Bytes that are not RFC 0002's canonical CBOR, or are nested too
    /// deeply.
    NotCanonical,
    /// A map with a key that is no text.
    NonTextKey,
}

impl fmt::Display for DataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            DataError::OutOfRange => "an integer outside CBOR's range, -2^64 to 2^64 - 1",
            DataError::NotCanonical => "not RFC 0002's canonical CBOR",
            DataError::NonTextKey => "a map with a key that is no text",
        })
    }
}

impl std::error::Error for DataError {}

impl Data {
    /// The canonical bytes of the value (RFC 0002, "The canonical
    /// encoding"): equal values, equal bytes.
    pub fn encode(&self) -> Result<Vec<u8>, DataError> {
        Ok(cbor::encode(&self.to_value()?))
    }

    /// The value of canonical bytes; any other form is refused.
    pub fn decode(bytes: &[u8]) -> Result<Data, DataError> {
        let value = cbor::decode(bytes).map_err(|_| DataError::NotCanonical)?;
        Data::from_value(value)
    }

    /// The value as the core's CBOR value.
    #[doc(hidden)]
    pub fn to_value(&self) -> Result<Value, DataError> {
        Ok(match self {
            Data::Null => Value::Null,
            Data::Bool(b) => Value::Bool(*b),
            Data::Integer(n) if *n >= 0 => {
                Value::Unsigned(u64::try_from(*n).map_err(|_| DataError::OutOfRange)?)
            }
            Data::Integer(n) => {
                Value::Negative(u64::try_from(-1 - *n).map_err(|_| DataError::OutOfRange)?)
            }
            Data::Float(x) => Value::Float(*x),
            Data::Text(text) => Value::Text(text.clone()),
            Data::Bytes(bytes) => Value::Bytes(bytes.clone()),
            Data::Array(items) => {
                Value::Array(items.iter().map(Data::to_value).collect::<Result<_, _>>()?)
            }
            Data::Map(entries) => Value::Map(
                entries
                    .iter()
                    .map(|(key, value)| Ok((Key::Text(key.clone()), value.to_value()?)))
                    .collect::<Result<_, DataError>>()?,
            ),
        })
    }

    /// The core's CBOR value as data; a map with a key that is no text is
    /// refused.
    #[doc(hidden)]
    pub fn from_value(value: Value) -> Result<Data, DataError> {
        Ok(match value {
            Value::Null => Data::Null,
            Value::Bool(b) => Data::Bool(b),
            Value::Unsigned(n) => Data::Integer(i128::from(n)),
            Value::Negative(n) => Data::Integer(-1 - i128::from(n)),
            Value::Float(x) => Data::Float(x),
            Value::Text(text) => Data::Text(text),
            Value::Bytes(bytes) => Data::Bytes(bytes),
            Value::Array(items) => Data::Array(
                items
                    .into_iter()
                    .map(Data::from_value)
                    .collect::<Result<_, _>>()?,
            ),
            Value::Map(entries) => Data::Map(
                entries
                    .into_iter()
                    .map(|(key, value)| match key {
                        Key::Text(key) => Ok((key, Data::from_value(value)?)),
                        Key::Unsigned(_) => Err(DataError::NonTextKey),
                    })
                    .collect::<Result<_, _>>()?,
            ),
        })
    }
}

impl fmt::Debug for Data {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Data::Null => f.write_str("Null"),
            Data::Bool(_) => f.debug_tuple("Bool").field(&Hidden).finish(),
            Data::Integer(_) => f.debug_tuple("Integer").field(&Hidden).finish(),
            Data::Float(_) => f.debug_tuple("Float").field(&Hidden).finish(),
            Data::Text(text) => f
                .debug_tuple("Text")
                .field(&Size(text.len(), "bytes"))
                .finish(),
            Data::Bytes(bytes) => f
                .debug_tuple("Bytes")
                .field(&Size(bytes.len(), "bytes"))
                .finish(),
            Data::Array(items) => f
                .debug_tuple("Array")
                .field(&Size(items.len(), "items"))
                .finish(),
            Data::Map(entries) => f
                .debug_tuple("Map")
                .field(&Size(entries.len(), "entries"))
                .finish(),
        }
    }
}
