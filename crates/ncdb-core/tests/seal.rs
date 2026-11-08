//! Signing and sealing a change, as RFC 0003 defines it ("How a member
//! signs and encrypts a change"): a change member A signs verifies for
//! member B, and a change with any bit flipped does not.

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::BTreeSet;

use ncdb_core::cbor::{self, Key, Value};
use ncdb_core::change::{Change, Header, Kind, Operation, Payload};
use ncdb_core::clock::Hlc;
use ncdb_core::ids::{ChangeId, ObjectId, SpaceId};
use ncdb_core::keys::{MemberKey, MemberPublicKey, SpaceKey};
use ncdb_core::seal::{self, OpenError};

const SPACE: SpaceId = SpaceId([7; 16]);

fn member(seed: u8) -> MemberKey {
    MemberKey::from_secrets([seed; 32], [seed ^ 0xff; 32])
}

fn control_header(author: &MemberKey) -> Header {
    Header::new(
        SPACE,
        author.id(),
        1,
        1,
        Kind::CONTROL,
        vec![0xa1, 0x01, 0x01],
        seal::SUITE,
    )
}

/// Whether member B, holding A's public keys, accepts these frame bytes.
fn verifies(frame: &[u8], author: &MemberPublicKey) -> bool {
    Change::decode(frame).is_ok_and(|change| seal::verify(&change, author).is_ok())
}

#[test]
fn a_change_signed_by_a_verifies_for_b() {
    let a = member(1);
    let change = seal::sign(&a, control_header(&a));
    // Member B receives only the bytes and knows only A's public keys.
    let received = Change::decode(change.bytes()).unwrap();
    assert_eq!(seal::verify(&received, &a.public()), Ok(()));
    assert_eq!(received.signature().len(), 64);
}

#[test]
fn a_flipped_bit_in_header_or_signature_is_rejected() {
    let a = member(1);
    let change = seal::sign(&a, control_header(&a));
    let frame = change.bytes();
    let signature_start = frame.len() - 64;
    let header_start = frame
        .windows(change.header_bytes().len())
        .position(|window| window == change.header_bytes())
        .unwrap();
    assert!(verifies(frame, &a.public()));
    // One bit per byte, a different bit in each of eight neighbours, so
    // every byte and every bit position is covered.
    for byte in 0..frame.len() {
        let bit = byte % 8;
        let mut tampered = frame.to_vec();
        tampered[byte] ^= 1 << bit;
        assert!(
            !verifies(&tampered, &a.public()),
            "bit {bit} of byte {byte} flipped (header from {header_start}, signature from {signature_start})"
        );
    }
}

#[test]
fn a_flipped_bit_in_the_signature_names_the_signature() {
    let a = member(1);
    let change = seal::sign(&a, control_header(&a));
    let mut signature = change.signature().to_vec();
    signature[10] ^= 4;
    let tampered = Change::new(change.header().clone(), signature);
    assert_eq!(
        seal::verify(&tampered, &a.public()),
        Err(OpenError::Signature)
    );
}

#[test]
fn a_signature_of_the_wrong_length_is_rejected() {
    let a = member(1);
    let change = seal::sign(&a, control_header(&a));
    for length in [0, 63, 65] {
        let tampered = Change::new(change.header().clone(), vec![0; length]);
        assert_eq!(
            seal::verify(&tampered, &a.public()),
            Err(OpenError::Signature)
        );
    }
}

#[test]
fn another_members_keys_do_not_verify_the_change() {
    let (a, b) = (member(1), member(2));
    let change = seal::sign(&a, control_header(&a));
    assert_eq!(
        seal::verify(&change, &b.public()),
        Err(OpenError::AuthorMismatch)
    );
}

#[test]
fn b_cannot_sign_as_a() {
    let (a, b) = (member(1), member(2));
    // B signs a header that names A as its author.
    let forged = seal::sign(&b, control_header(&a));
    assert_eq!(
        seal::verify(&forged, &a.public()),
        Err(OpenError::Signature)
    );
}

