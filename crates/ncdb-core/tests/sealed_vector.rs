//! The vector of a signed, sealed change, written by tests/vectors/
//! generate.py with Python's cryptography (HKDF, Ed25519) and PyNaCl
//! (XChaCha20-Poly1305), independently of this crate: sealing the same
//! payload with the same keys and nonce gives the same bytes, and the
//! vector opens to that payload.

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::change::{Change, Payload};
use ncdb_core::ids::{ChangeId, SpaceId};
use ncdb_core::keys::{MemberKey, SpaceKey};
use ncdb_core::seal;

const FRAME: &str = include_str!("vectors/signed-sealed-change.frame.hex");
const ID: &str = include_str!("vectors/signed-sealed-change.id.hex");
const PLAINTEXT: &str = include_str!("vectors/signed-sealed-change.plaintext.hex");
const PUBLIC: &str = include_str!("vectors/signed-sealed-change.public.hex");
// The payload of RFC 0002's vectors, which this change seals.
const PAYLOAD: &str = include_str!("vectors/payload.hex");

const SPACE: SpaceId = SpaceId([
    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
]);
const SEQUENCE: u64 = 7;
const GENERATION: u64 = 2;

fn hex(text: &str) -> Vec<u8> {
    let text: String = text.split_whitespace().collect();
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn member() -> MemberKey {
    MemberKey::from_secrets(
        std::array::from_fn(|i| 0x10 + i as u8),
        std::array::from_fn(|i| 0x60 + i as u8),
    )
}

fn space_key() -> SpaceKey {
    SpaceKey::from_bytes(std::array::from_fn(|i| 0xc0 + i as u8))
}

fn unpadded() -> Payload {
    let mut payload = Payload::decode(&hex(PAYLOAD)).unwrap();
    payload.padding = None;
    payload
}

#[test]
fn the_member_has_the_vectors_public_keys() {
    let public = member().public();
    assert_eq!(
        to_hex(&[public.signing, public.agreement].concat()),
        to_hex(&hex(PUBLIC))
    );
}

#[test]
fn sealing_gives_the_vectors_bytes() {
    let nonce = std::array::from_fn(|i| 0x90 + i as u8);
    let change = seal::seal_with_nonce(
        &member(),
        &space_key(),
        SPACE,
        SEQUENCE,
        GENERATION,
        &unpadded(),
        nonce,
    );
    assert_eq!(to_hex(change.bytes()), to_hex(&hex(FRAME)));
    assert_eq!(change.id(), ChangeId(hex(ID).try_into().unwrap()));
    assert_eq!(
        to_hex(&seal::pad(&unpadded()).encode()),
        to_hex(&hex(PLAINTEXT))
    );
}

#[test]
fn the_vector_opens_to_the_payload() {
    let change = Change::decode(&hex(FRAME)).unwrap();
    assert_eq!(seal::verify(&change, &member().public()), Ok(()));
    let opened = seal::open(&change, &member().public(), &space_key()).unwrap();
    assert_eq!(to_hex(&opened.encode()), to_hex(&hex(PLAINTEXT)));
    let mut opened = opened;
    opened.padding = None;
    assert_eq!(opened, unpadded());
}
