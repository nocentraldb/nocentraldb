//! A rich-text Yjs client id names its author (RFC 0013, "Client ids name
//! their author"): the low 53 bits of SHA-256 over a fixed label, the
//! member id and an 8-byte nonce, read as a big-endian number.
#![cfg(ncdb_rich_text)]

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::ids::MemberId;
use ncdb_core::rich_text::client_id;

const MEMBER: MemberId = MemberId([
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
    26, 27, 28, 29, 30, 31,
]);
const NONCE: [u8; 8] = [7, 6, 5, 4, 3, 2, 1, 0];

/// Computed outside the core, with Python's hashlib:
/// `int.from_bytes(sha256(b"nocentraldb/yjs-client-id" + member + nonce)
/// .digest()[24:], "big") & (2**53 - 1)`.
#[test]
fn a_client_id_is_the_low_53_bits_of_the_hash_of_member_and_nonce() {
    assert_eq!(client_id(&MEMBER, &NONCE), 2_553_909_962_938_370);
}

#[test]
fn a_client_id_fits_a_javascript_number() {
    for nonce in 0u64..256 {
        assert!(client_id(&MEMBER, &nonce.to_be_bytes()) < 1 << 53);
    }
}

#[test]
fn another_member_or_nonce_gives_another_client_id() {
    let other = MemberId([9; 32]);
    assert_ne!(client_id(&MEMBER, &NONCE), client_id(&other, &NONCE));
    assert_ne!(client_id(&MEMBER, &NONCE), client_id(&MEMBER, &[0; 8]));
}
