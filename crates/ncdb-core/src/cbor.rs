//! The canonical CBOR encoding of RFC 0002 ("The canonical encoding").
//!
//! CBOR (RFC 8949) with the core deterministic encoding of §4.2.1,
//! restricted further: definite and minimal lengths and integers, map keys
//! that are unsigned integers or text, sorted by the bytes of their encoding
//! and each at most once, no tags, no simple values other than `false`,
//! `true` and `null`, floats in their shortest exact form with every NaN as
//! `0xf97e00`, and integers within 64 bits.
//!
//! [`decode`] accepts only canonical bytes: it decodes, encodes the value
//! again and compares, so a value has exactly one byte representation.

use std::collections::BTreeMap;
use std::fmt;

/// A CBOR value of the profile.
#[derive(Clone, Debug)]
pub enum Value {
    /// An unsigned integer, major type 0.
    Unsigned(u64),
    /// A negative integer, major type 1: the value is `-1 - n`.
    Negative(u64),
    /// A byte string.
    Bytes(Vec<u8>),
    /// A text string, valid UTF-8, stored as given.
    Text(String),
    /// An array.
    Array(Vec<Value>),
    /// A map, sorted by the bytes of each key's encoding.
    Map(BTreeMap<Key, Value>),
    /// `false` or `true`.
    Bool(bool),
    /// `null`.
    Null,
    /// A floating-point number. Every NaN is the same value, and negative
    /// zero differs from positive zero.
    Float(f64),
}

impl PartialEq for Value {
    fn eq(&self, _other: &Self) -> bool {
        todo!()
    }
}

impl Eq for Value {}

/// A map key: an unsigned integer or a text string. Keys order by the bytes
/// of their encoding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Key {
    /// An unsigned integer key.
    Unsigned(u64),
    /// A text key, allowed in the values of field types.
    Text(String),
}

impl PartialOrd for Key {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Key {
    fn cmp(&self, _other: &Self) -> std::cmp::Ordering {
        todo!()
    }
}

/// Why bytes are not a canonical CBOR value of the profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    /// The bytes end inside a value.
    Truncated,
    /// Bytes follow the value.
    TrailingBytes,
    /// An indefinite-length string, array or map.
    IndefiniteLength,
    /// A tag.
    Tag,
    /// A simple value other than `false`, `true` and `null`, or a reserved
    /// initial byte.
    DisallowedSimple,
    /// A map key that is neither an unsigned integer nor a text string.
    KeyType,
    /// A map key that is not greater than the key before it.
    UnsortedKeys,
    /// A map key that appears twice.
    DuplicateKey,
    /// A text string that is not valid UTF-8.
    InvalidUtf8,
    /// A negative integer below -2^64.
    IntegerOutOfRange,
    /// A value that has a shorter or other canonical form: an integer, a
    /// length or a float that is not in its shortest form, or a NaN other
    /// than `0xf97e00`.
    NotCanonical,
}

/// Bytes that are not a canonical CBOR value of the profile. Says where and
/// why, never what the bytes held.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DecodeError {
    /// The offset of the first byte that breaks the profile.
    pub offset: usize,
    /// Which rule it breaks.
    pub reason: Reason,
}

impl fmt::Display for DecodeError {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!()
    }
}

impl std::error::Error for DecodeError {}

/// Encodes a value in its one canonical form.
pub fn encode(_value: &Value) -> Vec<u8> {
    todo!()
}

/// Decodes canonical bytes into a value, and rejects every other form.
pub fn decode(_bytes: &[u8]) -> Result<Value, DecodeError> {
    todo!()
}