#[test]
fn an_invalid_author_key_names_the_key() {
    // The first 32-byte string with a y coordinate that is not on the
    // curve: no Ed25519 public key has this encoding.
    let invalid = (2u8..)
        .map(|y| {
            let mut bytes = [0; 32];
            bytes[0] = y;
            bytes
        })
        .find(|bytes| ed25519_dalek::VerifyingKey::from_bytes(bytes).is_err())
        .unwrap();
    let author = MemberPublicKey {
        signing: invalid,
        agreement: [9; 32],
    };
    let header = Header::new(SPACE, author.id(), 1, 1, Kind::CONTROL, vec![], seal::SUITE);
    let change = Change::new(header, vec![0; 64]);
    assert_eq!(seal::verify(&change, &author), Err(OpenError::AuthorKey));
}

#[test]
fn verification_is_strict() {
    // A small-order public key (the identity) with R the identity and S
    // zero: plain Ed25519 verification accepts it for every message,
    // verify_strict rejects it. Members must accept exactly the same
    // signatures, or their copies diverge (RFC 0003, "Primitives").
    let mut identity = [0; 32];
    identity[0] = 1;
    let author = MemberPublicKey {
        signing: identity,
        agreement: [9; 32],
    };
    let header = Header::new(SPACE, author.id(), 1, 1, Kind::CONTROL, vec![], seal::SUITE);
    let mut signature = vec![0; 64];
    signature[0] = 1;
    let change = Change::new(header, signature.clone());

    use ed25519_dalek::Verifier;
    let key = ed25519_dalek::VerifyingKey::from_bytes(&identity).unwrap();
    let signature = ed25519_dalek::Signature::from_slice(&signature).unwrap();
    assert!(key.verify(change.header_bytes(), &signature).is_ok());

    assert_eq!(
        seal::verify(&change, &author),
        Err(OpenError::Signature)
    );
}

#[test]
fn errors_name_the_step_that_failed() {
    let texts = [
        (OpenError::NotSealed(1), "not sealed"),
        (OpenError::UnknownSuite(9), "suite 9"),
        (OpenError::AuthorMismatch, "author"),
        (OpenError::AuthorKey, "author's public key"),
        (OpenError::Signature, "signature"),
        (OpenError::Decryption, "decrypt"),
    ];
    for (error, text) in texts {
        let shown = error.to_string();
        assert!(shown.contains(text), "{shown:?} should contain {text:?}");
    }
}

// --- Sealing: encrypt the payload, then sign ---------------------------

const SECRET: &str = "SECRET-MARKER-IN-THE-PAYLOAD";

fn payload() -> Payload {
    let operation = Operation::new(
        ObjectId([0x5e; 16]),
        "notes".to_owned(),
        Some("title".to_owned()),
        1,
        Some(Value::Text(SECRET.to_owned())),
    );
    Payload::new(
        Hlc(1_793_000_000_000 << 16),
        BTreeSet::from([ChangeId([3; 32])]),
        vec![operation],
    )
}

/// The payload as the author wrote it, whatever padding sealing added.
fn unpadded(mut payload: Payload) -> Payload {
    payload.padding = None;
    payload
}

fn sealed(author: &MemberKey, key: &SpaceKey) -> Change {
    seal::seal(author, key, SPACE, 5, 3, &payload())
}

#[test]
fn a_change_sealed_by_a_opens_for_b() {
    let (a, key) = (member(1), SpaceKey::from_bytes([0x11; 32]));
    let change = sealed(&a, &key);
    let received = Change::decode(change.bytes()).unwrap();
    let opened = seal::open(&received, &a.public(), &SpaceKey::from_bytes([0x11; 32])).unwrap();
    assert_eq!(unpadded(opened), payload());
}

#[test]
fn the_header_carries_only_what_the_relay_may_see() {
    let (a, key) = (member(1), SpaceKey::from_bytes([0x11; 32]));
    let change = sealed(&a, &key);
    let header = change.header();
    assert_eq!(header.space, SPACE);
    assert_eq!(header.author, a.id());
    assert_eq!(header.sequence, 5);
    assert_eq!(header.key_generation, 3);
    assert_eq!(header.kind, Kind::SEALED);
    assert_eq!(header.format, Header::FORMAT);
    assert_eq!(header.suite, seal::SUITE);
    assert_eq!(header.extensions().iter().count(), 0);
    // The header map holds exactly keys 1 to 8 (RFC 0002, "A change at a
    // glance"); everything else is inside the ciphertext.
    let Value::Map(map) = cbor::decode(change.header_bytes()).unwrap() else {
        panic!("the header is a map");
    };
    let keys: Vec<Key> = map.keys().cloned().collect();
    assert_eq!(keys, (1..=8).map(Key::Unsigned).collect::<Vec<_>>());
    // Nothing of the payload shows in the frame.
    let frame = change.bytes();
    assert!(!frame.windows(SECRET.len()).any(|w| w == SECRET.as_bytes()));
    assert!(!frame.windows(5).any(|w| w == b"notes"));
    assert!(!frame.windows(5).any(|w| w == b"title"));
    assert!(!frame.windows(16).any(|w| w == [0x5e; 16]));
}

