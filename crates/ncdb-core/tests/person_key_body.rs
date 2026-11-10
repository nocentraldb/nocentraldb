//! The bodies of RFC 0008's person-key entry (entry type 4) and shred entry
//! (entry type 5), the account sign-off of RFC 0003 ("Account sign-off of
//! a shred") and the signed person-key record of RFC 0007 ("Keys and the
//! person-key store").

#[path = "support/space.rs"]
mod space;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::{BTreeMap, BTreeSet};

use ed25519_dalek::SigningKey;
use ncdb_core::cbor::{self, Key, Value};
use ncdb_core::change::ChangeError;
use ncdb_core::ids::{ChangeId, MemberId, SpaceId};
use ncdb_core::membership::{
    BodyError, ControlBody, Entry, PersonKeyEntry, ShredEntry, SignOff, record_hash, sign_record,
    verify_record,
};
use sha2::{Digest, Sha256};

use space::key;

const SPACE: SpaceId = SpaceId([0x5a; 16]);
const PERSON: [u8; 16] = [0x9e; 16];

fn deps() -> BTreeSet<ChangeId> {
    BTreeSet::from([ChangeId([0x11; 32])])
}

fn person_key_entry() -> PersonKeyEntry {
    PersonKeyEntry {
        person_key: PERSON,
        commitment: [0xc0; 32],
        generation: 0x0001_2345_6789_abcd,
        record_hash: [0x4a; 32],
    }
}

/// A map with unsigned keys, as the body is written.
fn map(entries: Vec<(u64, Value)>) -> Value {
    Value::Map(
        entries
            .into_iter()
            .map(|(k, v)| (Key::Unsigned(k), v))
            .collect::<BTreeMap<_, _>>(),
    )
}

fn deps_value() -> Value {
    Value::Array(vec![Value::Bytes(vec![0x11; 32])])
}

#[test]
fn a_person_key_entry_carries_its_id_commitment_generation_and_record_hash() {
    let body = ControlBody::new(deps(), Entry::PersonKey(person_key_entry()));
    let expected = map(vec![
        (1, Value::Unsigned(4)),
        (3, deps_value()),
        (8, Value::Bytes(PERSON.to_vec())),
        (9, Value::Bytes(vec![0xc0; 32])),
        (10, Value::Unsigned(0x0001_2345_6789_abcd)),
        (11, Value::Bytes(vec![0x4a; 32])),
    ]);
    assert_eq!(body.encode(), cbor::encode(&expected));
    assert_eq!(body.entry_type(), 4);
    assert_eq!(ControlBody::decode(&body.encode()), Ok(body));
}

#[test]
fn a_person_key_entry_without_a_field_or_with_a_short_one_is_malformed() {
    let full = || {
        vec![
            (1, Value::Unsigned(4)),
            (3, deps_value()),
            (8, Value::Bytes(PERSON.to_vec())),
            (9, Value::Bytes(vec![0xc0; 32])),
            (10, Value::Unsigned(7)),
            (11, Value::Bytes(vec![0x4a; 32])),
        ]
    };
    for missing in 8..=11 {
        let mut entries = full();
        entries.retain(|(k, _)| *k != missing);
        let bytes = cbor::encode(&map(entries));
        assert_eq!(
            ControlBody::decode(&bytes),
            Err(BodyError::Malformed(ChangeError::MissingKey(missing))),
            "without key {missing}"
        );
    }
    for (short, value) in [
        (8, Value::Bytes(vec![0x9e; 15])),
        (9, Value::Bytes(vec![0xc0; 31])),
        (10, Value::Bytes(vec![7])),
        (11, Value::Unsigned(1)),
    ] {
        let mut entries = full();
        entries.retain(|(k, _)| *k != short);
        entries.push((short, value));
        entries.sort_by_key(|(k, _)| *k);
        let bytes = cbor::encode(&map(entries));
        assert_eq!(
            ControlBody::decode(&bytes),
            Err(BodyError::Malformed(ChangeError::InvalidValue(short))),
            "key {short} of the wrong shape"
        );
    }
}

#[test]
fn a_person_key_entry_with_a_key_it_does_not_define_is_not_understood() {
    let body = ControlBody::new(deps(), Entry::PersonKey(person_key_entry()));
    let Value::Map(mut entries) = cbor::decode(&body.encode()).unwrap() else {
        panic!("a map")
    };
    entries.insert(Key::Unsigned(12), Value::Unsigned(1));
    let bytes = cbor::encode(&Value::Map(entries));
    assert_eq!(ControlBody::decode(&bytes), Err(BodyError::NotUnderstood));
}

fn account() -> SigningKey {
    SigningKey::from_bytes(&[0xAD; 32])
}

fn performer() -> MemberId {
    MemberId([0x77; 32])
}

