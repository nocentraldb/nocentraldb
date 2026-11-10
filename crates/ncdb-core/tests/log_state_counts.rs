//! How many changes of a log are parked and rejected, per reason, and how
//! many operations of its folded changes are parked, per reason (RFC 0009,
//! "Sync and status": `parked` and `rejected` of `space.status`). The log
//! keeps the counts as states are set, so a status reads them without a
//! pass over the log, and a change whose state moves leaves its old reason.

#[path = "support/member.rs"]
mod member;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::BTreeMap;

use ncdb_core::change::Operation;
use ncdb_core::ids::ObjectId;
use ncdb_core::log::{OperationParkReason, ParkReason, RejectReason, StateCounts};

use member::*;

/// A member that holds the key of generation 0.
fn with_key() -> Member {
    let mut m = Member::default();
    m.receive(&envelope(FOUNDER, 1, 0));
    m
}

#[test]
fn a_new_log_counts_nothing() {
    let m = Member::default();

    assert_eq!(m.log.state_counts(), &StateCounts::default());
}

#[test]
fn parked_and_rejected_changes_count_per_reason() {
    let mut m = with_key();
    let mut suite = sealed_header(FOUNDER, 2, 0, payload(vec![set_title(1)]));
    suite.suite = 2;
    m.receive(&frame(suite, GOOD));
    let mut kind = sealed_header(FOUNDER, 3, 0, payload(vec![set_title(1)]));
    kind.kind = ncdb_core::change::Kind(9);
    m.receive(&frame(kind, GOOD));
    let mut also_suite = sealed_header(FOUNDER, 4, 0, payload(vec![set_title(2)]));
    also_suite.suite = 3;
    m.receive(&frame(also_suite, GOOD));
    m.receive(&frame(
        sealed_header(FOUNDER, 5, 0, payload(vec![set_title(1)])),
        BAD,
    ));
    m.receive(&frame(sealed_header(5, 1, 0, GARBLED.to_vec()), GOOD));
    m.receive(&sealed(FOUNDER, 6, 0, vec![set_title(3)]));

    let counts = m.log.state_counts();

    assert_eq!(
        counts.parked,
        BTreeMap::from([(ParkReason::UnknownSuite, 2), (ParkReason::UnknownKind, 1)])
    );
    assert_eq!(
        counts.rejected,
        BTreeMap::from([(RejectReason::BadSignature, 1), (RejectReason::NotMember, 1)])
    );
    assert!(counts.parked_operations.is_empty());
}

#[test]
fn parked_operations_of_folded_changes_count_per_reason() {
    let mut m = with_key();
    let unknown_collection = Operation::new(ObjectId([1; 16]), "later".into(), None, 0, None);
    let mut unknown_field = set_title(1);
    unknown_field.field = Some("due".into());
    let mut other_field = set_title(2);
    other_field.field = Some("owner".into());
    m.receive(&sealed(
        FOUNDER,
        2,
        0,
        vec![set_title(1), unknown_collection, unknown_field],
    ));
    m.receive(&sealed(FOUNDER, 3, 0, vec![other_field]));

    let counts = m.log.state_counts();

    assert_eq!(
        counts.parked_operations,
        BTreeMap::from([
            (OperationParkReason::UnknownCollection, 1),
            (OperationParkReason::UnknownField, 2),
        ])
    );
    assert!(counts.parked.is_empty());
    assert!(counts.rejected.is_empty());
}

#[test]
fn a_change_whose_state_moves_leaves_its_old_reason() {
    let mut m = Member::default();
    // No key envelope yet: rejected for lack of a key.
    m.receive(&sealed(FOUNDER, 2, 0, vec![set_title(1)]));
    assert_eq!(
        m.log.state_counts().rejected,
        BTreeMap::from([(RejectReason::NoKey, 1)])
    );

    // The envelope arrives: the change is checked again and folded.
    m.receive(&envelope(FOUNDER, 1, 0));

    assert_eq!(m.log.state_counts(), &StateCounts::default());
}
