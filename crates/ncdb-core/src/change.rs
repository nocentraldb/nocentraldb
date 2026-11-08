//! The change, as RFC 0002 defines it ("A change at a glance").
//!
//! A change is a frame, `[frame version, signed header bytes, signature]`.
//! The header is a map the relay reads; its key 6 holds the payload, an AEAD
//! ciphertext (kind 0) or a control body (kind 1). The payload map, after
//! decryption, holds the clock, the dependencies and the operations.
//!
//! Sealing and signing are RFC 0003's and NOC-15's: here the signature and
//! the sealed payload are opaque bytes, and [`Payload`] encodes and decodes
//! the plaintext map.
//!
//! Keys 1 to 63 of the header, payload and operation maps are "must
//! understand", keys from 64 up "may ignore". RFC 0002 does not place key 0;
//! this core treats it as "must understand" too. A key this core does not
//! know is kept and encoded again unchanged; one from 0 to 63, a higher
//! header or payload format, or an unknown kind makes the change not
//! foldable.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::cbor::{self, DecodeError, Key, Value};
use crate::clock::Hlc;
use crate::ids::{ChangeId, MemberId, ObjectId, OperationId, SpaceId};
use crate::redact::Size;

/// The depth of the header map in a change: inside the frame.
const HEADER_DEPTH: usize = 2;
/// The depth of the payload map in a change: inside the header's key 6.
const PAYLOAD_DEPTH: usize = 3;

/// The highest key of the "must understand" range.
pub const LAST_MUST_UNDERSTAND_KEY: u64 = 63;

/// What a change's payload is (header key 5).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Kind(pub u64);

impl Kind {
    /// The payload is an AEAD ciphertext: content or a work order.
    pub const SEALED: Kind = Kind(0);
    /// The payload is a control body the relay reads (RFC 0003).
    pub const CONTROL: Kind = Kind(1);

    /// Whether this core knows the kind.
    pub fn is_known(self) -> bool {
        self == Kind::SEALED || self == Kind::CONTROL
    }
}

/// Unknown keys of a header, payload or operation map, kept to be encoded
/// again unchanged.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Extensions(BTreeMap<u64, Value>);

impl Extensions {
    /// No unknown keys.
    pub fn new() -> Extensions {
        Extensions::default()
    }

    /// The unknown keys and their values, in key order.
    pub fn iter(&self) -> impl Iterator<Item = (u64, &Value)> {
        self.0.iter().map(|(key, value)| (*key, value))
    }

    /// Whether any unknown key is "must understand" (0 to 63).
    pub fn must_understand(&self) -> bool {
        self.0.range(..=LAST_MUST_UNDERSTAND_KEY).next().is_some()
    }

    /// Adds an unknown key, unless the map defines it (`1..=known`).
    fn insert(&mut self, known: u64, key: u64, value: Value) -> bool {
        if (1..=known).contains(&key) {
            return false;
        }
        self.0.insert(key, value);
        true
    }
}

/// The header of a change: what the relay reads. Its `Debug` output shows
/// the payload only by its length.
#[derive(Clone, PartialEq, Eq)]
pub struct Header {
    /// Key 1: the space the change belongs to.
    pub space: SpaceId,
    /// Key 2: the member whose key signed the change.
    pub author: MemberId,
    /// Key 3: 1, 2, 3, ... per author and space.
    pub sequence: u64,
    /// Key 4: the space key generation that sealed the payload.
    pub key_generation: u64,
    /// Key 5: sealed or control.
    pub kind: Kind,
    /// Key 6: the AEAD ciphertext, or the control body.
    pub payload: Vec<u8>,
    /// Key 7: the header format.
    pub format: u64,
    /// Key 8: the suite id (RFC 0003).
    pub suite: u64,
    /// Keys this core does not know.
    extensions: Extensions,
}

