//! What sealing and opening a large change holds in memory (NOC-422). A
//! change's operations, as a tree of values, take many times the bytes of
//! their encoding; sealing and opening must not hold such a tree of the
//! whole payload next to the operations, nor several copies of it, so that
//! a change near the relay's limit fits a phone or a browser tab.

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[path = "support/peak.rs"]
mod peak;

use std::collections::{BTreeMap, BTreeSet};

use ncdb_core::cbor::{Key, Value};
use ncdb_core::change::{Change, Operation, Payload};
use ncdb_core::clock::Hlc;
use ncdb_core::ids::{ObjectId, SpaceId};
use ncdb_core::keys::{MemberKey, SpaceKey};
use ncdb_core::seal;

#[global_allocator]
static COUNTING: peak::Counting = peak::Counting;

/// Records of a task tracker, as an import writes them: a create and six
/// fields each, every field's value a map of the value and the operations
/// it replaces, as basic fields write them.
fn payload(records: u32) -> Payload {
    let mut operations = Vec::new();
    for record in 0..records {
        let mut object = [0; 16];
        object[..4].copy_from_slice(&record.to_be_bytes());
        let object = ObjectId(object);
        operations.push(Operation::new(object, "tasks".into(), None, 0, None));
        for field in ["title", "status", "priority", "estimate", "urgent", "tags"] {
            let value = BTreeMap::from([
                (
                    Key::Unsigned(1),
                    Value::Text(format!("{field} of record {record:06}")),
                ),
                (Key::Unsigned(2), Value::Array(Vec::new())),
            ]);
            operations.push(Operation::new(
                object,
                "tasks".into(),
                Some(field.into()),
                1,
                Some(Value::Map(value)),
            ));
        }
    }
    Payload::new(Hlc(1), BTreeSet::new(), operations)
}

fn member() -> MemberKey {
    MemberKey::from_secrets([1; 32], [2; 32])
}

fn space_key() -> SpaceKey {
    SpaceKey::from_bytes([3; 32])
}

/// Sealing holds the payload's bytes a few times over (plaintext,
/// ciphertext, header, frame) and one operation's tree at a time, not a
/// tree or a copy of the whole payload.
#[test]
fn sealing_holds_a_few_copies_of_the_payloads_bytes_and_no_tree_of_them() {
    let payload = payload(2_000);
    let size = payload.encode().len();
    let author = member();
    let key = space_key();
    let (_change, held) =
        peak::peak(|| seal::seal(&author, &key, SpaceId([4; 16]), 1, 1, &payload));
    let held = held.peak;
    assert!(
        held <= 8 * size,
        "sealing {size} bytes held {held} bytes, {:.1} times as many",
        held as f64 / size as f64
    );
}

/// An opened payload holds each operation's value as the bytes the change
/// carries, not as a tree: a few times the payload's bytes, where a tree
/// of maps takes about twenty. Opening holds the decrypted bytes and one
/// operation's tree at a time beyond it, never a tree of the whole payload.
#[test]
fn an_opened_payload_holds_a_few_times_its_bytes() {
    let payload = payload(2_000);
    let size = payload.encode().len();
    let author = member();
    let key = space_key();
    let change = seal::seal(&author, &key, SpaceId([4; 16]), 1, 1, &payload);
    drop(payload);
    let (opened, held) = peak::peak(|| seal::open(&change, &author.public(), &key).unwrap());
    assert_eq!(opened.operations.len(), 14_000);
    let ratio = |bytes: usize| bytes as f64 / size as f64;
    assert!(
        held.kept <= 4 * size && held.peak <= 6 * size,
        "opening {size} bytes kept {:.1} times as many, and held {:.1} times at its peak",
        ratio(held.kept),
        ratio(held.peak)
    );
}

/// A change keeps its frame's bytes and the header's payload, which the
/// relay and the checks read: the ciphertext twice, not a third time as
/// the header's bytes, which are part of the frame. Decoding a frame
/// builds no tree of it, which would copy the ciphertext again.
#[test]
fn a_decoded_change_holds_its_ciphertext_twice() {
    let payload = payload(2_000);
    let frame = seal::seal(&member(), &space_key(), SpaceId([4; 16]), 1, 1, &payload)
        .bytes()
        .to_vec();
    drop(payload);
    let size = frame.len();
    let (change, held) = peak::peak(|| Change::decode(&frame).unwrap());
    assert_eq!(change.bytes(), frame.as_slice());
    let ratio = |bytes: usize| bytes as f64 / size as f64;
    assert!(
        held.kept <= 2 * size + 1_000 && held.peak <= 2 * size + 1_000,
        "decoding a frame of {size} bytes kept {:.1} times as many, and held {:.1} times at its peak",
        ratio(held.kept),
        ratio(held.peak)
    );
}
