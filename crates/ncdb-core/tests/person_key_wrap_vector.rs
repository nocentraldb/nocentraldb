//! A person key wrapped under a generation, as the owner decided on NOC-209
//! (Carsten, 2026-11-11) and RFC 0003 gives it ("Person-key record"): nonce
//! (24) ‖ ciphertext (32) ‖ tag (16), 72 bytes, XChaCha20-Poly1305 with a
//! random nonce under the person-key subkey of the generation's space key,
//! with the space id, the generation id and the person-key id as associated
//! data, against the vector tests/vectors/membership/generate.py writes with
//! cryptography and PyNaCl, independently of this crate.

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::ids::SpaceId;
use ncdb_core::keys::{PersonKey, SpaceKey};
use ncdb_core::membership::{
    self, WRAPPED_PERSON_KEY_LEN, person_key_commitment, unwrap_person_key, wrap_person_key,
    wrap_person_key_with_nonce,
};

const SPACE_ID: &str = include_str!("vectors/membership/space-id.hex");
const SECOND_GENERATION: &str = include_str!("vectors/membership/second-generation.hex");
const WRAPPED: &str = include_str!("vectors/membership/person-key-wrapped.hex");

const PERSON_KEY_ID: [u8; 16] = [
    0x61, 0x62, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69, 0x6a, 0x6b, 0x6c, 0x6d, 0x6e, 0x6f, 0x70,
];

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

fn generation() -> u64 {
    u64::from_be_bytes(hex(SECOND_GENERATION).try_into().unwrap())
}

fn generation_2_key() -> SpaceKey {
    SpaceKey::from_bytes([0x42; 32])
}

fn person_key() -> PersonKey {
    PersonKey::from_bytes([0x55; 32])
}

fn nonce() -> [u8; 24] {
    core::array::from_fn(|i| 0xa1 + i as u8)
}

fn wrapped() -> [u8; WRAPPED_PERSON_KEY_LEN] {
    hex(WRAPPED).try_into().expect("72 bytes")
}

/// Whether two person keys are the same key: they commit to the same bytes.
/// `PersonKey` has no equality of its own, as a secret.
fn same(a: &PersonKey, b: &PersonKey) -> bool {
    a.commitment(SpaceId([9; 16]), [7; 16]) == b.commitment(SpaceId([9; 16]), [7; 16])
}

#[test]
fn a_person_key_wraps_to_the_vectors_bytes() {
    let sealed = wrap_person_key_with_nonce(
        &generation_2_key(),
        space(),
        generation(),
        PERSON_KEY_ID,
        &person_key(),
        nonce(),
    );
    assert_eq!(sealed, wrapped());
    assert_eq!(&sealed[..24], &nonce(), "the nonce comes first");
}

#[test]
fn the_vector_unwraps_to_the_person_key_and_its_commitment() {
    let unwrapped = unwrap_person_key(
        &generation_2_key(),
        space(),
        generation(),
        PERSON_KEY_ID,
        &wrapped(),
    )
    .expect("it unwraps");
    assert!(same(&unwrapped, &person_key()));
    assert_eq!(
        unwrapped.commitment(space(), PERSON_KEY_ID),
        person_key_commitment(space(), PERSON_KEY_ID, &[0x55; 32]),
    );
}

#[test]
fn the_wrap_is_bound_to_its_key_space_generation_and_person_key_id() {
    let unwrap = |key: &SpaceKey, space, generation, id, wrapped: &[u8]| {
        unwrap_person_key(key, space, generation, id, wrapped)
    };
    let (space, two) = (space(), generation());
    let key = generation_2_key();
    assert!(
        unwrap(
            &SpaceKey::from_bytes([0x41; 32]),
            space,
            two,
            PERSON_KEY_ID,
            &wrapped()
        )
        .is_none()
    );
    assert!(unwrap(&key, SpaceId([0; 16]), two, PERSON_KEY_ID, &wrapped()).is_none());
    assert!(unwrap(&key, space, two ^ 1, PERSON_KEY_ID, &wrapped()).is_none());
    assert!(unwrap(&key, space, two, [0; 16], &wrapped()).is_none());
    let mut tampered = wrapped();
    tampered[40] ^= 1;
    assert!(unwrap(&key, space, two, PERSON_KEY_ID, &tampered).is_none());
    assert!(unwrap(&key, space, two, PERSON_KEY_ID, &wrapped()[..71]).is_none());
}

#[test]
fn a_wrap_draws_a_fresh_nonce_each_time() {
    let key = generation_2_key();
    let one = wrap_person_key(&key, space(), generation(), PERSON_KEY_ID, &person_key());
    let two = wrap_person_key(&key, space(), generation(), PERSON_KEY_ID, &person_key());
    assert_ne!(one[..24], two[..24]);
    for wrapped in [one, two] {
        let unwrapped =
            unwrap_person_key(&key, space(), generation(), PERSON_KEY_ID, &wrapped).unwrap();
        assert!(same(&unwrapped, &person_key()));
    }
}

#[test]
fn a_person_key_never_prints_its_bytes() {
    let shown = format!("{:?}", PersonKey::from_bytes([0x55; 32]));
    assert!(!shown.contains("55"), "{shown}");
    assert!(!format!("{:?}", PersonKey::generate()).is_empty());
    let _ = membership::WRAPPED_PERSON_KEY_LEN;
}
