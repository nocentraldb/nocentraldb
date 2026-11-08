//! The change of RFC 0002: its frame, header, payload and operations, its
//! ids, its byte-exact encoding, and the keys and versions it does not know.

#[path = "support/props.rs"]
mod props;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::{BTreeMap, BTreeSet};

use ncdb_core::cbor::{self, Key, Reason, Value};
use ncdb_core::change::{Change, ChangeError, Header, Kind, Operation, Payload};
use ncdb_core::clock::Hlc;
use ncdb_core::ids::{ChangeId, MemberId, ObjectId, OperationId, SpaceId};
use proptest::prelude::*;

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

// The vectors of tests/vectors/, written by generate.py with cbor2 and
// hashlib, independently of this crate. include_str! puts them into the
// test binary, so the wasm32 run checks them too.
const SEALED_FRAME: &str = include_str!("vectors/sealed-change.frame.hex");
const SEALED_ID: &str = include_str!("vectors/sealed-change.id.hex");
const CONTROL_FRAME: &str = include_str!("vectors/control-change.frame.hex");
const CONTROL_ID: &str = include_str!("vectors/control-change.id.hex");
const NEWER_HEADER_FRAME: &str = include_str!("vectors/newer-header.frame.hex");
const NEWER_HEADER_ID: &str = include_str!("vectors/newer-header.id.hex");
const PAYLOAD: &str = include_str!("vectors/payload.hex");
const NEWER_PAYLOAD: &str = include_str!("vectors/newer-payload.hex");

const SPACE: SpaceId = SpaceId([
    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
]);

fn author() -> MemberId {
    MemberId(std::array::from_fn(|i| 0xa0 + i as u8))
}

fn object(first: u8) -> ObjectId {
    ObjectId(std::array::from_fn(|i| first + i as u8))
}

fn text(s: &str) -> Value {
    Value::Text(s.to_owned())
}

fn sealed_header(sequence: u64) -> Header {
    Header::new(
        SPACE,
        author(),
        sequence,
        0,
        Kind::SEALED,
        b"opaque ciphertext".to_vec(),
        1,
    )
}

const CLOCK: Hlc = Hlc((1_793_000_000_000 << 16) | 3);

fn vector_payload() -> Payload {
    let a = object(0x30);
    let estimate: BTreeMap<Key, Value> = [
        ("high", Value::Float(1.5)),
        ("low", Value::Float(-0.0)),
        ("nan", Value::Float(f64::NAN)),
        ("max", Value::Float(100000.0)),
        ("pi", Value::Float(1234.5678)),
        ("n", Value::Negative(999)),
    ]
    .into_iter()
    .map(|(k, v)| (Key::Text(k.into()), v))
    .collect();
    let sealed_personal: BTreeMap<Key, Value> = [
        (Key::Unsigned(1), Value::Bytes(vec![0x77; 16])),
        (Key::Unsigned(2), Value::Bytes(b"sealed".to_vec())),
    ]
    .into_iter()
    .collect();
    let dependencies: BTreeSet<ChangeId> = [
        "ca978112ca1bbdcafac231b39a23dc4da786eff8147c4e72b9807785afee48bb",
        "3e23e8160039594a33894f6564e1b1348bbd7a0088d42c4acb73eeaed59c009d",
    ]
    .into_iter()
    .map(|h| ChangeId(hex(h).try_into().unwrap()))
    .collect();
    let mut payload = Payload::new(
        CLOCK,
        dependencies,
        vec![
            Operation::new(a, "tasks".into(), None, Operation::CREATE, None),
            Operation::new(
                a,
                "tasks".into(),
                Some("title".into()),
                1,
                Some(text("Write the RFC")),
            ),
            Operation::new(
                a,
                "tasks".into(),
                Some("estimate".into()),
                1,
                Some(Value::Map(estimate)),
            ),
            Operation::new(
                object(0x50),
                "people".into(),
                Some("email".into()),
                1,
                Some(Value::Map(sealed_personal)),
            ),
        ],
    );
    payload.padding = Some(vec![0; 4]);
    payload
}

#[test]
fn generated_headers_round_trip() {
    props::check(512, props::header(), |header| {
        let bytes = header.encode();
        prop_assert_eq!(Header::decode(&bytes), Ok(header.clone()));
        prop_assert_eq!(header.encode(), bytes);
        Ok(())
    });
}