/// The payload of a change, after decryption: only members read it. Its
/// `Debug` output shows the ordering metadata and how many operations it
/// holds, not the operations.
#[derive(Clone, PartialEq, Eq)]
pub struct Payload {
    /// Key 1: the payload format.
    pub format: u64,
    /// Key 2: the hybrid logical clock.
    pub clock: Hlc,
    /// Key 3: the author's heads when it wrote, sorted and without repeats.
    pub dependencies: BTreeSet<ChangeId>,
    /// Key 4: the operations, applied in order.
    pub operations: Vec<Operation>,
    /// Key 5: padding, absent or a byte string the reader ignores.
    pub padding: Option<Vec<u8>>,
    /// Keys this core does not know.
    extensions: Extensions,
}

/// One edit inside a payload: one operation on one field of one object.
/// Its `Debug` output shows only its unknown keys: everything else is
/// content.
#[derive(Clone, PartialEq, Eq)]
pub struct Operation {
    /// Key 1: the object.
    pub object: ObjectId,
    /// Key 2: the collection's name in `model.lock.json`.
    pub collection: String,
    /// Key 3: the field's name, absent for "create the object" (op 0).
    pub field: Option<String>,
    /// Key 4: the operation, as the field type defines it.
    pub op: u64,
    /// Key 5: the value, as the field type defines it.
    pub value: Option<Value>,
    /// Keys this core does not know.
    extensions: Extensions,
}

/// A change: the frame's exact bytes, its header, signature and id. Its
/// `Debug` output shows the id and the header, and the signature only by
/// its length.
#[derive(Clone, PartialEq, Eq)]
pub struct Change {
    bytes: Vec<u8>,
    header: Header,
    header_bytes: Vec<u8>,
    signature: Vec<u8>,
    id: ChangeId,
}

impl fmt::Debug for Extensions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let keys: Vec<u64> = self.0.keys().copied().collect();
        f.debug_struct("Extensions").field("keys", &keys).finish()
    }
}

impl fmt::Debug for Header {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Header")
            .field("space", &self.space)
            .field("author", &self.author)
            .field("sequence", &self.sequence)
            .field("key_generation", &self.key_generation)
            .field("kind", &self.kind)
            .field("payload", &Size(self.payload.len(), "bytes"))
            .field("format", &self.format)
            .field("suite", &self.suite)
            .field("extensions", &self.extensions)
            .finish()
    }
}

impl fmt::Debug for Payload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Payload")
            .field("format", &self.format)
            .field("clock", &self.clock)
            .field("dependencies", &self.dependencies)
            .field("operations", &Size(self.operations.len(), "operations"))
            .field(
                "padding",
                &self.padding.as_ref().map(|p| Size(p.len(), "bytes")),
            )
            .field("extensions", &self.extensions)
            .finish()
    }
}

impl fmt::Debug for Operation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Operation")
            .field("extensions", &self.extensions)
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for Change {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Change")
            .field("id", &self.id)
            .field("header", &self.header)
            .field("signature", &Size(self.signature.len(), "bytes"))
            .finish()
    }
}

/// Why bytes are not a change, header, payload or operation. Names keys
/// and positions, never their content.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeError {
    /// The bytes are not canonical CBOR of the profile.
    Encoding(DecodeError),
    /// The frame is not an array of three items.
    FrameShape,
    /// The frame version is not 1.
    FrameVersion(u64),
    /// A map where a map was expected is something else.
    NotAMap,
    /// A map has a text key where only unsigned keys are allowed.
    TextKey,
    /// A required key is missing.
    MissingKey(u64),
    /// A key holds a value of the wrong type or length.
    InvalidValue(u64),
    /// The dependencies are not sorted by bytes, or repeat an id.
    UnsortedDependencies,
}

impl fmt::Display for ChangeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChangeError::Encoding(error) => error.fmt(f),
            ChangeError::FrameShape => {
                f.write_str("the frame is not [version, header bytes, signature]")
            }
            ChangeError::FrameVersion(version) => write!(f, "unknown frame version {version}"),
            ChangeError::NotAMap => f.write_str("expected a map"),
            ChangeError::TextKey => f.write_str("a text key where only unsigned keys are allowed"),
            ChangeError::MissingKey(key) => write!(f, "key {key} is missing"),
            ChangeError::InvalidValue(key) => {
                write!(f, "key {key} holds a value of the wrong type or length")
            }
            ChangeError::UnsortedDependencies => {
                f.write_str("the dependencies are not sorted, or repeat an id")
            }
        }
    }
}