#[test]
fn the_sealed_payload_is_nonce_ciphertext_and_tag() {
    let (a, key) = (member(1), SpaceKey::from_bytes([0x11; 32]));
    let change = sealed(&a, &key);
    let padded = seal::pad(&payload()).encode().len();
    assert_eq!(
        change.header().payload.len(),
        seal::NONCE_LEN + padded + seal::TAG_LEN
    );
}

#[test]
fn every_seal_takes_a_fresh_nonce() {
    let (a, key) = (member(1), SpaceKey::from_bytes([0x11; 32]));
    let (first, second) = (sealed(&a, &key), sealed(&a, &key));
    assert_ne!(
        first.header().payload[..seal::NONCE_LEN],
        second.header().payload[..seal::NONCE_LEN]
    );
    assert_ne!(first.id(), second.id());
}

#[test]
fn a_member_without_the_space_key_cannot_read_the_payload() {
    let (a, key) = (member(1), SpaceKey::from_bytes([0x11; 32]));
    let change = sealed(&a, &key);
    // The signature still verifies: the relay checks it without a key.
    assert_eq!(seal::verify(&change, &a.public()), Ok(()));
    for other in [[0x12; 32], [0; 32]] {
        assert_eq!(
            seal::open(&change, &a.public(), &SpaceKey::from_bytes(other)),
            Err(OpenError::Decryption)
        );
    }
}

#[test]
fn a_flipped_bit_anywhere_in_a_sealed_change_is_rejected() {
    let (a, key) = (member(1), SpaceKey::from_bytes([0x11; 32]));
    let frame = sealed(&a, &key).bytes().to_vec();
    for byte in 0..frame.len() {
        let mut tampered = frame.clone();
        tampered[byte] ^= 1 << (byte % 8);
        let opened = Change::decode(&tampered)
            .ok()
            .map(|change| seal::open(&change, &a.public(), &key));
        assert!(
            !matches!(opened, Some(Ok(_))),
            "byte {byte} flipped, and the change still opened"
        );
    }
}

#[test]
fn a_ciphertext_re_signed_after_a_flipped_bit_does_not_decrypt() {
    let (a, key) = (member(1), SpaceKey::from_bytes([0x11; 32]));
    let change = sealed(&a, &key);
    let length = change.header().payload.len();
    // The nonce, the ciphertext and the tag.
    for byte in [0, seal::NONCE_LEN, length / 2, length - 1] {
        let mut header = change.header().clone();
        header.payload[byte] ^= 0x20;
        let unsigned = Change::new(header.clone(), change.signature().to_vec());
        assert_eq!(
            seal::open(&unsigned, &a.public(), &key),
            Err(OpenError::Signature)
        );
        let re_signed = seal::sign(&a, header);
        assert_eq!(
            seal::open(&re_signed, &a.public(), &key),
            Err(OpenError::Decryption),
            "byte {byte}"
        );
    }
    let mut truncated = change.header().clone();
    truncated.payload.truncate(seal::NONCE_LEN + seal::TAG_LEN - 1);
    assert_eq!(
        seal::open(&seal::sign(&a, truncated), &a.public(), &key),
        Err(OpenError::Decryption)
    );
}