#[test]
fn generated_payloads_round_trip() {
    props::check(512, props::payload(), |payload| {
        let bytes = payload.encode();
        prop_assert_eq!(Payload::decode(&bytes), Ok(payload.clone()));
        prop_assert_eq!(payload.encode(), bytes);
        Ok(())
    });
}

#[test]
fn generated_changes_round_trip() {
    let strategy = (
        props::header(),
        proptest::collection::vec(any::<u8>(), 0..80),
    );
    props::check(512, strategy, |(header, signature)| {
        let change = Change::new(header.clone(), signature.clone());
        let decoded = Change::decode(change.bytes());
        prop_assert_eq!(decoded.as_ref(), Ok(&change));
        let decoded = decoded.unwrap();
        prop_assert_eq!(decoded.header(), &header);
        prop_assert_eq!(decoded.signature(), &signature[..]);
        prop_assert_eq!(decoded.header_bytes(), &header.encode()[..]);
        prop_assert_eq!(decoded.bytes(), change.bytes());
        prop_assert_eq!(decoded.id(), change.id());
        Ok(())
    });
}

#[test]
fn the_sealed_change_matches_its_vector() {
    let change = Change::new(sealed_header(1), vec![0xee; 64]);
    assert_eq!(to_hex(change.bytes()), to_hex(&hex(SEALED_FRAME)));
    assert_eq!(change.id(), ChangeId(hex(SEALED_ID).try_into().unwrap()));
    assert_eq!(change.id().to_string(), to_hex(&hex(SEALED_ID)));
    let decoded = Change::decode(&hex(SEALED_FRAME)).unwrap();
    assert_eq!(decoded, change);
    assert!(decoded.foldable());
}

#[test]
fn the_control_change_matches_its_vector() {
    let header = Header::new(
        SPACE,
        author(),
        2,
        3,
        Kind::CONTROL,
        vec![0xa1, 0x01, 0x01],
        1,
    );
    let change = Change::new(header, (0..64).collect());
    assert_eq!(to_hex(change.bytes()), to_hex(&hex(CONTROL_FRAME)));
    assert_eq!(change.id(), ChangeId(hex(CONTROL_ID).try_into().unwrap()));
    assert_eq!(Change::decode(&hex(CONTROL_FRAME)), Ok(change));
}

#[test]
fn the_payload_matches_its_vector() {
    let payload = vector_payload();
    assert_eq!(to_hex(&payload.encode()), to_hex(&hex(PAYLOAD)));
    assert_eq!(Payload::decode(&hex(PAYLOAD)), Ok(payload.clone()));
    assert!(payload.foldable());
}

#[test]
fn a_newer_header_is_kept_byte_for_byte_and_not_folded() {
    let bytes = hex(NEWER_HEADER_FRAME);
    let change = Change::decode(&bytes).unwrap();
    assert_eq!(change.bytes(), &bytes[..]);
    assert_eq!(
        change.id(),
        ChangeId(hex(NEWER_HEADER_ID).try_into().unwrap())
    );
    assert_eq!(change.header().format, 2);
    assert!(!change.foldable());
    // Encoding the decoded header again yields the same bytes, unknown keys
    // and all, so the change id and the signature still hold.
    let again = Change::new(change.header().clone(), change.signature().to_vec());
    assert_eq!(again.bytes(), &bytes[..]);

    let keys: Vec<u64> = change
        .header()
        .extensions()
        .iter()
        .map(|(k, _)| k)
        .collect();
    assert_eq!(keys, vec![9, 64, 1000]);
    let mut expected = sealed_header(3);
    expected.format = 2;
    assert!(expected.insert_extension(9, text("must understand")));
    assert!(expected.insert_extension(
        64,
        Value::Array(vec![
            Value::Unsigned(1),
            Value::Negative(0),
            Value::Float(1.5)
        ])
    ));
    assert!(
        expected.insert_extension(
            1000,
            Value::Map(
                [(Key::Text("note".into()), Value::Null)]
                    .into_iter()
                    .collect()
            )
        )
    );
    assert_eq!(change.header(), &expected);
}

#[test]
fn a_newer_payload_is_kept_byte_for_byte_and_not_folded() {
    let bytes = hex(NEWER_PAYLOAD);
    let payload = Payload::decode(&bytes).unwrap();
    assert_eq!(payload.encode(), bytes);
    assert_eq!(payload.format, 2);
    assert!(!payload.foldable());
    let keys: Vec<u64> = payload.extensions().iter().map(|(k, _)| k).collect();
    assert_eq!(keys, vec![64]);
    let op_keys: Vec<u64> = payload.operations[0]
        .extensions()
        .iter()
        .map(|(k, _)| k)
        .collect();
    assert_eq!(op_keys, vec![6, 70]);
}