impl std::error::Error for ChangeError {}

impl From<DecodeError> for ChangeError {
    fn from(error: DecodeError) -> Self {
        ChangeError::Encoding(error)
    }
}

/// The entries of a map with unsigned keys, taken out one by one; what is
/// left are the unknown keys.
struct Fields(BTreeMap<u64, Value>);

impl Fields {
    fn decode(bytes: &[u8], depth: usize) -> Result<Fields, ChangeError> {
        Fields::of(cbor::decode_at(bytes, depth)?)
    }

    fn of(value: Value) -> Result<Fields, ChangeError> {
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

    fn optional(&mut self, key: u64) -> Option<(u64, Value)> {
        self.0.remove(&key).map(|value| (key, value))
    }

    fn required(&mut self, key: u64) -> Result<(u64, Value), ChangeError> {
        self.optional(key).ok_or(ChangeError::MissingKey(key))
    }

    fn rest(self) -> Extensions {
        Extensions(self.0)
    }
}

fn unsigned((key, value): (u64, Value)) -> Result<u64, ChangeError> {
    match value {
        Value::Unsigned(n) => Ok(n),
        _ => Err(ChangeError::InvalidValue(key)),
    }
}

fn bytes((key, value): (u64, Value)) -> Result<Vec<u8>, ChangeError> {
    match value {
        Value::Bytes(bytes) => Ok(bytes),
        _ => Err(ChangeError::InvalidValue(key)),
    }
}

fn fixed<const N: usize>(entry: (u64, Value)) -> Result<[u8; N], ChangeError> {
    let key = entry.0;
    bytes(entry)?
        .try_into()
        .map_err(|_| ChangeError::InvalidValue(key))
}

fn text((key, value): (u64, Value)) -> Result<String, ChangeError> {
    match value {
        Value::Text(text) => Ok(text),
        _ => Err(ChangeError::InvalidValue(key)),
    }
}

/// A map of known entries and unknown keys, in key order.
fn map(known: Vec<(u64, Value)>, extensions: &Extensions) -> Value {
    let unknown = extensions.iter().map(|(key, value)| (key, value.clone()));
    Value::Map(
        known
            .into_iter()
            .chain(unknown)
            .map(|(key, value)| (Key::Unsigned(key), value))
            .collect(),
    )
}

impl Header {
    /// The current header format.
    pub const FORMAT: u64 = 1;

    /// A header of the current format, without unknown keys.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        space: SpaceId,
        author: MemberId,
        sequence: u64,
        key_generation: u64,
        kind: Kind,
        payload: Vec<u8>,
        suite: u64,
    ) -> Header {
        Header {
            space,
            author,
            sequence,
            key_generation,
            kind,
            payload,
            format: Header::FORMAT,
            suite,
            extensions: Extensions::new(),
        }
    }

    /// The keys this core does not know.
    pub fn extensions(&self) -> &Extensions {
        &self.extensions
    }

    /// Adds a key this core does not know, as a newer client would write
    /// it. Returns `false`, and changes nothing, for a key this core knows.
    pub fn insert_extension(&mut self, key: u64, value: Value) -> bool {
        self.extensions.insert(8, key, value)
    }

    /// Whether this core can fold the change: a known header format and
    /// kind, and no unknown "must understand" key.
    pub fn foldable(&self) -> bool {
        self.format == Header::FORMAT && self.kind.is_known() && !self.extensions.must_understand()
    }

