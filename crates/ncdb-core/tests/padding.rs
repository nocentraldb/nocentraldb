//! Padding before encryption (RFC 0002, "Padding"): the canonical
//! encoding of every sealed payload map, padding (key 5) included, is a
//! multiple of 256 bytes. The padding is a byte string of zeros; it costs
//! at least two bytes (its key and its length), so a payload one byte
//! short of a multiple, or 26 bytes short (where the length prefix grows
//! from one byte to two), pads to the multiple after.

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::BTreeSet;

use ncdb_core::cbor::Value;
use ncdb_core::change::{Operation, Payload};
use ncdb_core::clock::Hlc;
use ncdb_core::ids::{ObjectId, SpaceId};
use ncdb_core::keys::{MemberKey, SpaceKey};
use ncdb_core::seal;

/// A payload whose canonical encoding is exactly `length` bytes, if one of
/// this shape exists: one operation, its value a byte string, its
/// collection name one to three bytes long.
fn payload_of_length(length: usize) -> Option<Payload> {
    (1..=3).find_map(|collection| {
        (0..length).find_map(|value| {
            let operation = Operation::new(
                ObjectId([1; 16]),
                "c".repeat(collection),
                Some("f".to_owned()),
                1,
                Some(Value::Bytes(vec![0xab; value])),
            );
            let payload = Payload::new(Hlc(1), BTreeSet::new(), vec![operation]);
            (payload.encode().len() == length).then_some(payload)
        })
    })
}

/// The length a payload of `length` bytes pads to.
fn expected(length: usize) -> usize {
    let mut target = length.div_ceil(256) * 256;
    loop {
        let missing = target - length;
        // The padding entry costs 1 (key) + 1 (length) + n for n < 24, and
        // 1 + 2 + n for 24 <= n < 256, and 1 + 3 + n up to 65535.
        let reachable = missing == 0
            || (2..=25).contains(&missing)
            || (27..=258).contains(&missing)
            || (260..=65_539).contains(&missing);
        if reachable {
            return target;
        }
        target += 256;
    }
}

#[test]
fn the_named_lengths_pad_as_expected() {
    for (length, padded) in [(254, 256), (255, 512), (256, 256), (257, 512), (512, 512)] {
        assert_eq!(expected(length), padded, "{length}");
        let payload = payload_of_length(length).unwrap();
        assert_eq!(seal::pad(&payload).encode().len(), padded, "{length}");
    }
}

#[test]
fn every_length_pads_to_the_next_reachable_multiple_of_256() {
    let mut covered = 0;
    for length in 20..=1100 {
        let Some(payload) = payload_of_length(length) else {
            continue;
        };
        covered += 1;
        let padded = seal::pad(&payload);
        assert_eq!(padded.encode().len(), expected(length), "{length}");
        let padding = padded.padding.clone().unwrap_or_default();
        assert!(padding.iter().all(|byte| *byte == 0), "{length}");
        assert_eq!(padded.padding.is_none(), length % 256 == 0, "{length}");
    }
    assert!(covered > 1000, "only {covered} lengths covered");
}

#[test]
fn padding_replaces_padding_the_payload_already_has() {
    let mut payload = payload_of_length(100).unwrap();
    payload.padding = Some(vec![0; 1000]);
    assert_eq!(seal::pad(&payload).encode().len(), 256);
}

#[test]
fn padded_payloads_seal_to_their_padded_length_and_open_unchanged() {
    let (author, key) = (
        MemberKey::from_secrets([1; 32], [2; 32]),
        SpaceKey::from_bytes([3; 32]),
    );
    for length in [254, 255, 256, 257, 512] {
        let payload = payload_of_length(length).unwrap();
        let change = seal::seal(&author, &key, SpaceId([4; 16]), 1, 1, &payload);
        assert_eq!(
            change.header().payload.len(),
            seal::NONCE_LEN + expected(length) + seal::TAG_LEN,
            "{length}"
        );
        let mut opened = seal::open(&change, &author.public(), &key).unwrap();
        opened.padding = None;
        assert_eq!(opened, payload, "{length}");
    }
}
