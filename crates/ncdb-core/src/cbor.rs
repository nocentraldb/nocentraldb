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

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt;

use minicbor::data::{Int, Type};
use minicbor::{Decoder, Encoder};

use crate::redact::{Hidden, Size};

/// A CBOR value of the profile.
///
/// Its `Debug` output shows the kind and size of a value, never the value:
/// a value can be content.
#[derive(Clone)]
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
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Unsigned(a), Value::Unsigned(b)) | (Value::Negative(a), Value::Negative(b)) => {
                a == b
            }
            (Value::Bytes(a), Value::Bytes(b)) => a == b,
            (Value::Text(a), Value::Text(b)) => a == b,
            (Value::Array(a), Value::Array(b)) => a == b,
            (Value::Map(a), Value::Map(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Null, Value::Null) => true,
            // Equal when their canonical encodings are: every NaN is one
            // value, and negative zero is not zero.
            (Value::Float(a), Value::Float(b)) => {
                (a.is_nan() && b.is_nan()) || a.to_bits() == b.to_bits()
            }
            _ => false,
        }
    }
}

impl Eq for Value {}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Unsigned(_) => f.debug_tuple("Unsigned").field(&Hidden).finish(),
            Value::Negative(_) => f.debug_tuple("Negative").field(&Hidden).finish(),
            Value::Bytes(bytes) => f
                .debug_tuple("Bytes")
                .field(&Size(bytes.len(), "bytes"))
                .finish(),
            Value::Text(text) => f
                .debug_tuple("Text")
                .field(&Size(text.len(), "bytes"))
                .finish(),
            Value::Array(items) => f
                .debug_tuple("Array")
                .field(&Size(items.len(), "items"))
                .finish(),
            Value::Map(entries) => f
                .debug_tuple("Map")
                .field(&Size(entries.len(), "entries"))
                .finish(),
            Value::Bool(_) => f.debug_tuple("Bool").field(&Hidden).finish(),
            Value::Null => f.write_str("Null"),
            Value::Float(_) => f.debug_tuple("Float").field(&Hidden).finish(),
        }
    }
}

/// A map key: an unsigned integer or a text string. Keys order by the bytes
/// of their encoding. Its `Debug` output hides the key.
#[derive(Clone, PartialEq, Eq)]
pub enum Key {
    /// An unsigned integer key.
    Unsigned(u64),
    /// A text key, allowed in the values of field types.
    Text(String),
}

impl fmt::Debug for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Key::Unsigned(_) => f.debug_tuple("Unsigned").field(&Hidden).finish(),
            Key::Text(text) => f
                .debug_tuple("Text")
                .field(&Size(text.len(), "bytes"))
                .finish(),
        }
    }
}