    /// The canonical bytes of the header map.
    pub fn encode(&self) -> Vec<u8> {
        let known = vec![
            (1, Value::Bytes(self.space.0.to_vec())),
            (2, Value::Bytes(self.author.0.to_vec())),
            (3, Value::Unsigned(self.sequence)),
            (4, Value::Unsigned(self.key_generation)),
            (5, Value::Unsigned(self.kind.0)),
            (6, Value::Bytes(self.payload.clone())),
            (7, Value::Unsigned(self.format)),
            (8, Value::Unsigned(self.suite)),
        ];
        cbor::encode(&map(known, &self.extensions))
    }

    /// Decodes canonical bytes of a header map. The header sits at depth
    /// 2, inside the frame, as [`cbor::MAX_DEPTH`] counts.
    pub fn decode(bytes: &[u8]) -> Result<Header, ChangeError> {
        let mut fields = Fields::decode(bytes, HEADER_DEPTH)?;
        Ok(Header {
            space: SpaceId(fixed(fields.required(1)?)?),
            author: MemberId(fixed(fields.required(2)?)?),
            sequence: unsigned(fields.required(3)?)?,
            key_generation: unsigned(fields.required(4)?)?,
            kind: Kind(unsigned(fields.required(5)?)?),
            payload: self::bytes(fields.required(6)?)?,
            format: unsigned(fields.required(7)?)?,
            suite: unsigned(fields.required(8)?)?,
            extensions: fields.rest(),
        })
    }
}

impl Payload {
    /// The current payload format.
    pub const FORMAT: u64 = 1;

    /// A payload of the current format, without padding or unknown keys.
    pub fn new(
        clock: Hlc,
        dependencies: BTreeSet<ChangeId>,
        operations: Vec<Operation>,
    ) -> Payload {
        Payload {
            format: Payload::FORMAT,
            clock,
            dependencies,
            operations,
            padding: None,
            extensions: Extensions::new(),
        }
    }

    /// The keys this core does not know.
    pub fn extensions(&self) -> &Extensions {
        &self.extensions
    }

    /// Adds a key this core does not know. Returns `false`, and changes
    /// nothing, for a key this core knows.
    pub fn insert_extension(&mut self, key: u64, value: Value) -> bool {
        self.extensions.insert(5, key, value)
    }

    /// Whether this core can fold the payload: a known payload format and no
    /// unknown "must understand" key, in the payload or any operation.
    pub fn foldable(&self) -> bool {
        self.format == Payload::FORMAT
            && !self.extensions.must_understand()
            && !self
                .operations
                .iter()
                .any(|operation| operation.extensions.must_understand())
    }

    /// The canonical bytes of the payload map.
    pub fn encode(&self) -> Vec<u8> {
        let mut known = vec![
            (1, Value::Unsigned(self.format)),
            (2, Value::Unsigned(self.clock.0)),
            (
                3,
                Value::Array(
                    self.dependencies
                        .iter()
                        .map(|id| Value::Bytes(id.0.to_vec()))
                        .collect(),
                ),
            ),
            (
                4,
                Value::Array(self.operations.iter().map(Operation::to_value).collect()),
            ),
        ];
        if let Some(padding) = &self.padding {
            known.push((5, Value::Bytes(padding.clone())));
        }
        cbor::encode(&map(known, &self.extensions))
    }

