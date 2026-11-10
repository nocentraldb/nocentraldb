//! How deeply values may nest in a change: at most `cbor::MAX_DEPTH`,
//! counted from the frame. Deeper input is an error, never a stack overflow.
//!
//! The limit is RFC 0002's ("The CBOR profile"), decided by the owner in NOC-24
//! ("How deeply may a CBOR value nest in a change?").

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::BTreeSet;

use ncdb_core::cbor::{self, MAX_DEPTH, Reason, Value};
use ncdb_core::change::{Change, ChangeError, Header, Kind, Operation, Payload};
use ncdb_core::clock::Hlc;
use ncdb_core::ids::{MemberId, ObjectId, SpaceId};

/// The bytes of a value `depth` levels deep: arrays of one item around a 0.
fn nested_bytes(depth: usize) -> Vec<u8> {
    let mut bytes = vec![0x81; depth - 1];
    bytes.push(0x00);
    bytes
}

/// The same as a value; only for depths the encoder's recursion handles.
fn nested(depth: usize) -> Value {
    (1..depth).fold(Value::Unsigned(0), |inner, _| Value::Array(vec![inner]))
}

fn header() -> Header {
    Header::new(
        SpaceId([1; 16]),
        MemberId([2; 32]),
        1,
        0,
        Kind::SEALED,
        vec![],
        1,
    )
}

fn too_deep(error: ChangeError) -> bool {
    matches!(error, ChangeError::Encoding(e) if e.reason == Reason::TooDeep)
}

#[test]
fn the_limit_is_32() {
    assert_eq!(MAX_DEPTH, 32);
}

#[test]
fn a_value_32_deep_decodes_and_33_deep_is_rejected() {
    assert_eq!(cbor::decode(&nested_bytes(32)), Ok(nested(32)));
    let error = cbor::decode(&nested_bytes(33)).unwrap_err();
    assert_eq!(error.reason, Reason::TooDeep);
    assert_eq!(error.offset, 32, "the 33rd level starts at byte 32");
    // Map values count the same way as array items.
    let mut in_map = vec![0xa1, 0x00];
    in_map.extend(nested_bytes(32));
    assert_eq!(cbor::decode(&in_map).unwrap_err().reason, Reason::TooDeep);
}

#[test]
fn a_value_100000_deep_is_an_error_not_a_crash() {
    let error = cbor::decode(&nested_bytes(100_000)).unwrap_err();
    assert_eq!(error.reason, Reason::TooDeep);
}

#[test]
fn the_header_counts_from_the_frame() {
    // Frame 1, header map 2, so an unknown key's value starts at depth 3
    // and may hold 30 levels.
    for (levels, fits) in [(30, true), (31, false)] {
        let mut header = header();
        assert!(header.insert_extension(64, nested(levels)));
        let change = Change::new(header.clone(), vec![]);
        match (Change::decode(change.bytes()), fits) {
            (Ok(decoded), true) => assert_eq!(decoded, change),
            (Err(error), false) => assert!(too_deep(error), "{levels}: {error}"),
            (result, _) => panic!("{levels} levels: {result:?}"),
        }
        let decoded = Header::decode(&header.encode());
        assert_eq!(decoded.is_ok(), fits, "{levels} levels: {decoded:?}");
    }
}

#[test]
fn the_payload_counts_from_the_frame() {
    // Frame 1, header map 2, payload map 3 (inside header key 6): an
    // unknown key's value starts at depth 4 and may hold 29 levels; an
    // operation's value (operations 4, operation 5) starts at 6, 27 levels.
    for (levels, fits) in [(29, true), (30, false)] {
        let mut payload = Payload::new(Hlc(1), BTreeSet::new(), vec![]);
        assert!(payload.insert_extension(64, nested(levels)));
        match Payload::decode(&payload.encode()) {
            Ok(decoded) if fits => assert_eq!(decoded, payload),
            Err(error) if !fits => assert!(too_deep(error), "{levels}: {error}"),
            result => panic!("payload key, {levels} levels: {result:?}"),
        }
    }
    for (levels, fits) in [(27, true), (28, false)] {
        let operation = Operation::new(
            ObjectId([3; 16]),
            "c".into(),
            Some("f".into()),
            1,
            Some(nested(levels)),
        );
        let payload = Payload::new(Hlc(1), BTreeSet::new(), vec![operation]);
        match Payload::decode(&payload.encode()) {
            Ok(decoded) if fits => assert_eq!(decoded, payload),
            Err(error) if !fits => assert!(too_deep(error), "{levels}: {error}"),
            result => panic!("operation value, {levels} levels: {result:?}"),
        }
    }
}

#[test]
fn a_frame_100000_deep_is_an_error_not_a_crash() {
    // A header whose unknown key 64 holds 100,000 levels, built as bytes:
    // the encoder is not meant for values that deep.
    let mut header_bytes = header().encode();
    assert_eq!(header_bytes[0], 0xa8, "a map of eight keys");
    header_bytes[0] = 0xa9;
    header_bytes.extend_from_slice(&[0x18, 0x40]);
    header_bytes.extend(nested_bytes(100_000));
    let mut frame = vec![0x83, 0x01];
    frame.extend(cbor::encode(&Value::Bytes(header_bytes.clone())));
    frame.push(0x40);
    assert!(too_deep(Change::decode(&frame).unwrap_err()));
    assert!(too_deep(Header::decode(&header_bytes).unwrap_err()));
}

// RFC 0002's examples N1 and N2 ("Byte-exact examples"), written by
// docs/rfc/vectors/0002/generate.py with cbor2, independently of this crate.
const N1: &str = include_str!("../../../docs/rfc/vectors/0002/N1.hex");
const N2: &str = include_str!("../../../docs/rfc/vectors/0002/N2.hex");

fn hex(text: &str) -> Vec<u8> {
    let text: String = text.split_whitespace().collect();
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

#[test]
fn rfc_0002_n1_reaches_level_32_and_decodes_n2_reaches_33_and_is_refused() {
    let n1 = hex(N1);
    let payload = Payload::decode(&n1).expect("N1 decodes");
    assert_eq!(payload.encode(), n1, "N1 re-encodes to its bytes");
    assert!(too_deep(Payload::decode(&hex(N2)).unwrap_err()));
}
