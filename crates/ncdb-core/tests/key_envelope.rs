//! Sealing and opening a key envelope, as RFC 0003 defines it ("Key
//! envelope"): the member an envelope is for opens it, and no one else; a
//! changed space, generation or recipient, or a truncated envelope, does
//! not open.

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::cbor::{Key, Value};
use ncdb_core::change::Payload;
use ncdb_core::clock::Hlc;
use ncdb_core::envelope::{self, EnvelopeError, KeyEnvelope};
use ncdb_core::ids::SpaceId;
use ncdb_core::keys::{MemberKey, SpaceKey};
use ncdb_core::seal;

const SPACE: SpaceId = SpaceId([7; 16]);
const GENERATION: u64 = (1 << 48) | 0x0102_0304_0506;

fn member(seed: u8) -> MemberKey {
    MemberKey::from_secrets([seed; 32], [seed ^ 0xff; 32])
}

fn space_key() -> SpaceKey {
    SpaceKey::from_bytes([0x5a; 32])
}

fn envelope_for(recipient: &MemberKey) -> KeyEnvelope {
    envelope::seal(&space_key(), SPACE, GENERATION, &recipient.public())
}

/// Two space keys are the same key when a change one seals opens with the
/// other; a space key does not show its bytes.
fn same_key(a: &SpaceKey, b: &SpaceKey) -> bool {
    let author = member(0x33);
    let payload = Payload::new(Hlc(1 << 16), Default::default(), Vec::new());
    let change = seal::seal(&author, a, SPACE, 1, GENERATION, &payload);
    seal::open(&change, &author.public(), b).is_ok()
}

#[test]
fn the_recipient_opens_the_space_key() {
    let alice = member(1);
    let opened = envelope::open(&envelope_for(&alice), SPACE, &alice).unwrap();
    assert!(same_key(&opened, &space_key()));
}

#[test]
fn the_envelope_names_its_recipient_and_generation() {
    let alice = member(1);
    let sealed = envelope_for(&alice);
    assert_eq!(sealed.recipient, alice.id());
    assert_eq!(sealed.generation, GENERATION);
    assert_eq!(sealed.encapsulated_key.len(), envelope::ENCAPSULATED_KEY_LEN);
    assert_eq!(sealed.ciphertext.len(), envelope::CIPHERTEXT_LEN);
}

#[test]
fn each_seal_draws_a_new_ephemeral_key() {
    let alice = member(1);
    let first = envelope_for(&alice);
    let second = envelope_for(&alice);
    assert_ne!(first.encapsulated_key, second.encapsulated_key);
    assert_ne!(first.ciphertext, second.ciphertext);
}

#[test]
fn another_member_cannot_open_it() {
    let sealed = envelope_for(&member(1));
    assert_eq!(
        envelope::open(&sealed, SPACE, &member(2)).err(),
        Some(EnvelopeError::WrongRecipient)
    );
}

#[test]
fn another_member_cannot_open_it_by_naming_itself() {
    let bob = member(2);
    let mut sealed = envelope_for(&member(1));
    sealed.recipient = bob.id();
    assert_eq!(
        envelope::open(&sealed, SPACE, &bob).err(),
        Some(EnvelopeError::Open)
    );
}

#[test]
fn a_changed_space_does_not_open() {
    let alice = member(1);
    let sealed = envelope_for(&alice);
    assert_eq!(
        envelope::open(&sealed, SpaceId([8; 16]), &alice).err(),
        Some(EnvelopeError::Open)
    );
}

#[test]
fn a_changed_generation_does_not_open() {
    let alice = member(1);
    let mut sealed = envelope_for(&alice);
    sealed.generation += 1;
    assert_eq!(
        envelope::open(&sealed, SPACE, &alice).err(),
        Some(EnvelopeError::Open)
    );
}