    /// Decodes canonical bytes of a payload map. The payload sits at depth
    /// 3, inside the header's key 6, as [`cbor::MAX_DEPTH`] counts.
    pub fn decode(bytes: &[u8]) -> Result<Payload, ChangeError> {
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
}

/// The dependencies, which the encoding holds sorted by bytes and without
/// repeats.
fn dependencies((key, value): (u64, Value)) -> Result<BTreeSet<ChangeId>, ChangeError> {
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

impl Operation {
    /// The op that creates an object; it takes no field.
    pub const CREATE: u64 = 0;

    /// An operation without unknown keys.
    pub fn new(
        object: ObjectId,
        collection: String,
        field: Option<String>,
        op: u64,
        value: Option<Value>,
    ) -> Operation {
        Operation {
            object,
            collection,
            field,
            op,
            value,
            extensions: Extensions::new(),
        }
    }

    /// The keys this core does not know.
    pub fn extensions(&self) -> &Extensions {
        &self.extensions
    }

    /// Adds a key this core does not know. Returns `false`, and changes
    /// nothing, for a key this core knows.
    pub fn insert_extension(&mut self, key: u64, value: Value) -> bool {
        self.extensions.insert(5, key, value)
    }

    fn to_value(&self) -> Value {
        let mut known = vec![
            (1, Value::Bytes(self.object.0.to_vec())),
            (2, Value::Text(self.collection.clone())),
        ];
        if let Some(field) = &self.field {
            known.push((3, Value::Text(field.clone())));
        }
        known.push((4, Value::Unsigned(self.op)));
        if let Some(value) = &self.value {
            known.push((5, value.clone()));
        }
        map(known, &self.extensions)
    }

    fn from_value(value: Value) -> Result<Operation, ChangeError> {
        let mut fields = Fields::of(value).map_err(|_| ChangeError::InvalidValue(4))?;
        Ok(Operation {
            object: ObjectId(fixed(fields.required(1)?)?),
            collection: text(fields.required(2)?)?,
            field: fields.optional(3).map(text).transpose()?,
            op: unsigned(fields.required(4)?)?,
            value: fields.optional(5).map(|(_, value)| value),
            extensions: fields.rest(),
        })
    }
}

impl Change {
    /// The frame version, fixed forever.
    pub const FRAME_VERSION: u64 = 1;

    /// The change of this header and signature.
    pub fn new(header: Header, signature: Vec<u8>) -> Change {
        let header_bytes = header.encode();
        let bytes = cbor::encode(&Value::Array(vec![
            Value::Unsigned(Change::FRAME_VERSION),
            Value::Bytes(header_bytes.clone()),
            Value::Bytes(signature.clone()),
        ]));
        Change::from_parts(bytes, header, header_bytes, signature)
    }

    /// Decodes the canonical bytes of a frame. The change keeps exactly
    /// these bytes.
    pub fn decode(bytes: &[u8]) -> Result<Change, ChangeError> {
        let Value::Array(items) = cbor::decode(bytes)? else {
            return Err(ChangeError::FrameShape);
        };
        let Ok([version, header_bytes, signature]) = <[Value; 3]>::try_from(items) else {
            return Err(ChangeError::FrameShape);
        };
        let (Value::Unsigned(version), Value::Bytes(header_bytes), Value::Bytes(signature)) =
            (version, header_bytes, signature)
        else {
            return Err(ChangeError::FrameShape);
        };
        if version != Change::FRAME_VERSION {
            return Err(ChangeError::FrameVersion(version));
        }
        let header = Header::decode(&header_bytes)?;
        Ok(Change::from_parts(
            bytes.to_vec(),
            header,
            header_bytes,
            signature,
        ))
    }

    fn from_parts(
        bytes: Vec<u8>,
        header: Header,
        header_bytes: Vec<u8>,
        signature: Vec<u8>,
    ) -> Change {
        let version = u8::try_from(Change::FRAME_VERSION).expect("the frame version is 1");
        let id = ChangeId::compute(version, &header_bytes);
        Change {
            bytes,
            header,
            header_bytes,
            signature,
            id,
        }
    }

    /// The frame's exact bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The header.
    pub fn header(&self) -> &Header {
        &self.header
    }

    /// The signed header bytes, as the frame holds them.
    pub fn header_bytes(&self) -> &[u8] {
        &self.header_bytes
    }

    /// The signature, as the frame holds it.
    pub fn signature(&self) -> &[u8] {
        &self.signature
    }

    /// The change id.
    pub fn id(&self) -> ChangeId {
        self.id
    }

    /// The id of the operation at this position of the change's payload.
    pub fn operation_id(&self, position: u64) -> OperationId {
        OperationId {
            change: self.id,
            position,
        }
    }

    /// Whether this core can fold the change, as far as the header tells.
    pub fn foldable(&self) -> bool {
        self.header.foldable()
    }
}