/// Each header field except the payload, changed: the original signature
/// no longer verifies, and a re-signed header no longer decrypts, because
/// the field is part of the associated data. A different author re-signs
/// with its own key; the kind and the suite stop at their own checks.
#[test]
fn every_header_field_is_bound_to_the_payload() {
    let (a, b, key) = (member(1), member(2), SpaceKey::from_bytes([0x11; 32]));
    let change = sealed(&a, &key);
    type Edit = fn(&mut Header, &MemberKey);
    let edits: [(&str, Edit, OpenError); 8] = [
        ("space", |h, _| h.space.0[0] ^= 1, OpenError::Decryption),
        ("author", |h, b| h.author = b.id(), OpenError::Decryption),
        ("sequence", |h, _| h.sequence += 1, OpenError::Decryption),
        ("key generation", |h, _| h.key_generation += 1, OpenError::Decryption),
        ("kind", |h, _| h.kind = Kind(2), OpenError::NotSealed(2)),
        ("header format", |h, _| h.format = 2, OpenError::Decryption),
        ("suite", |h, _| h.suite = 2, OpenError::UnknownSuite(2)),
        (
            "extension",
            |h, _| assert!(h.insert_extension(64, Value::Unsigned(1))),
            OpenError::Decryption,
        ),
    ];
    for (field, edit, after_re_signing) in edits {
        let mut header = change.header().clone();
        edit(&mut header, &b);
        let signer = if header.author == b.id() { &b } else { &a };
        let unsigned = Change::new(header.clone(), change.signature().to_vec());
        assert!(
            matches!(
                seal::open(&unsigned, &signer.public(), &key),
                Err(OpenError::Signature)
            ),
            "{field}: the old signature must not verify"
        );
        let re_signed = seal::sign(signer, header);
        assert_eq!(
            seal::open(&re_signed, &signer.public(), &key),
            Err(after_re_signing),
            "{field}"
        );
    }
}

#[test]
fn a_control_change_does_not_open() {
    let (a, key) = (member(1), SpaceKey::from_bytes([0x11; 32]));
    let change = seal::sign(&a, control_header(&a));
    assert_eq!(
        seal::open(&change, &a.public(), &key),
        Err(OpenError::NotSealed(1))
    );
}

/// The content key and the associated data as the module documents them,
/// computed here with the crates directly.
fn seal_raw(author: &MemberKey, space_key: [u8; 32], plaintext: &[u8]) -> Change {
    use chacha20poly1305::aead::{Aead, KeyInit, Payload as AeadPayload};
    use chacha20poly1305::XChaCha20Poly1305;
    let (sequence, generation) = (1u64, 4u64);
    let mut info = b"nocentraldb/content-key".to_vec();
    info.extend_from_slice(&SPACE.0);
    info.extend_from_slice(&generation.to_be_bytes());
    info.extend_from_slice(&seal::SUITE.to_be_bytes());
    let mut content_key = [0; 32];
    hkdf::Hkdf::<sha2::Sha256>::new(None, &space_key)
        .expand(&info, &mut content_key)
        .unwrap();
    let associated = cbor::encode(&Value::Map(
        [
            (1, Value::Bytes(SPACE.0.to_vec())),
            (2, Value::Bytes(author.id().0.to_vec())),
            (3, Value::Unsigned(sequence)),
            (4, Value::Unsigned(generation)),
            (5, Value::Unsigned(0)),
            (7, Value::Unsigned(1)),
            (8, Value::Unsigned(seal::SUITE)),
        ]
        .into_iter()
        .map(|(key, value)| (Key::Unsigned(key), value))
        .collect(),
    ));
    let nonce = [0x24; 24];
    let cipher = XChaCha20Poly1305::new(&content_key.into());
    let ciphertext = cipher
        .encrypt(
            &nonce.into(),
            AeadPayload {
                msg: plaintext,
                aad: &associated,
            },
        )
        .unwrap();
    let sealed = [nonce.as_slice(), &ciphertext].concat();
    let header = Header::new(SPACE, author.id(), sequence, generation, Kind::SEALED, sealed, seal::SUITE);
    seal::sign(author, header)
}

#[test]
fn the_content_key_and_associated_data_are_the_documented_ones() {
    let a = member(1);
    let plaintext = seal::pad(&payload()).encode();
    let change = seal_raw(&a, [0x11; 32], &plaintext);
    let opened = seal::open(&change, &a.public(), &SpaceKey::from_bytes([0x11; 32])).unwrap();
    assert_eq!(unpadded(opened), payload());
}

#[test]
fn a_plaintext_that_is_no_payload_names_the_payload() {
    let a = member(1);
    let change = seal_raw(&a, [0x11; 32], &[0x01]);
    assert!(matches!(
        seal::open(&change, &a.public(), &SpaceKey::from_bytes([0x11; 32])),
        Err(OpenError::Payload(_))
    ));
}
