//! A field value, read and written by the field's name (RFC 0009, "The Rust
//! SDK"): a text, an integer, a float, a boolean, a [`Date`], a
//! [`Timestamp`], an object id or a [`Reference`], as the field's type in
//! the lock asks; a set or many links
//! reads as the list of its elements and is written with [`Value::add`] and
//! [`Value::remove`], never whole (RFC 0009, "Field operations per field
//! type"). [`Value::clear`] empties a basic field, a link or a reference.
//! Its `Debug` output prints `[redacted]`: a value is content.

use std::fmt;

use ncdb_core::basic::Kind;
use ncdb_core::cbor;
use ncdb_core::reference::Target;
use ncdb_plugin::Data;

use crate::date::{Date, Timestamp};
use crate::error::{Error, ErrorCode};
use crate::id::{ObjectId, SpaceId};

/// A value to write into a field, or one read from it.
#[derive(Clone, PartialEq)]
pub struct Value(pub(crate) Repr);

#[derive(Clone, PartialEq)]
pub(crate) enum Repr {
    Text(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    /// A calendar day (RFC 0005, "Days and points in time").
    Date(Date),
    /// A point in time to the millisecond, in UTC.
    Timestamp(Timestamp),
    Id(ObjectId),
    /// A reference: the target's space id and object id (RFC 0004,
    /// "References").
    Ref(Reference),
    /// The elements of a set or of many links, as a read gives them.
    List(Vec<Value>),
    /// Elements to add to a set or to many links.
    Add(Vec<Value>),
    /// Elements to remove from a set or from many links.
    Remove(Vec<Value>),
    /// A plugin field's read value, or a write intent for it (RFC 0011,
    /// "The data a plugin reads and writes").
    Data(Data),
    /// The clear of a basic field, a link or a reference: a set (1) with
    /// no value.
    Clear,
    /// A rich-text update under a leased client id, which only the bridge's
    /// editor binding writes (RFC 0013, "An editor: the binding").
    #[cfg(ncdb_rich_text)]
    RichText(crate::rich_text::RichTextUpdate),
    /// A rich-text field's document, as a read gives it (RFC 0013, "The
    /// Rust SDK").
    #[cfg(ncdb_rich_text)]
    RichTextRead(Box<crate::rich_text_field::RichText>),
    /// A first document from Markdown, for a rich-text field that holds no
    /// content (RFC 0013, "The first content").
    #[cfg(ncdb_rich_text)]
    Markdown(String),
}

impl Value {
    /// Adds `elements` to a set or to many links.
    pub fn add<V: Into<Value>>(elements: impl IntoIterator<Item = V>) -> Value {
        Value(Repr::Add(elements.into_iter().map(Into::into).collect()))
    }

    /// Removes `elements` from a set or from many links: the additions of
    /// each that the copy holds.
    pub fn remove<V: Into<Value>>(elements: impl IntoIterator<Item = V>) -> Value {
        Value(Repr::Remove(elements.into_iter().map(Into::into).collect()))
    }

