//! The encrypted previous key of a key chain entry, as the owner decided in
//! NOC-97 (Carsten, 2026-11-10): nonce (24) ‖ ciphertext (32) ‖ tag (16),
//! 72 bytes, XChaCha20-Poly1305 with a random nonce under RFC 0003's
//! key-chain subkey, against the vectors tests/vectors/membership/
//! generate.py writes with cryptography and PyNaCl, independently of this
//! crate.

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::{BTreeMap, BTreeSet};

use ncdb_core::ids::{ChangeId, SpaceId};
use ncdb_core::keys::{MemberKey, SpaceKey};
use ncdb_core::membership::{
    self, ControlBody, Entry, KeyChainEntry, MergeRotation, PREVIOUS_KEY_LEN, PreviousKey,
};

const SPACE_ID: &str = include_str!("vectors/membership/space-id.hex");
const FIRST_GENERATION: &str = include_str!("vectors/membership/first-generation.hex");
const SECOND_GENERATION: &str = include_str!("vectors/membership/second-generation.hex");
const PREVIOUS_FIELD: &str = include_str!("vectors/membership/key-chain-previous.hex");
const KEY_CHAIN_BODY: &str = include_str!("vectors/membership/key-chain.body.hex");
const ADD_ID: &str = include_str!("vectors/membership/add.id.hex");

fn hex(text: &str) -> Vec<u8> {
    let text: String = text.split_whitespace().collect();
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

fn space() -> SpaceId {
    SpaceId(hex(SPACE_ID).try_into().unwrap())
}

fn generation(text: &str) -> u64 {
    u64::from_be_bytes(hex(text).try_into().unwrap())
}

fn generation_1_key() -> SpaceKey {
    SpaceKey::from_bytes([0x41; 32])
}

fn generation_2_key() -> SpaceKey {
    SpaceKey::from_bytes([0x42; 32])
}

fn creator() -> MemberKey {
    MemberKey::from_secrets([0x21; 32], [0x22; 32])
}

fn nonce() -> [u8; 24] {
    core::array::from_fn(|i| 0x81 + i as u8)
}

/// Whether two space keys are the same key: they seal the same bytes with
/// the same nonce. `SpaceKey` has no equality of its own, as a secret.
fn same(a: &SpaceKey, b: &SpaceKey) -> bool {
    let probe =
        |key| membership::seal_previous_key_with_nonce(key, SpaceId([9; 16]), 2, 1, key, [7; 24]);
    probe(a) == probe(b)
}

fn field() -> [u8; PREVIOUS_KEY_LEN] {
    hex(PREVIOUS_FIELD).try_into().expect("72 bytes")
}

#[test]
fn the_second_generation_derives_as_the_vector_says() {
    let first = generation(FIRST_GENERATION);
    let derived = membership::generation_id(space(), creator().id(), &[first]);
    assert_eq!(derived, generation(SECOND_GENERATION));
}

#[test]
fn a_previous_key_seals_to_the_vectors_bytes() {
    let sealed = membership::seal_previous_key_with_nonce(
        &generation_2_key(),
        space(),
        generation(SECOND_GENERATION),
        generation(FIRST_GENERATION),
        &generation_1_key(),
        nonce(),
    );
    assert_eq!(sealed, field());
    assert_eq!(&sealed[..24], &nonce(), "the nonce comes first");
}

#[test]
fn the_vector_opens_to_the_previous_key_and_nowhere_else() {
    let (space, two, first) = (
        space(),
        generation(SECOND_GENERATION),
        generation(FIRST_GENERATION),
    );
    let open = |key: &SpaceKey, space, new, previous| {
        membership::open_previous_key(key, space, new, previous, &field())
    };
    let opened = open(&generation_2_key(), space, two, first).expect("it opens");
    assert!(same(&opened, &generation_1_key()));
    // The associated data binds the space and both generation ids.
    assert!(open(&generation_1_key(), space, two, first).is_none());
    assert!(open(&generation_2_key(), SpaceId([0; 16]), two, first).is_none());
    assert!(open(&generation_2_key(), space, two ^ 1, first).is_none());
    assert!(open(&generation_2_key(), space, two, first ^ 1).is_none());
    let mut tampered = field();
    tampered[30] ^= 1;
    let reopened = membership::open_previous_key(&generation_2_key(), space, two, first, &tampered);
    assert!(reopened.is_none());
}

#[test]
fn the_key_chain_body_vector_decodes_and_encodes() {
    let entry = KeyChainEntry {
        generation: generation(SECOND_GENERATION),
        previous: vec![PreviousKey {
            generation: generation(FIRST_GENERATION),
            key: field(),
        }],
        answers: BTreeSet::new(),
    };
    let add = ChangeId(hex(ADD_ID).try_into().unwrap());
    let body = ControlBody::new(BTreeSet::from([add]), Entry::KeyChain(entry));
    assert_eq!(body.encode(), hex(KEY_CHAIN_BODY));
    assert_eq!(ControlBody::decode(&hex(KEY_CHAIN_BODY)).unwrap(), body);
}

#[test]
fn a_sealed_previous_key_takes_a_fresh_nonce_each_time() {
    let seal = || {
        membership::seal_previous_key(
            &generation_2_key(),
            space(),
            generation(SECOND_GENERATION),
            generation(FIRST_GENERATION),
            &generation_1_key(),
        )
    };
    let (one, two) = (seal(), seal());
    assert_ne!(one[..24], two[..24]);
    for sealed in [one, two] {
        let opened = membership::open_previous_key(
            &generation_2_key(),
            space(),
            generation(SECOND_GENERATION),
            generation(FIRST_GENERATION),
            &sealed,
        );
        assert!(same(&opened.expect("it opens"), &generation_1_key()));
    }
}

#[test]
fn a_merge_rotation_encrypts_every_heads_key_under_the_new_generation() {
    let first = generation(FIRST_GENERATION);
    let other = first + 1;
    let rotation = MergeRotation {
        space: space(),
        generation: membership::generation_id(space(), creator().id(), &[first, other]),
        previous: vec![first, other],
        recipients: BTreeSet::from([creator().id()]),
    };
    let keys = BTreeMap::from([
        (first, generation_1_key()),
        (other, SpaceKey::from_bytes([0x43; 32])),
    ]);
    let new_key = SpaceKey::from_bytes([0x44; 32]);
    let entry = rotation
        .entry(&new_key, |g| keys.get(&g))
        .expect("every head's key");
    assert_eq!(entry.generation, rotation.generation);
    assert!(entry.answers.is_empty());
    assert_eq!(entry.previous.len(), 2);
    for previous in &entry.previous {
        let opened = membership::open_previous_key(
            &new_key,
            space(),
            rotation.generation,
            previous.generation,
            &previous.key,
        );
        assert!(same(
            &opened.expect("it opens"),
            &keys[&previous.generation]
        ));
    }
    // Without the key of a head there is no merge rotation to write.
    let only_first = |g| (g == first).then(|| &keys[&first]);
    assert!(rotation.entry(&new_key, only_first).is_none());
}