#[test]
fn unknown_keys_from_64_up_are_ignored_and_kept() {
    let mut header = sealed_header(1);
    assert!(header.insert_extension(64, Value::Bool(true)));
    assert!(header.insert_extension(u64::MAX, Value::Null));
    let change = Change::new(header.clone(), vec![1, 2, 3]);
    let decoded = Change::decode(change.bytes()).unwrap();
    assert!(decoded.foldable());
    assert_eq!(decoded.header(), &header);

    let mut operation = Operation::new(object(1), "c".into(), Some("f".into()), 1, None);
    assert!(operation.insert_extension(64, Value::Unsigned(5)));
    let mut payload = Payload::new(CLOCK, BTreeSet::new(), vec![operation]);
    assert!(payload.insert_extension(99, text("later")));
    let decoded = Payload::decode(&payload.encode()).unwrap();
    assert!(decoded.foldable());
    assert_eq!(decoded, payload);
}

#[test]
fn unknown_keys_from_1_to_63_make_the_change_not_foldable() {
    for key in [9, 40, 63] {
        let mut header = sealed_header(1);
        assert!(header.insert_extension(key, Value::Null));
        let decoded = Change::decode(Change::new(header, vec![]).bytes()).unwrap();
        assert!(!decoded.foldable(), "header key {key}");
    }
    for key in [6, 63] {
        let mut payload = Payload::new(CLOCK, BTreeSet::new(), vec![]);
        assert!(payload.insert_extension(key, Value::Null));
        assert!(
            !Payload::decode(&payload.encode()).unwrap().foldable(),
            "payload key {key}"
        );

        let mut operation = Operation::new(object(1), "c".into(), None, 0, None);
        assert!(operation.insert_extension(key, Value::Null));
        let payload = Payload::new(CLOCK, BTreeSet::new(), vec![operation]);
        assert!(
            !Payload::decode(&payload.encode()).unwrap().foldable(),
            "operation key {key}"
        );
    }
}

#[test]
fn a_higher_format_or_an_unknown_kind_is_not_foldable() {
    let mut header = sealed_header(1);
    header.format = 2;
    assert!(!header.foldable());
    let mut header = sealed_header(1);
    header.kind = Kind(2);
    assert!(!header.foldable());
    assert!(!Kind(2).is_known());
    assert!(Kind::SEALED.is_known() && Kind::CONTROL.is_known());
    assert!(sealed_header(1).foldable());

    let mut payload = Payload::new(CLOCK, BTreeSet::new(), vec![]);
    assert!(payload.foldable());
    payload.format = 2;
    assert!(!payload.foldable());
}

#[test]
fn known_keys_are_not_extensions() {
    let mut header = sealed_header(1);
    for key in 1..=8 {
        assert!(
            !header.insert_extension(key, Value::Null),
            "header key {key}"
        );
    }
    assert_eq!(header, sealed_header(1));
    let mut payload = Payload::new(CLOCK, BTreeSet::new(), vec![]);
    let mut operation = Operation::new(object(1), "c".into(), None, 0, None);
    for key in 1..=5 {
        assert!(
            !payload.insert_extension(key, Value::Null),
            "payload key {key}"
        );
        assert!(
            !operation.insert_extension(key, Value::Null),
            "operation key {key}"
        );
    }
}

#[test]
fn the_change_id_is_the_hash_of_frame_version_and_header_bytes() {
    // Independent of the signature, which RFC 0003 may randomise.
    let a = Change::new(sealed_header(1), vec![1; 64]);
    let b = Change::new(sealed_header(1), vec![2; 64]);
    assert_eq!(a.id(), b.id());
    assert_eq!(a.id(), ChangeId::compute(1, &sealed_header(1).encode()));
    assert_ne!(a.id(), ChangeId::compute(2, &sealed_header(1).encode()));
    assert_eq!(ChangeId::CONTEXT, b"nocentraldb/change-id");
}

#[test]
fn an_operation_id_is_the_change_id_and_the_position() {
    let change = Change::new(sealed_header(1), vec![]);
    assert_eq!(
        change.operation_id(3),
        OperationId {
            change: change.id(),
            position: 3
        }
    );
}

