//! Signing and sealing a change, as RFC 0003 defines it ("How a member
//! signs and encrypts a change"): a change member A signs verifies for
//! member B, and a change with any bit flipped does not.

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::change::{Change, Header, Kind};
use ncdb_core::ids::SpaceId;
use ncdb_core::keys::{MemberKey, MemberPublicKey};
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
