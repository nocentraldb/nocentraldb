//! The newest model version a space's parked data needs (RFC 0002, "What
//! the model version is for"; RFC 0005, "Data from a newer model", point
//! 6): the highest model version, from payload key 64, among the parked
//! changes and the changes with parked operations of the member's own
//! model. A change parked as "another model" belongs to another lineage and
//! does not count, nor does a folded change with nothing parked.

#[path = "support/member.rs"]
mod member;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::BTreeSet;

use ncdb_core::cbor::Value;
use ncdb_core::change::{Change, Operation, Payload, WriterModel};
use ncdb_core::clock::Hlc;
use ncdb_core::ids::ObjectId;

use member::*;

/// A member that holds the key of generation 0.
fn with_key() -> Member {
    let mut m = Member::default();
    m.receive(&envelope(FOUNDER, 1, 0));
    m
}

/// An operation on a collection the member's lock does not know.
fn later() -> Operation {
    Operation::new(ObjectId([1; 16]), "later".into(), None, 0, None)
}

/// Receives a change of `FOUNDER` under `sequence`, written by `model`,
/// with `operations` and, when `unknown_key`, an unknown "must understand"
/// payload key.
fn receive(
    m: &mut Member,
    sequence: u64,
    model: Option<WriterModel>,
    operations: Vec<Operation>,
    unknown_key: bool,
) -> Change {
    let mut payload = Payload::new(Hlc(sequence << 16), BTreeSet::new(), operations);
    if let Some(model) = model {
        payload.set_writer_model(model);
    }
    if unknown_key {
        assert!(payload.insert_extension(9, Value::Unsigned(1)));
    }
    m.receive(&frame(
        sealed_header(FOUNDER, sequence, 0, payload.encode()),
        GOOD,
    ))
}

fn own(version: u64) -> Option<WriterModel> {
    Some(WriterModel { id: MODEL, version })
}

#[test]
fn nothing_parked_needs_no_model() {
    let mut m = with_key();
    receive(&mut m, 2, own(50), vec![set_title(1)], false);

    assert_eq!(m.log.newest_model(&m.control, MODEL), None);
}

#[test]
fn the_highest_version_among_parked_changes_and_operations_of_the_own_model() {
    let mut m = with_key();
    receive(&mut m, 2, own(5), vec![set_title(1), later()], false);
    receive(&mut m, 3, own(7), vec![set_title(2)], true);
    receive(&mut m, 4, own(6), vec![later()], false);
    // Folded whole: nothing parked, so its version does not count.
    receive(&mut m, 5, own(50), vec![set_title(3)], false);
    // Another lineage: parked as another model, and not counted.
    let other = Some(WriterModel {
        id: [0x01; 16],
        version: 99,
    });
    receive(&mut m, 6, other, vec![later()], false);
    // No key 64: parked, but it names no version.
    receive(&mut m, 7, None, vec![later()], false);

    assert_eq!(m.log.newest_model(&m.control, MODEL), Some(7));
}