    /// Clears a basic field, a link or a reference, as `null` does in
    /// TypeScript: a set (1) with no value (RFC 0009, "The Rust SDK"). A clear of a set or of
    /// many links is `InvalidValue`; a create with a clear leaves the field
    /// unset.
    pub fn clear() -> Value {
        Value(Repr::Clear)
    }
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

/// A reference into a space, the value of an `nc.ref` field: the target's
/// space id and object id (RFC 0004, "References"; Carsten, 2026-11-16,
/// NOC-354). Written with `Value::from`, read with `get::<Reference>`. It
/// names the target and holds nothing of it: an app that wants the target
/// reads it in its space. A reference field also takes an [`ObjectId`],
/// which names the written object's own space, and a link field takes a
/// `Reference` into the object's own space and refuses one into another
/// with `InvalidValue` (RFC 0018, "One function, two cases").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct Reference {
    /// The target's space id.
    pub space: SpaceId,
    /// The target's object id.
    pub id: ObjectId,
}

impl Reference {
    /// The reference to the object `id` in `space`.
    pub fn new(space: SpaceId, id: ObjectId) -> Reference {
        Reference { space, id }
    }
}

impl From<Date> for Value {
    fn from(day: Date) -> Value {
        Value(Repr::Date(day))
    }
}

impl From<Timestamp> for Value {
    fn from(point: Timestamp) -> Value {
        Value(Repr::Timestamp(point))
    }
}

impl From<Reference> for Value {
    fn from(reference: Reference) -> Value {
        Value(Repr::Ref(reference))
    }
}

impl From<ObjectId> for Value {
    fn from(id: ObjectId) -> Value {
        Value(Repr::Id(id))
    }
}

/// The error of a read that asks for another kind than the field holds.
pub(crate) fn other_kind(asked: &str) -> Error {
    Error::new(
        ErrorCode::InvalidValue,
        crate::texts::error::field_does_not_hold(asked),
    )
}

macro_rules! read_as {
    ($t:ty, $asked:expr, $pattern:pat => $out:expr) => {
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

read_as!(&'a str, crate::texts::part::KIND_TEXT, Repr::Text(text) => text.as_str());
read_as!(i64, crate::texts::part::KIND_INTEGER, Repr::Integer(n) => *n);
read_as!(f64, crate::texts::part::KIND_FLOAT, Repr::Float(x) => *x);
read_as!(bool, crate::texts::part::KIND_BOOLEAN, Repr::Boolean(b) => *b);
read_as!(Date, crate::texts::part::KIND_DATE, Repr::Date(day) => *day);
read_as!(Timestamp, crate::texts::part::KIND_TIMESTAMP, Repr::Timestamp(point) => *point);
read_as!(ObjectId, crate::texts::part::KIND_OBJECT_ID, Repr::Id(id) => *id);
read_as!(Reference, crate::texts::part::KIND_REFERENCE, Repr::Ref(reference) => *reference);
read_as!(&'a [Value], crate::texts::part::KIND_LIST, Repr::List(elements) => elements.as_slice());
read_as!(Data, crate::texts::part::KIND_DATA, Repr::Data(data) => data.clone());

impl From<Data> for Value {
    /// A write intent for a plugin field, which the plugin's `write` turns
    /// into operations (RFC 0011).
    fn from(data: Data) -> Value {
        Value(Repr::Data(data))
    }
}

impl Value {
    /// The value as a change carries it (RFC 0004, "Value rules per
    /// kind"): an object id as its 16 bytes. A list, an addition or a
    /// removal is no single value, nor is a clear.
    /// Whether this is [`Value::clear`]: in a query, a field fixed to unset.
    pub(crate) fn is_clear(&self) -> bool {
        matches!(self.0, Repr::Clear)
    }

    pub(crate) fn to_cbor(&self) -> Option<cbor::Value> {
        Some(match &self.0 {
            Repr::Text(text) => cbor::Value::Text(text.clone()),
            Repr::Integer(n) if *n >= 0 => cbor::Value::Unsigned(n.unsigned_abs()),
            Repr::Integer(n) => cbor::Value::Negative((-1 - *n).unsigned_abs()),
            Repr::Float(x) => cbor::Value::Float(*x),
            Repr::Boolean(b) => cbor::Value::Bool(*b),
            Repr::Date(day) => integer(day.days()),
            Repr::Timestamp(point) => integer(point.as_millis()),
            Repr::Id(id) => ncdb_core::link::value(id.0),
            Repr::Ref(reference) => ncdb_core::reference::value(Target {
                space: reference.space.0,
                object: reference.id.0,
            }),
            Repr::List(_) | Repr::Add(_) | Repr::Remove(_) | Repr::Data(_) | Repr::Clear => {
                return None;
            }
            // Operation 5's arguments, which `crate::encode` makes.
            #[cfg(ncdb_rich_text)]
            Repr::RichText(_) => return None,
            // A document is no single value of a change.
            #[cfg(ncdb_rich_text)]
            Repr::RichTextRead(_) | Repr::Markdown(_) => return None,
        })
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

    /// The value a basic field of `kind` holds: a day or a point in time
    /// for those kinds (RFC 0005, "Days and points in time"), otherwise as
    /// [`Value::from_cbor`] reads it. `None` for a value the SDK cannot read
    /// as one: the field reads unset.
    pub(crate) fn from_basic(kind: &Kind, value: &cbor::Value) -> Option<Value> {
        let n = || match Value::from_cbor(value, false)?.0 {
            Repr::Integer(n) => Some(n),
            _ => None,
        };
        match kind {
            Kind::Date => Date::from_days(n()?).map(Value::from),
            Kind::Timestamp => Timestamp::from_millis(n()?).ok().map(Value::from),
            _ => Value::from_cbor(value, false),
        }
    }

    /// Whether a basic field of `kind` takes this value as written: a day
    /// only a [`Date`], a point in time only a [`Timestamp`], and no other
    /// kind either of them. Anything else the merge rule checks.
    pub(crate) fn fits(&self, kind: &Kind) -> bool {
        match (&self.0, kind) {
            (Repr::Clear, _) => true,
            (Repr::Date(_), Kind::Date) | (Repr::Timestamp(_), Kind::Timestamp) => true,
            (Repr::Date(_) | Repr::Timestamp(_), _) | (_, Kind::Date | Kind::Timestamp) => false,
            _ => true,
        }
    }

    /// The reference a field of `ncdb/ref` holds, or `None` for a value
    /// that names no target: the field reads unset (RFC 0004, "The value
    /// and its encoding").
    pub(crate) fn from_reference(value: &cbor::Value) -> Option<Value> {
        let target = ncdb_core::reference::target(value)?;
        Some(Value::from(Reference::new(
            SpaceId(target.space),
            ObjectId(target.object),
        )))
    }
}

/// An `i64` as the CBOR integer RFC 0002's profile writes.
fn integer(n: i64) -> cbor::Value {
    match n >= 0 {
        true => cbor::Value::Unsigned(n.unsigned_abs()),
        false => cbor::Value::Negative((-1 - n).unsigned_abs()),
    }
}
