//! A field value, read and written by the field's name (RFC 0009, decision
//! S18): a text, an integer, a float, a boolean or an object id, as the
//! field's type in the lock asks. Its `Debug` output prints `[redacted]`:
//! a value is content.

use std::fmt;

use ncdb_core::cbor;

use crate::error::{Error, ErrorCode};
use crate::id::ObjectId;

/// A value to write into a field, or one read from it.
#[derive(Clone, PartialEq)]
pub struct Value(pub(crate) Repr);

#[derive(Clone, PartialEq)]
pub(crate) enum Repr {
    Text(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Id(ObjectId),
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[redacted]")
    }
}

impl From<&str> for Value {
    fn from(text: &str) -> Value {
        Value(Repr::Text(text.to_owned()))
    }
}

impl From<String> for Value {
    fn from(text: String) -> Value {
        Value(Repr::Text(text))
    }
}

impl From<i64> for Value {
    fn from(n: i64) -> Value {
        Value(Repr::Integer(n))
    }
}

impl From<i32> for Value {
    fn from(n: i32) -> Value {
        Value(Repr::Integer(n.into()))
    }
}

impl From<f64> for Value {
    fn from(x: f64) -> Value {
        Value(Repr::Float(x))
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Value {
        Value(Repr::Boolean(b))
    }
}

impl From<ObjectId> for Value {
    fn from(id: ObjectId) -> Value {
        Value(Repr::Id(id))
    }
}

/// The error of a read that asks for another kind than the field holds.
fn other_kind(asked: &str) -> Error {
    Error::new(
        ErrorCode::InvalidValue,
        format!("the field does not hold {asked}"),
    )
}

macro_rules! read_as {
    ($t:ty, $asked:literal, $pattern:pat => $out:expr) => {
        impl<'a> TryFrom<&'a Value> for $t {
            type Error = Error;

            fn try_from(value: &'a Value) -> Result<$t, Error> {
                match &value.0 {
                    $pattern => Ok($out),
                    _ => Err(other_kind($asked)),
                }
            }
        }
    };
}

read_as!(&'a str, "a text", Repr::Text(text) => text.as_str());
read_as!(i64, "an integer", Repr::Integer(n) => *n);
read_as!(f64, "a float", Repr::Float(x) => *x);
read_as!(bool, "a boolean", Repr::Boolean(b) => *b);
read_as!(ObjectId, "an object id", Repr::Id(id) => *id);

impl Value {
    /// The value as a change carries it (RFC 0004, "Value rules per
    /// kind"): an object id as its 16 bytes.
    pub(crate) fn to_cbor(&self) -> cbor::Value {
        match &self.0 {
            Repr::Text(text) => cbor::Value::Text(text.clone()),
            Repr::Integer(n) if *n >= 0 => cbor::Value::Unsigned(n.unsigned_abs()),
            Repr::Integer(n) => cbor::Value::Negative((-1 - *n).unsigned_abs()),
            Repr::Float(x) => cbor::Value::Float(*x),
            Repr::Boolean(b) => cbor::Value::Bool(*b),
            Repr::Id(id) => ncdb_core::link::value(id.0),
        }
    }

    /// The value a field of this kind holds, or `None` for a value the
    /// SDK cannot read as one: the field reads unset.
    pub(crate) fn from_cbor(value: &cbor::Value, id: bool) -> Option<Value> {
        let repr = match value {
            cbor::Value::Bytes(bytes) if id => Repr::Id(ObjectId(ncdb_core::ids::ObjectId(
                bytes.as_slice().try_into().ok()?,
            ))),
            cbor::Value::Text(text) => Repr::Text(text.clone()),
            cbor::Value::Unsigned(n) => Repr::Integer(i64::try_from(*n).ok()?),
            cbor::Value::Negative(n) => Repr::Integer(-1 - i64::try_from(*n).ok()?),
            cbor::Value::Float(x) => Repr::Float(*x),
            cbor::Value::Bool(b) => Repr::Boolean(*b),
            _ => return None,
        };
        Some(Value(repr))
    }
}
