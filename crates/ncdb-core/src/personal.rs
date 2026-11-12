//! A personal value, sealed under the person key of the person it belongs
//! to, as RFC 0003 defines it ("Personal value"; Carsten, 2026-11-12,
//! NOC-244), and the two values a set of a personal field carries in
//! argument 1 (RFC 0004, "A personal value"; RFC 0005, "Identifying the
//! person").
//!
//! The key is HKDF-SHA256 of the person key, without salt, with the info
//! `"nocentraldb/personal-value"`. The associated data is that context,
//! then the space id, the object id and the person-key id (16 bytes each),
//! then the collection name and the field name, each as its UTF-8 length (2
//! bytes, big-endian) and its bytes. The plaintext is the canonical CBOR of
//! the value. The sealed bytes are nonce (24) || ciphertext || tag (16),
//! XChaCha20-Poly1305 with a random nonce. Once the person key is
//! destroyed, nothing opens the value again: that is the erasure.

use std::collections::BTreeMap;

use chacha20poly1305::XChaCha20Poly1305;
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use hkdf::Hkdf;
use sha2::Sha256;
use zeroize::Zeroizing;

use crate::basic::{self, Values, Write};
use crate::cbor::{self, Key, Value};
use crate::clock::Hlc;
use crate::field::{FieldType, Outcome};
use crate::ids::{MemberId, ObjectId, OperationId, SpaceId};
use crate::keys::{self, PersonKey};
use crate::seal::{NONCE_LEN, TAG_LEN};

/// The context string: the HKDF info of the subkey and the start of the
/// associated data.
pub const CONTEXT: &[u8] = b"nocentraldb/personal-value";

/// The hidden field on the person's object that names the person id and
/// the person-key id in this space (RFC 0005, "Identifying the person").
pub const PERSON_FIELD: &str = "$person";

/// Where a personal value stands: what its associated data binds it to, so
/// it opens only there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Place<'a> {
    /// The space the object is in.
    pub space: SpaceId,
    /// The object the field belongs to.
    pub object: ObjectId,
    /// The id of the person key the value is sealed under.
    pub person_key: [u8; 16],
    /// The object's collection.
    pub collection: &'a str,
    /// The personal field.
    pub field: &'a str,
}

/// Why a sealed personal value does not open. None carries a key or a
/// value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenError {
    /// Shorter than a nonce and a tag.
    TooShort,
    /// It does not open under this key at this place.
    DoesNotOpen,
    /// It opens, but the plaintext is not one canonical CBOR value.
    NotCanonical,
}

/// The associated data of a personal value at `place`.
///
/// # Panics
///
/// When a name is longer than 65,535 bytes, which no schema allows.
pub fn associated_data(place: &Place<'_>) -> Vec<u8> {
    let mut ad =
        Vec::with_capacity(CONTEXT.len() + 48 + 4 + place.collection.len() + place.field.len());
    ad.extend_from_slice(CONTEXT);
    ad.extend_from_slice(&place.space.0);
    ad.extend_from_slice(&place.object.0);
    ad.extend_from_slice(&place.person_key);
    for name in [place.collection, place.field] {
        let length = u16::try_from(name.len()).expect("a name fits in 2 bytes");
        ad.extend_from_slice(&length.to_be_bytes());
        ad.extend_from_slice(name.as_bytes());
    }
    ad
}

fn cipher(key: &PersonKey) -> XChaCha20Poly1305 {
    let mut subkey = Zeroizing::new([0u8; 32]);
    Hkdf::<Sha256>::new(None, key.bytes())
        .expand(CONTEXT, subkey.as_mut())
        .expect("32 bytes are a valid HKDF-SHA256 length");
    XChaCha20Poly1305::new(&(*subkey).into())
}

/// Seals `value` at `place` under `key`, with a random nonce.
///
/// # Panics
///
/// When the platform has no secure random generator.
pub fn seal(key: &PersonKey, place: &Place<'_>, value: &Value) -> Vec<u8> {
    seal_with_nonce(key, place, value, *keys::random::<NONCE_LEN>())
}

/// [`seal`] with a given nonce, for test vectors. A nonce must never be
/// used twice under one key.
pub fn seal_with_nonce(
    key: &PersonKey,
    place: &Place<'_>,
    value: &Value,
    nonce: [u8; NONCE_LEN],
) -> Vec<u8> {
    let plaintext = Zeroizing::new(cbor::encode(value));
    seal_bytes_with_nonce(key, place, &plaintext, nonce)
}

/// Seals `plaintext` as given, which need not be canonical CBOR: for tests
/// of [`open`]. A nonce must never be used twice under one key.
#[doc(hidden)]
pub fn seal_bytes_with_nonce(
    key: &PersonKey,
    place: &Place<'_>,
    plaintext: &[u8],
    nonce: [u8; NONCE_LEN],
) -> Vec<u8> {
    let ciphertext = cipher(key)
        .encrypt(
            &nonce.into(),
            Payload {
                msg: plaintext,
                aad: &associated_data(place),
            },
        )
        .expect("a value is far below XChaCha20-Poly1305's length limit");
    [nonce.as_slice(), &ciphertext].concat()
}