#[test]
fn the_sign_off_signs_the_hash_of_space_person_key_and_performer() {
    let sign_off = SignOff::sign(&account(), SPACE, PERSON, performer());
    let mut hash = Sha256::new();
    hash.update(b"nocentraldb/shred-sign-off");
    hash.update(SPACE.0);
    hash.update(PERSON);
    hash.update(performer().0);
    let digest: [u8; 32] = hash.finalize().into();
    assert_eq!(SignOff::digest(SPACE, PERSON, performer()), digest);
    assert_eq!(sign_off.account_key, account().verifying_key().to_bytes());
    let signature = ed25519_dalek::Signature::from_bytes(&sign_off.signature);
    assert!(
        account()
            .verifying_key()
            .verify_strict(&digest, &signature)
            .is_ok()
    );
    assert!(sign_off.verify(SPACE, PERSON, performer()));
}

#[test]
fn a_sign_off_verifies_only_for_its_space_person_key_and_performer() {
    let sign_off = SignOff::sign(&account(), SPACE, PERSON, performer());
    assert!(!sign_off.verify(SpaceId([0x5b; 16]), PERSON, performer()));
    assert!(!sign_off.verify(SPACE, [0x9f; 16], performer()));
    assert!(!sign_off.verify(SPACE, PERSON, MemberId([0x78; 32])));
    let mut forged = sign_off.clone();
    forged.signature[0] ^= 1;
    assert!(!forged.verify(SPACE, PERSON, performer()));
}

#[test]
fn a_shred_entry_carries_its_person_key_id_and_the_sign_off_map() {
    let sign_off = SignOff::sign(&account(), SPACE, PERSON, performer());
    let shred = ShredEntry {
        person_key: PERSON,
        sign_off: sign_off.clone(),
    };
    let body = ControlBody::new(deps(), Entry::Shred(shred));
    let expected = map(vec![
        (1, Value::Unsigned(5)),
        (3, deps_value()),
        (8, Value::Bytes(PERSON.to_vec())),
        (
            9,
            map(vec![
                (1, Value::Bytes(sign_off.account_key.to_vec())),
                (2, Value::Bytes(sign_off.signature.to_vec())),
            ]),
        ),
    ]);
    assert_eq!(body.encode(), cbor::encode(&expected));
    assert_eq!(body.entry_type(), 5);
    assert_eq!(ControlBody::decode(&body.encode()), Ok(body));
}

#[test]
fn a_shred_entry_with_a_sign_off_of_the_wrong_shape_is_malformed() {
    let sign_off = |entries: Vec<(u64, Value)>| {
        cbor::encode(&map(vec![
            (1, Value::Unsigned(5)),
            (3, deps_value()),
            (8, Value::Bytes(PERSON.to_vec())),
            (9, map(entries)),
        ]))
    };
    let key = || (1, Value::Bytes(vec![1; 32]));
    let signature = || (2, Value::Bytes(vec![2; 64]));
    for entries in [
        vec![key()],
        vec![signature()],
        vec![key(), (2, Value::Bytes(vec![2; 63]))],
        vec![key(), signature(), (3, Value::Unsigned(0))],
    ] {
        assert_eq!(
            ControlBody::decode(&sign_off(entries)),
            Err(BodyError::Malformed(ChangeError::InvalidValue(9)))
        );
    }
    let without = cbor::encode(&map(vec![
        (1, Value::Unsigned(5)),
        (3, deps_value()),
        (8, Value::Bytes(PERSON.to_vec())),
    ]));
    assert_eq!(
        ControlBody::decode(&without),
        Err(BodyError::Malformed(ChangeError::MissingKey(9)))
    );
}

#[test]
fn a_record_hash_is_the_sha_256_of_the_wrapped_bytes() {
    let wrapped = b"a wrapped person key".to_vec();
    let expected: [u8; 32] = Sha256::digest(&wrapped).into();
    assert_eq!(record_hash(&wrapped), expected);
}

#[test]
fn a_record_is_signed_by_its_writer_over_its_clear_fields_and_wrapped_bytes() {
    let writer = key(5);
    let wrapped = vec![0xee; 80];
    let generation = 0x0001_0000_0000_0042;
    let signature = sign_record(&writer, SPACE, PERSON, generation, &wrapped);
    let public = writer.public();
    assert!(verify_record(&public, SPACE, PERSON, generation, &wrapped, &signature));
    // Another space, person key, generation, writer or wrapped key fails.
    assert!(!verify_record(&public, SpaceId([1; 16]), PERSON, generation, &wrapped, &signature));
    assert!(!verify_record(&public, SPACE, [1; 16], generation, &wrapped, &signature));
    assert!(!verify_record(&public, SPACE, PERSON, generation + 1, &wrapped, &signature));
    assert!(!verify_record(&key(6).public(), SPACE, PERSON, generation, &wrapped, &signature));
    assert!(!verify_record(&public, SPACE, PERSON, generation, &[0xef; 80], &signature));
    assert!(!verify_record(&public, SPACE, PERSON, generation, &wrapped, &signature[..63]));
}
