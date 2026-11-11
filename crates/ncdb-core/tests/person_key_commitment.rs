//! The commitment to a person key (RFC 0003, "The person key"):
//! `SHA-256("nocentraldb/person-key-commit" || space id || person-key id ||
//! key)`, which every person-key entry carries (RFC 0008, "Person-key
//! entry"). The vector was computed with Python's `hashlib`:
//!
//! ```text
//! hashlib.sha256(b"nocentraldb/person-key-commit" + bytes([1])*16
//!                + bytes([2])*16 + bytes([3])*32).hexdigest()
//! ```

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::ids::SpaceId;
use ncdb_core::membership::person_key_commitment;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn the_commitment_matches_the_independent_vector() {
    let commitment = person_key_commitment(SpaceId([1; 16]), [2; 16], &[3; 32]);
    assert_eq!(
        hex(&commitment),
        "a07b74cee02843d538136e5ec934516b5ce09bb4362c8caa9f1f822304957b29"
    );
}

#[test]
fn the_commitment_binds_the_space_the_person_key_id_and_the_key() {
    let base = person_key_commitment(SpaceId([1; 16]), [2; 16], &[3; 32]);
    assert_ne!(
        person_key_commitment(SpaceId([9; 16]), [2; 16], &[3; 32]),
        base
    );
    assert_ne!(
        person_key_commitment(SpaceId([1; 16]), [9; 16], &[3; 32]),
        base
    );
    assert_ne!(
        person_key_commitment(SpaceId([1; 16]), [2; 16], &[9; 32]),
        base
    );
}