#[test]
fn a_flipped_bit_does_not_open() {
    let alice = member(1);
    let sealed = envelope_for(&alice);
    for index in 0..sealed.ciphertext.len() {
        let mut flipped = sealed.clone();
        flipped.ciphertext[index] ^= 1;
        assert_eq!(
            envelope::open(&flipped, SPACE, &alice).err(),
            Some(EnvelopeError::Open),
            "ciphertext byte {index}"
        );
    }
    for index in 0..sealed.encapsulated_key.len() {
        let mut flipped = sealed.clone();
        flipped.encapsulated_key[index] ^= 1;
        assert_eq!(
            envelope::open(&flipped, SPACE, &alice).err(),
            Some(EnvelopeError::Open),
            "encapsulated key byte {index}"
        );
    }
}

#[test]
fn a_truncated_ciphertext_does_not_open() {
    let alice = member(1);
    let sealed = envelope_for(&alice);
    for length in 0..sealed.ciphertext.len() {
        let mut truncated = sealed.clone();
        truncated.ciphertext.truncate(length);
        assert_eq!(
            envelope::open(&truncated, SPACE, &alice).err(),
            Some(EnvelopeError::Open),
            "ciphertext of {length} bytes"
        );
    }
}

#[test]
fn a_truncated_encapsulated_key_does_not_open() {
    let alice = member(1);
    let sealed = envelope_for(&alice);
    for length in 0..sealed.encapsulated_key.len() {
        let mut truncated = sealed.clone();
        truncated.encapsulated_key.truncate(length);
        assert_eq!(
            envelope::open(&truncated, SPACE, &alice).err(),
            Some(EnvelopeError::Open),
            "encapsulated key of {length} bytes"
        );
    }
}

#[test]
fn the_fields_read_back() {
    let sealed = envelope_for(&member(1));
    assert_eq!(KeyEnvelope::from_fields(&sealed.fields()), Ok(sealed));
}

#[test]
fn the_fields_are_keys_8_to_11() {
    let fields = envelope_for(&member(1)).fields();
    let keys: Vec<&Key> = fields.keys().collect();
    assert_eq!(
        keys,
        [8, 9, 10, 11].map(Key::Unsigned).iter().collect::<Vec<_>>()
    );
}

#[test]
fn other_keys_of_the_control_body_are_left_alone() {
    let sealed = envelope_for(&member(1));
    let mut body = sealed.fields();
    body.insert(Key::Unsigned(1), Value::Unsigned(2));
    body.insert(Key::Unsigned(3), Value::Array(Vec::new()));
    body.insert(Key::Unsigned(64), Value::Text("may ignore".into()));
    assert_eq!(KeyEnvelope::from_fields(&body), Ok(sealed));
}

#[test]
fn a_missing_or_mistyped_field_is_malformed() {
    let fields = envelope_for(&member(1)).fields();
    for key in 8..=11 {
        let mut missing = fields.clone();
        missing.remove(&Key::Unsigned(key));
        assert_eq!(
            KeyEnvelope::from_fields(&missing),
            Err(EnvelopeError::Malformed(key)),
            "key {key} missing"
        );
        let mut mistyped = fields.clone();
        mistyped.insert(Key::Unsigned(key), Value::Text("x".into()));
        assert_eq!(
            KeyEnvelope::from_fields(&mistyped),
            Err(EnvelopeError::Malformed(key)),
            "key {key} mistyped"
        );
    }
}

#[test]
fn a_recipient_that_is_not_32_bytes_is_malformed() {
    let mut fields = envelope_for(&member(1)).fields();
    fields.insert(Key::Unsigned(8), Value::Bytes(vec![0; 31]));
    assert_eq!(
        KeyEnvelope::from_fields(&fields),
        Err(EnvelopeError::Malformed(8))
    );
}

#[test]
fn errors_and_envelopes_show_no_key() {
    let alice = member(1);
    let sealed = envelope_for(&alice);
    let opened = envelope::open(&sealed, SPACE, &alice).unwrap();
    assert_eq!(format!("{opened:?}"), "SpaceKey(<redacted>)");
    for error in [
        EnvelopeError::WrongRecipient,
        EnvelopeError::Malformed(8),
        EnvelopeError::Open,
    ] {
        let shown = format!("{error} {error:?}");
        assert!(!shown.contains("5a5a"), "{shown}");
    }
}