impl PartialOrd for Key {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Key {
    fn cmp(&self, other: &Self) -> Ordering {
        // The minimal encoding of an unsigned integer orders like the
        // number, every unsigned key's major type sorts before text's, and
        // text's length comes first in its encoding.
        match (self, other) {
            (Key::Unsigned(a), Key::Unsigned(b)) => a.cmp(b),
            (Key::Unsigned(_), Key::Text(_)) => Ordering::Less,
            (Key::Text(_), Key::Unsigned(_)) => Ordering::Greater,
            (Key::Text(a), Key::Text(b)) => (a.len(), a.as_bytes()).cmp(&(b.len(), b.as_bytes())),
        }
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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let reason = match self.reason {
            Reason::Truncated => "the bytes end inside a value",
            Reason::TrailingBytes => "bytes follow the value",
            Reason::IndefiniteLength => "an indefinite length",
            Reason::Tag => "a tag",
            Reason::DisallowedSimple => "a simple value or initial byte the profile does not allow",
            Reason::KeyType => "a map key that is neither an unsigned integer nor text",
            Reason::UnsortedKeys => "a map key out of order",
            Reason::DuplicateKey => "a map key twice",
            Reason::InvalidUtf8 => "text that is not valid UTF-8",
            Reason::NotCanonical => "a shorter or other canonical form exists",
        };
        write!(f, "not canonical CBOR at byte {}: {reason}", self.offset)
    }
}

impl std::error::Error for DecodeError {}

/// Encodes a value in its one canonical form.
pub fn encode(value: &Value) -> Vec<u8> {
    let mut encoder = Encoder::new(Vec::new());
    write(&mut encoder, value);
    encoder.into_writer()
}

/// Decodes canonical bytes into a value, and rejects every other form.
pub fn decode(bytes: &[u8]) -> Result<Value, DecodeError> {
    let mut decoder = Decoder::new(bytes);
    let value = read(&mut decoder)?;
    if decoder.position() != bytes.len() {
        return Err(error(decoder.position(), Reason::TrailingBytes));
    }
    // Every rule the reader does not check itself (shortest integers,
    // lengths and floats, the one NaN) shows as a difference here.
    let canonical = encode(&value);
    if let Some(offset) = canonical.iter().zip(bytes).position(|(a, b)| a != b) {
        return Err(error(offset, Reason::NotCanonical));
    }
    if canonical.len() != bytes.len() {
        return Err(error(
            canonical.len().min(bytes.len()),
            Reason::NotCanonical,
        ));
    }
    Ok(value)
}

fn error(offset: usize, reason: Reason) -> DecodeError {
    DecodeError { offset, reason }
}

type Writer = Encoder<Vec<u8>>;

const INFALLIBLE: &str = "writing to a Vec cannot fail";

fn write(encoder: &mut Writer, value: &Value) {
    match value {
        Value::Unsigned(n) => {
            encoder.u64(*n).expect(INFALLIBLE);
        }
        Value::Negative(n) => {
            let int = Int::try_from(-1 - i128::from(*n)).expect("-1 - u64 fits a CBOR integer");
            encoder.int(int).expect(INFALLIBLE);
        }
        Value::Bytes(bytes) => {
            encoder.bytes(bytes).expect(INFALLIBLE);
        }
        Value::Text(text) => {
            encoder.str(text).expect(INFALLIBLE);
        }
        Value::Array(items) => {
            encoder.array(len(items.len())).expect(INFALLIBLE);
            for item in items {
                write(encoder, item);
            }
        }
        Value::Map(entries) => {
            encoder.map(len(entries.len())).expect(INFALLIBLE);
            for (key, item) in entries {
                match key {
                    Key::Unsigned(n) => encoder.u64(*n).expect(INFALLIBLE),
                    Key::Text(text) => encoder.str(text).expect(INFALLIBLE),
                };
                write(encoder, item);
            }
        }
        Value::Bool(b) => {
            encoder.bool(*b).expect(INFALLIBLE);
        }
        Value::Null => {
            encoder.null().expect(INFALLIBLE);
        }
        Value::Float(x) => encoder.writer_mut().extend_from_slice(&float_bytes(*x)),
    }
}

fn len(n: usize) -> u64 {
    u64::try_from(n).expect("a length fits 64 bits")
}

/// The shortest of the 16-, 32- and 64-bit forms that keeps the value
/// exactly; every NaN is `0xf97e00`.
fn float_bytes(x: f64) -> Vec<u8> {
    if x.is_nan() {
        return vec![0xf9, 0x7e, 0x00];
    }
    if let Some(half) = to_half(x) {
        let mut bytes = vec![0xf9];
        bytes.extend_from_slice(&half.to_be_bytes());
        return bytes;
    }
    // Casting is exact when the value survives the round trip.
    #[allow(clippy::cast_possible_truncation)]
    let single = x as f32;
    if f64::from(single) == x {
        let mut bytes = vec![0xfa];
        bytes.extend_from_slice(&single.to_be_bytes());
        return bytes;
    }
    let mut bytes = vec![0xfb];
    bytes.extend_from_slice(&x.to_be_bytes());
    bytes
}

/// The half-precision bits of `x`, when it has an exact half-precision
/// form. `x` is not NaN.
fn to_half(x: f64) -> Option<u16> {
    let sign: u16 = if x.is_sign_negative() { 0x8000 } else { 0 };
    let magnitude = x.abs();
    if magnitude == 0.0 {
        return Some(sign);
    }
    if magnitude.is_infinite() {
        return Some(sign | 0x7c00);
    }
    if magnitude >= 2f64.powi(-14) {
        // A normal half: a biased exponent of 1 to 30 and ten bits of
        // mantissa; the 42 lower bits of the double's mantissa are zero.
        let bits = magnitude.to_bits();
        let exponent = i64::try_from(bits >> 52).expect("11 bits") - 1023;
        if exponent > 15 || bits & ((1 << 42) - 1) != 0 {
            return None;
        }
        let biased = u16::try_from(exponent + 15).expect("1 to 30");
        let mantissa = u16::try_from((bits >> 42) & 0x3ff).expect("10 bits");
        return Some(sign | biased << 10 | mantissa);
    }
    // A subnormal half: a multiple of 2^-24 below 2^-14.
    let steps = magnitude * 2f64.powi(24);
    if steps.fract() != 0.0 {
        return None;
    }
    // A whole number from 1 to 1023, so the cast is exact.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let steps = steps as u16;
    Some(sign | steps)
}

fn from_half(bits: u16) -> f64 {
    let magnitude = match (bits >> 10) & 0x1f {
        0 => f64::from(bits & 0x3ff) * 2f64.powi(-24),
        0x1f if bits & 0x3ff == 0 => f64::INFINITY,
        0x1f => f64::NAN,
        exponent => f64::from(0x400 | (bits & 0x3ff)) * 2f64.powi(i32::from(exponent) - 25),
    };
    if bits & 0x8000 == 0 {
        magnitude
    } else {
        -magnitude
    }
}

fn read(decoder: &mut Decoder<'_>) -> Result<Value, DecodeError> {
    let at = decoder.position();
    let truncated = |_| error(at, Reason::Truncated);
    let datatype = decoder.datatype().map_err(truncated)?;
    let value = match datatype {
        Type::U8 | Type::U16 | Type::U32 | Type::U64 => {
            Value::Unsigned(decoder.u64().map_err(truncated)?)
        }
        Type::I8 | Type::I16 | Type::I32 | Type::I64 | Type::Int => {
            let n = i128::from(decoder.int().map_err(truncated)?);
            Value::Negative(u64::try_from(-1 - n).expect("a negative CBOR integer"))
        }
        Type::Bytes => Value::Bytes(decoder.bytes().map_err(truncated)?.to_vec()),
        Type::String => Value::Text(read_text(decoder)?),
        Type::Array => {
            let n = decoder.array().map_err(truncated)?.expect("definite");
            let mut items = Vec::new();
            for _ in 0..n {
                items.push(read(decoder)?);
            }
            Value::Array(items)
        }
        Type::Map => {
            let n = decoder.map().map_err(truncated)?.expect("definite");
            let mut entries = BTreeMap::new();
            let mut previous: Option<Key> = None;
            for _ in 0..n {
                let key_at = decoder.position();
                let key = match read(decoder)? {
                    Value::Unsigned(n) => Key::Unsigned(n),
                    Value::Text(text) => Key::Text(text),
                    _ => return Err(error(key_at, Reason::KeyType)),
                };
                if let Some(previous) = &previous {
                    match key.cmp(previous) {
                        Ordering::Less => return Err(error(key_at, Reason::UnsortedKeys)),
                        Ordering::Equal => return Err(error(key_at, Reason::DuplicateKey)),
                        Ordering::Greater => {}
                    }
                }
                let item = read(decoder)?;
                previous = Some(key.clone());
                entries.insert(key, item);
            }
            Value::Map(entries)
        }
        Type::Bool => Value::Bool(decoder.bool().map_err(truncated)?),
        Type::Null => {
            decoder.null().map_err(truncated)?;
            Value::Null
        }
        Type::F16 => {
            let bytes = decoder
                .input()
                .get(at + 1..at + 3)
                .ok_or(error(at, Reason::Truncated))?;
            decoder.set_position(at + 3);
            Value::Float(from_half(u16::from_be_bytes([bytes[0], bytes[1]])))
        }
        Type::F32 => Value::Float(f64::from(decoder.f32().map_err(truncated)?)),
        Type::F64 => Value::Float(decoder.f64().map_err(truncated)?),
        Type::BytesIndef | Type::StringIndef | Type::ArrayIndef | Type::MapIndef => {
            return Err(error(at, Reason::IndefiniteLength));
        }
        Type::Tag => return Err(error(at, Reason::Tag)),
        Type::Undefined | Type::Simple | Type::Break | Type::Unknown(_) => {
            return Err(error(at, Reason::DisallowedSimple));
        }
    };
    Ok(value)
}

fn read_text(decoder: &mut Decoder<'_>) -> Result<String, DecodeError> {
    let at = decoder.position();
    match decoder.str() {
        Ok(text) => Ok(text.to_owned()),
        Err(e) if e.is_end_of_input() => Err(error(at, Reason::Truncated)),
        Err(_) => Err(error(at, Reason::InvalidUtf8)),
    }
}