fn change_error(bytes: &[u8]) -> ChangeError {
    match Change::decode(bytes) {
        Ok(_) => panic!("{} decoded as a change", to_hex(bytes)),
        Err(error) => error,
    }
}

fn frame(header: &[u8]) -> Vec<u8> {
    cbor::encode(&Value::Array(vec![
        Value::Unsigned(1),
        Value::Bytes(header.to_vec()),
        Value::Bytes(vec![]),
    ]))
}

fn header_map(entries: Vec<(u64, Value)>) -> Vec<u8> {
    cbor::encode(&Value::Map(
        entries
            .into_iter()
            .map(|(k, v)| (Key::Unsigned(k), v))
            .collect(),
    ))
}

fn header_entries() -> Vec<(u64, Value)> {
    vec![
        (1, Value::Bytes(SPACE.0.to_vec())),
        (2, Value::Bytes(author().0.to_vec())),
        (3, Value::Unsigned(1)),
        (4, Value::Unsigned(0)),
        (5, Value::Unsigned(0)),
        (6, Value::Bytes(vec![])),
        (7, Value::Unsigned(1)),
        (8, Value::Unsigned(1)),
    ]
}

#[test]
fn rejects_frames_that_are_not_canonical_or_not_frames() {
    let mut bytes = hex(SEALED_FRAME);
    bytes.push(0);
    assert!(matches!(
        change_error(&bytes),
        ChangeError::Encoding(e) if e.reason == Reason::TrailingBytes
    ));
    assert_eq!(change_error(&hex("82 01 40")), ChangeError::FrameShape);
    assert_eq!(change_error(&hex("a0")), ChangeError::FrameShape);
    assert_eq!(change_error(&hex("83 01 60 40")), ChangeError::FrameShape);
    assert_eq!(
        change_error(&hex("83 02 41 a0 40")),
        ChangeError::FrameVersion(2)
    );
    // The header bytes inside the frame are checked too: 0x1801 is a
    // non-shortest 1.
    let error = change_error(&frame(&header_bytes_with_long_sequence()));
    assert!(matches!(error, ChangeError::Encoding(e) if e.reason == Reason::NotCanonical));
}

fn header_bytes_with_long_sequence() -> Vec<u8> {
    let canonical = header_map(header_entries());
    // Key 3, value 1 is "03 01"; make it "03 18 01".
    let at = canonical
        .windows(2)
        .position(|w| w == [0x03, 0x01])
        .unwrap();
    let mut bytes = canonical[..at + 1].to_vec();
    bytes.extend_from_slice(&[0x18, 0x01]);
    bytes.extend_from_slice(&canonical[at + 2..]);
    bytes
}

#[test]
fn rejects_headers_with_missing_or_invalid_keys() {
    assert_eq!(Header::decode(&hex("80")), Err(ChangeError::NotAMap));
    let mut with_text = header_entries()
        .into_iter()
        .map(|(k, v)| (Key::Unsigned(k), v))
        .collect::<BTreeMap<_, _>>();
    with_text.insert(Key::Text("x".into()), Value::Null);
    assert_eq!(
        Header::decode(&cbor::encode(&Value::Map(with_text))),
        Err(ChangeError::TextKey)
    );
    for key in 1..=8 {
        let entries = header_entries()
            .into_iter()
            .filter(|(k, _)| *k != key)
            .collect();
        assert_eq!(
            Header::decode(&header_map(entries)),
            Err(ChangeError::MissingKey(key)),
            "without key {key}"
        );
    }
    let invalid = [
        (1, Value::Bytes(vec![0; 15])),
        (2, Value::Bytes(vec![0; 31])),
        (3, Value::Negative(0)),
        (4, text("0")),
        (5, Value::Null),
        (6, text("ciphertext")),
        (7, Value::Float(1.0)),
        (8, Value::Bytes(vec![1])),
    ];
    for (key, value) in invalid {
        let entries = header_entries()
            .into_iter()
            .map(|(k, v)| if k == key { (k, value.clone()) } else { (k, v) })
            .collect();
        assert_eq!(
            Header::decode(&header_map(entries)),
            Err(ChangeError::InvalidValue(key)),
            "key {key}"
        );
    }
}

