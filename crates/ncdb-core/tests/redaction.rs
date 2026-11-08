//! No content and no key appears in the `Debug`, `Display` or error output
//! of the change and the log (GENESIS.md: "Never write content, keys or
//! personal fields into logs, errors, telemetry or crash reports").
//!
//! Everything a member could read only after decryption is content here:
//! the sealed payload, the operations with their objects, collections,
//! fields and values, unknown keys' values and the padding. The signature
//! is shown only by its length too. Ids of spaces, members and changes,
//! sequence numbers, formats, kinds and clocks are what the RFC lets the
//! relay or the member see without content, and may appear.

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::{BTreeMap, BTreeSet};

use ncdb_core::cbor::{self, Key, Value};
use ncdb_core::change::{Change, Header, Kind, Operation, Payload};
use ncdb_core::clock::Hlc;
use ncdb_core::ids::{MemberId, ObjectId, SpaceId};
use ncdb_core::log::Log;

const SECRET: &str = "SECRET-MARKER";
const SECRET_NUMBER: u64 = 31_337_424_242;
const SECRET_FLOAT: f64 = 27_182.818_28;
const OBJECT: ObjectId = ObjectId([0x5e; 16]);

/// The ways a secret can show in text: as itself, as hex, as a list of
/// decimal bytes, and the secret numbers and object id.
fn leaks(output: &str) -> Vec<String> {
    let bytes = SECRET.as_bytes();
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    let decimal = bytes
        .iter()
        .map(u8::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    let object_hex: String = OBJECT.0.iter().map(|b| format!("{b:02x}")).collect();
    let forms = [
        SECRET.to_owned(),
        SECRET.to_lowercase(),
        hex.clone(),
        hex.to_uppercase(),
        decimal,
        SECRET_NUMBER.to_string(),
        SECRET_FLOAT.to_string(),
        "94, 94, 94".to_owned(),
        object_hex,
    ];
    forms
        .into_iter()
        .filter(|form| output.contains(form.as_str()))
        .collect()
}

fn assert_no_leak(what: &str, output: String) {
    let found = leaks(&output);
    assert!(found.is_empty(), "{what} shows {found:?}:\n{output}");
}

fn assert_debug_hides(what: &str, value: &impl std::fmt::Debug) {
    assert_no_leak(what, format!("{value:?}"));
    assert_no_leak(what, format!("{value:#?}"));
}

fn secret_value() -> Value {
    let map: BTreeMap<Key, Value> = [
        (Key::Text(SECRET.into()), Value::Text(SECRET.into())),
        (Key::Unsigned(1), Value::Bytes(SECRET.as_bytes().to_vec())),
        (Key::Unsigned(2), Value::Unsigned(SECRET_NUMBER)),
        (Key::Unsigned(3), Value::Negative(SECRET_NUMBER)),
        (Key::Unsigned(4), Value::Float(SECRET_FLOAT)),
    ]
    .into_iter()
    .collect();
    Value::Array(vec![Value::Map(map), Value::Text(SECRET.into())])
}

fn secret_operation() -> Operation {
    let mut operation = Operation::new(
        OBJECT,
        SECRET.into(),
        Some(SECRET.into()),
        SECRET_NUMBER,
        Some(secret_value()),
    );
    assert!(operation.insert_extension(70, Value::Text(SECRET.into())));
    operation
}

fn secret_payload() -> Payload {
    let mut payload = Payload::new(Hlc(5), BTreeSet::new(), vec![secret_operation()]);
    payload.padding = Some(SECRET.as_bytes().to_vec());
    assert!(payload.insert_extension(64, secret_value()));
    payload
}

fn secret_header() -> Header {
    let mut header = Header::new(
        SpaceId([1; 16]),
        MemberId([2; 32]),
        1,
        0,
        Kind::SEALED,
        // Stands for the ciphertext, and for a control body.
        SECRET.as_bytes().to_vec(),
        1,
    );
    assert!(header.insert_extension(9, secret_value()));
    header
}

fn secret_change() -> Change {
    Change::new(secret_header(), SECRET.as_bytes().to_vec())
}

#[test]
fn values_and_keys_do_not_show_their_content() {
    assert_debug_hides("Value", &secret_value());
    assert_debug_hides("Key", &Key::Text(SECRET.into()));
    assert_debug_hides("Key", &Key::Unsigned(SECRET_NUMBER));
}

#[test]
fn operations_and_payloads_do_not_show_their_content() {
    assert_debug_hides("Operation", &secret_operation());
    assert_debug_hides("Payload", &secret_payload());
    assert_debug_hides("Extensions", secret_payload().extensions());
}

#[test]
fn headers_changes_and_logs_do_not_show_payload_or_signature() {
    assert_debug_hides("Header", &secret_header());
    let change = secret_change();
    assert_debug_hides("Change", &change);
    let mut log = Log::new(SpaceId([1; 16]));
    log.append(change).unwrap();
    assert_debug_hides("Log", &log);
}

#[test]
fn what_may_show_still_shows() {
    let output = format!("{:?}", secret_change());
    assert!(
        output.contains(&secret_change().id().to_string()),
        "{output}"
    );
    assert!(output.contains("sequence: 1"), "{output}");
    let output = format!("{:?}", secret_payload());
    assert!(output.contains("Hlc(5)"), "{output}");
}

#[test]
fn errors_do_not_show_the_bytes_they_reject() {
    let text = cbor::encode(&Value::Text(SECRET.into()));
    // Trailing bytes after a secret, a secret in a header that is not a
    // map, and a secret in a field of the wrong type.
    let mut trailing = text.clone();
    trailing.push(0);
    let not_a_map = cbor::encode(&Value::Array(vec![Value::Text(SECRET.into())]));
    let frame = |header: Vec<u8>| {
        cbor::encode(&Value::Array(vec![
            Value::Unsigned(1),
            Value::Bytes(header),
            Value::Bytes(SECRET.as_bytes().to_vec()),
        ]))
    };
    let mut fields: Vec<(u64, Value)> = vec![
        (1, Value::Bytes(vec![0; 16])),
        (2, Value::Bytes(vec![0; 32])),
        (3, Value::Text(SECRET.into())),
    ];
    fields.extend((4..=8).map(|k| (k, Value::Unsigned(1))));
    let wrong_type = cbor::encode(&Value::Map(
        fields
            .into_iter()
            .map(|(k, v)| (Key::Unsigned(k), v))
            .collect(),
    ));
    let inputs = [
        trailing.clone(),
        frame(not_a_map.clone()),
        frame(wrong_type.clone()),
    ];
    for input in &inputs {
        let error = Change::decode(input).unwrap_err();
        assert_debug_hides("ChangeError", &error);
        assert_no_leak("ChangeError", error.to_string());
    }
    let error = cbor::decode(&trailing).unwrap_err();
    assert_debug_hides("DecodeError", &error);
    assert_no_leak("DecodeError", error.to_string());
    for error in [
        Header::decode(&wrong_type).unwrap_err(),
        Payload::decode(&not_a_map).unwrap_err(),
    ] {
        assert_debug_hides("ChangeError", &error);
        assert_no_leak("ChangeError", error.to_string());
    }
}
