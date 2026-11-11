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
use ncdb_core::checks::SealChecks;
use ncdb_core::clock::Hlc;
use ncdb_core::envelope;
use ncdb_core::field_op::Origin;
use ncdb_core::ids::{MemberId, ObjectId, SpaceId};
use ncdb_core::keys::{MemberKey, SpaceKey};
use ncdb_core::log::{Log, OperationParkReason};
use ncdb_core::membership::{self, Certificate, ControlBody, Entry, Genesis, MemberKind};
use ncdb_core::object::ObjectOperation;
use ncdb_core::receive::{Fold, Stop};
use ncdb_core::seal;

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
    // Key 65, a "may ignore" key with no meaning: key 64 names the writer's
    // model, and a secret there would park the change as another model.
    assert!(payload.insert_extension(65, secret_value()));
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

/// 32 bytes made of the secret marker, so that any way of printing a key
/// that holds them shows the marker in one of the forms `leaks` knows.
fn secret_key_bytes() -> [u8; 32] {
    std::array::from_fn(|i| SECRET.as_bytes()[i % SECRET.len()])
}

#[test]
fn keys_and_sealing_errors_do_not_show_keys_or_content() {
    // RFC 0003, "Keys never reach logs, errors or telemetry": canary keys
    // and content, every error path of opening a change, and every type
    // formatted.
    let author = MemberKey::from_secrets(secret_key_bytes(), secret_key_bytes());
    let other = MemberKey::from_secrets([3; 32], [4; 32]);
    let key = SpaceKey::from_bytes(secret_key_bytes());
    assert_debug_hides("MemberKey", &author);
    assert_debug_hides("SpaceKey", &key);

    let space = SpaceId([1; 16]);
    let change = seal::seal(&author, &key, space, 1, 1, &secret_payload());
    assert_debug_hides("sealed Change", &change);
    let opened = seal::open(&change, &author.public(), &key).unwrap();
    assert_debug_hides("opened Payload", &opened);

    let mut tampered = change.header().clone();
    tampered.sequence = 2;
    let mut unknown_suite = change.header().clone();
    unknown_suite.suite = 2;
    let control = Header::new(space, author.id(), 2, 1, Kind::CONTROL, vec![], seal::SUITE);
    let mut invalid = author.public();
    invalid.signing = [0xff; 32];
    let invalid_change = Change::new(
        Header::new(space, invalid.id(), 1, 1, Kind::SEALED, vec![], seal::SUITE),
        vec![0; 64],
    );
    let errors = [
        seal::open(&change, &author.public(), &SpaceKey::from_bytes([9; 32])),
        seal::open(&change, &other.public(), &key),
        seal::open(
            &Change::new(tampered, change.signature().to_vec()),
            &author.public(),
            &key,
        ),
        seal::open(&seal::sign(&author, unknown_suite), &author.public(), &key),
        seal::open(&seal::sign(&author, control), &author.public(), &key),
        seal::open(&invalid_change, &invalid, &key),
    ];
    for result in errors {
        let error = result.unwrap_err();
        assert_debug_hides("OpenError", &error);
        assert_no_leak("OpenError", error.to_string());
    }
}

/// A space `author` creates, and the envelope that wraps `key`, the space
/// key of its first generation, to `author`: the space, its first
/// generation and the two control changes.
fn founded_by(author: &MemberKey, key: &SpaceKey) -> (SpaceId, u64, [Change; 2]) {
    let nonce = [7; 16];
    let space = membership::space_id(author.id(), &nonce);
    let first = membership::generation_id(space, author.id(), &[]);
    let account = ed25519_dalek::SigningKey::from_bytes(&[0xAC; 32]);
    let genesis = Genesis {
        nonce,
        creator: author.public(),
        certificate: Certificate::sign(&account, author.id(), MemberKind::Agent),
        first_generation: first,
        model_id: membership::seal_model_id(key, space, author.id(), [1; 16]),
    };
    let body = ControlBody::new(BTreeSet::new(), Entry::Genesis(genesis));
    let genesis = membership::write(author, space, 1, first, &body);
    let sealed = envelope::seal(key, space, first, &author.public()).unwrap();
    let body = ControlBody::new([genesis.id()].into(), Entry::KeyEnvelope(sealed));
    let to_author = membership::write(author, space, 2, first, &body);
    (space, first, [genesis, to_author])
}

/// A model that knows no collection, so every operation is parked.
struct NoModel;

impl Fold for NoModel {
    fn model_id(&self) -> [u8; 16] {
        [0; 16]
    }

    fn knows_collection(&self, _collection: &str) -> bool {
        false
    }

    fn knows_field(&self, _collection: &str, _field: &str) -> bool {
        false
    }

    fn fold(&mut self, _origin: Origin, _operation: &Operation) -> Result<(), OperationParkReason> {
        Ok(())
    }

    fn fold_object(&mut self, _object: ObjectId, _operation: &ObjectOperation) {
        unreachable!("a model without collections folds no object")
    }

    fn clear_field(&mut self, _object: ObjectId, _collection: &str, _field: &str) {}

    fn clear_object(&mut self, _object: ObjectId) {}
}

#[test]
fn the_seal_checks_do_not_show_keys_or_content() {
    // NOC-48: the checks hold the space keys, and every way a received
    // change stops, folded or not, leaves no key and no plaintext in a
    // reason, an error or `Debug` output.
    let author = MemberKey::from_secrets(secret_key_bytes(), secret_key_bytes());
    let key = SpaceKey::from_bytes(secret_key_bytes());
    let (space, first, controls) = founded_by(&author, &key);
    // The author receives its own changes: the envelope is to it.
    let same_author = MemberKey::from_secrets(secret_key_bytes(), secret_key_bytes());
    let mut checks = SealChecks::new(space, same_author);
    assert_debug_hides("SealChecks", &checks);

    let folded = seal::seal(&author, &key, space, 3, first, &secret_payload());
    let other_key = SpaceKey::from_bytes([9; 32]);
    let undecryptable = seal::seal(&author, &other_key, space, 4, first, &secret_payload());
    let not_member = seal::seal(&author, &key, space, 5, first ^ 1, &secret_payload());
    let mut bad_signature = folded.bytes().to_vec();
    let last = bad_signature.len() - 1;
    bad_signature[last] ^= 1;

    let mut log = Log::new(space);
    for bytes in [
        controls[0].bytes(),
        controls[1].bytes(),
        folded.bytes(),
        undecryptable.bytes(),
        not_member.bytes(),
        &bad_signature,
    ] {
        let appended = log.receive(bytes, &mut checks, &mut NoModel);
        assert_debug_hides("receive result", &appended);
        let change = Change::decode(bytes).unwrap();
        assert_debug_hides("State", &log.state(change.id()));
    }
    assert!(matches!(
        log.state(folded.id()),
        Some(ncdb_core::log::State::Folded { .. })
    ));
    assert_debug_hides("SealChecks", &checks);
    assert_debug_hides("member Log", &log);

    let mut garbled = folded.bytes().to_vec();
    garbled.push(0);
    let error = log
        .receive(&garbled, &mut checks, &mut NoModel)
        .unwrap_err();
    assert_debug_hides("ReceiveError", &error);
    assert_no_leak("ReceiveError", error.to_string());

    let opened = seal::open(&folded, &author.public(), &key);
    assert_debug_hides("Stop", &opened.map_err(Stop::from));
}