/// Opens `sealed` at `place` under `key`, to the value it carries.
pub fn open(key: &PersonKey, place: &Place<'_>, sealed: &[u8]) -> Result<Value, OpenError> {
    if sealed.len() < NONCE_LEN + TAG_LEN {
        return Err(OpenError::TooShort);
    }
    let (nonce, ciphertext) = sealed.split_at(NONCE_LEN);
    let nonce: [u8; NONCE_LEN] = nonce.try_into().expect("split at the nonce's length");
    let plaintext = Zeroizing::new(
        cipher(key)
            .decrypt(
                &nonce.into(),
                Payload {
                    msg: ciphertext,
                    aad: &associated_data(place),
                },
            )
            .map_err(|_| OpenError::DoesNotOpen)?,
    );
    cbor::decode(&plaintext).map_err(|_| OpenError::NotCanonical)
}

fn id(value: Option<&Value>) -> Option<[u8; 16]> {
    match value {
        Some(Value::Bytes(bytes)) => bytes.as_slice().try_into().ok(),
        _ => None,
    }
}

fn two_keys(value: &Value) -> Option<&BTreeMap<Key, Value>> {
    match value {
        Value::Map(map) if map.len() == 2 => Some(map),
        _ => None,
    }
}

/// A sealed personal value as argument 1 of a set carries it:
/// `{1: person-key id, 2: sealed bytes}` (RFC 0004, "A personal value").
#[derive(Clone, PartialEq, Eq)]
pub struct Sealed {
    /// The id of the person key it is sealed under.
    pub person_key: [u8; 16],
    /// Nonce || ciphertext || tag.
    pub bytes: Vec<u8>,
}

impl std::fmt::Debug for Sealed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sealed")
            .field("bytes", &self.bytes.len())
            .finish_non_exhaustive()
    }
}

impl Sealed {
    /// The map argument 1 carries.
    pub fn to_value(&self) -> Value {
        Value::Map(BTreeMap::from([
            (Key::Unsigned(1), Value::Bytes(self.person_key.to_vec())),
            (Key::Unsigned(2), Value::Bytes(self.bytes.clone())),
        ]))
    }

    /// The sealed value in argument 1, or `None` when it is not one: a
    /// plain `null` clears the field.
    pub fn from_value(value: &Value) -> Option<Sealed> {
        let map = two_keys(value)?;
        let person_key = id(map.get(&Key::Unsigned(1)))?;
        match map.get(&Key::Unsigned(2)) {
            Some(Value::Bytes(bytes)) => Some(Sealed {
                person_key,
                bytes: bytes.clone(),
            }),
            _ => None,
        }
    }
}

/// The value of [`PERSON_FIELD`]: `{1: person id, 2: person-key id}` (RFC
/// 0005, "Identifying the person").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Person {
    /// The person's id in this space.
    pub person: [u8; 16],
    /// The id of the person's key in this space.
    pub person_key: [u8; 16],
}

impl Person {
    /// The map argument 1 of a set of [`PERSON_FIELD`] carries.
    pub fn to_value(&self) -> Value {
        Value::Map(BTreeMap::from([
            (Key::Unsigned(1), Value::Bytes(self.person.to_vec())),
            (Key::Unsigned(2), Value::Bytes(self.person_key.to_vec())),
        ]))
    }

    /// The person in argument 1, or `None` when it is not one.
    pub fn from_value(value: &Value) -> Option<Person> {
        let map = two_keys(value)?;
        Some(Person {
            person: id(map.get(&Key::Unsigned(1)))?,
            person_key: id(map.get(&Key::Unsigned(2)))?,
        })
    }
}

/// The field type of the hidden field [`PERSON_FIELD`]: a basic value whose
/// values are [`Person`]s (RFC 0005, "Where the person id is kept").
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PersonField;

impl PersonField {
    /// A new write of `person`, or a clear with `None`, which replaces
    /// what `state` holds: the winner and every conflict.
    pub fn set(
        &self,
        state: &basic::State,
        id: OperationId,
        author: MemberId,
        stamp: Hlc,
        person: Option<Person>,
    ) -> Write {
        let value = person.map_or(Value::Null, |person| person.to_value());
        Values::Persons
            .set(state, id, author, stamp, value)
            .expect("a person or the empty value is the field's value")
    }
}

impl FieldType for PersonField {
    type State = basic::State;
    type Operation = Write;
    type Read = basic::Read;

    fn apply(&self, state: &mut basic::State, op: &Write) {
        state.insert(op);
    }

    fn state_bytes(&self, state: &basic::State) -> Vec<u8> {
        state.bytes()
    }

    fn read(&self, state: &basic::State) -> basic::Read {
        Values::Persons.read(state)
    }

    fn outcome(&self, state: &basic::State, id: OperationId) -> Option<Outcome> {
        state.outcome(id)
    }
}