#[test]
fn rejects_payloads_with_unsorted_or_repeated_dependencies() {
    let low = Value::Bytes(vec![1; 32]);
    let high = Value::Bytes(vec![2; 32]);
    let payload = |deps: Vec<Value>| {
        header_map(vec![
            (1, Value::Unsigned(1)),
            (2, Value::Unsigned(0)),
            (3, Value::Array(deps)),
            (4, Value::Array(vec![])),
        ])
    };
    assert!(Payload::decode(&payload(vec![low.clone(), high.clone()])).is_ok());
    assert_eq!(
        Payload::decode(&payload(vec![high.clone(), low.clone()])),
        Err(ChangeError::UnsortedDependencies)
    );
    assert_eq!(
        Payload::decode(&payload(vec![low.clone(), low])),
        Err(ChangeError::UnsortedDependencies)
    );
    assert_eq!(
        Payload::decode(&payload(vec![Value::Bytes(vec![1; 31])])),
        Err(ChangeError::InvalidValue(3))
    );
}

#[test]
fn rejects_payloads_and_operations_with_missing_or_invalid_keys() {
    let operation = |entries: Vec<(u64, Value)>| {
        header_map(vec![
            (1, Value::Unsigned(1)),
            (2, Value::Unsigned(0)),
            (3, Value::Array(vec![])),
            (
                4,
                Value::Array(vec![cbor::decode(&header_map(entries)).unwrap()]),
            ),
        ])
    };
    let op_entries = || {
        vec![
            (1, Value::Bytes(vec![0; 16])),
            (2, text("tasks")),
            (3, text("title")),
            (4, Value::Unsigned(1)),
            (5, text("x")),
        ]
    };
    assert!(Payload::decode(&operation(op_entries())).is_ok());
    for key in [1, 2, 4] {
        let entries = op_entries()
            .into_iter()
            .filter(|(k, _)| *k != key)
            .collect();
        assert_eq!(
            Payload::decode(&operation(entries)),
            Err(ChangeError::MissingKey(key)),
            "operation without key {key}"
        );
    }
    for (key, value) in [
        (1, Value::Bytes(vec![0; 17])),
        (2, Value::Bytes(vec![])),
        (3, Value::Unsigned(3)),
        (4, Value::Negative(1)),
    ] {
        let entries = op_entries()
            .into_iter()
            .map(|(k, v)| if k == key { (k, value.clone()) } else { (k, v) })
            .collect();
        assert_eq!(
            Payload::decode(&operation(entries)),
            Err(ChangeError::InvalidValue(key)),
            "operation key {key}"
        );
    }
    for key in [1, 2, 3, 4] {
        let entries = vec![
            (1, Value::Unsigned(1)),
            (2, Value::Unsigned(0)),
            (3, Value::Array(vec![])),
            (4, Value::Array(vec![])),
        ]
        .into_iter()
        .filter(|(k, _)| *k != key)
        .collect();
        assert_eq!(
            Payload::decode(&header_map(entries)),
            Err(ChangeError::MissingKey(key)),
            "payload without key {key}"
        );
    }
    let bad_padding = header_map(vec![
        (1, Value::Unsigned(1)),
        (2, Value::Unsigned(0)),
        (3, Value::Array(vec![])),
        (4, Value::Array(vec![])),
        (5, Value::Unsigned(0)),
    ]);
    assert_eq!(
        Payload::decode(&bad_padding),
        Err(ChangeError::InvalidValue(5))
    );
}

#[test]
fn key_0_is_must_understand() {
    // RFC 0002 names keys 1 to 63 and 64 up; key 0 is in neither range.
    // This core treats it as "must understand": a change that uses it is
    // kept, and not folded.
    let mut header = sealed_header(1);
    assert!(header.insert_extension(0, Value::Null));
    let decoded = Change::decode(Change::new(header, vec![]).bytes()).unwrap();
    assert!(!decoded.foldable(), "header key 0");

    let mut payload = Payload::new(CLOCK, BTreeSet::new(), vec![]);
    assert!(payload.insert_extension(0, Value::Null));
    let decoded = Payload::decode(&payload.encode()).unwrap();
    assert!(!decoded.foldable(), "payload key 0");
    assert_eq!(decoded, payload);

    let mut operation = Operation::new(object(1), "c".into(), None, 0, None);
    assert!(operation.insert_extension(0, Value::Null));
    let payload = Payload::new(CLOCK, BTreeSet::new(), vec![operation]);
    assert!(
        !Payload::decode(&payload.encode()).unwrap().foldable(),
        "operation key 0"
    );
}
