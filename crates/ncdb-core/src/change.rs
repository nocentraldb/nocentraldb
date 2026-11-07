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
//! understand", keys from 64 up "may ignore". A key this core does not know
//! is kept and encoded again unchanged; one from 1 to 63, a higher header or
//! payload format, or an unknown kind makes the change not foldable.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::cbor::{DecodeError, Value};
use crate::clock::Hlc;
use crate::ids::{ChangeId, MemberId, ObjectId, OperationId, SpaceId};

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
        todo!()
    }
}

/// Unknown keys of a header, payload or operation map, kept to be encoded
/// again unchanged.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
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

    /// Whether any unknown key is "must understand" (1 to 63).
    pub fn must_understand(&self) -> bool {
        todo!()
    }
}

/// The header of a change: what the relay reads.
#[derive(Clone, Debug, PartialEq, Eq)]
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

/// The payload of a change, after decryption: only members read it.
#[derive(Clone, Debug, PartialEq, Eq)]
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
#[derive(Clone, Debug, PartialEq, Eq)]
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

/// A change: the frame's exact bytes, its header, signature and id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    bytes: Vec<u8>,
    header: Header,
    id: ChangeId,
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
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!()
    }
}

impl std::error::Error for ChangeError {}

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
    pub fn insert_extension(&mut self, _key: u64, _value: Value) -> bool {
        todo!()
    }

    /// Whether this core can fold the change: a known header format and
    /// kind, and no unknown "must understand" key.
    pub fn foldable(&self) -> bool {
        todo!()
    }

    /// The canonical bytes of the header map.
    pub fn encode(&self) -> Vec<u8> {
        todo!()
    }

    /// Decodes canonical bytes of a header map.
    pub fn decode(_bytes: &[u8]) -> Result<Header, ChangeError> {
        todo!()
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
    pub fn insert_extension(&mut self, _key: u64, _value: Value) -> bool {
        todo!()
    }

    /// Whether this core can fold the payload: a known payload format and no
    /// unknown "must understand" key, in the payload or any operation.
    pub fn foldable(&self) -> bool {
        todo!()
    }

    /// The canonical bytes of the payload map.
    pub fn encode(&self) -> Vec<u8> {
        todo!()
    }

    /// Decodes canonical bytes of a payload map.
    pub fn decode(_bytes: &[u8]) -> Result<Payload, ChangeError> {
        todo!()
    }
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
    pub fn insert_extension(&mut self, _key: u64, _value: Value) -> bool {
        todo!()
    }
}

impl Change {
    /// The frame version, fixed forever.
    pub const FRAME_VERSION: u64 = 1;

    /// The change of this header and signature.
    pub fn new(_header: Header, _signature: Vec<u8>) -> Change {
        todo!()
    }

    /// Decodes the canonical bytes of a frame. The change keeps exactly
    /// these bytes.
    pub fn decode(_bytes: &[u8]) -> Result<Change, ChangeError> {
        todo!()
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
        todo!()
    }

    /// The signature, as the frame holds it.
    pub fn signature(&self) -> &[u8] {
        todo!()
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
