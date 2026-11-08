//! A member takes a received change through the steps of RFC 0002
//! Amendment 1, "What a member does with a change it receives", in order,
//! and keeps it in its log folded, parked or rejected (NOC-28). One test
//! per row of the amendment's table.

#[path = "support/member.rs"]
mod member;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::cbor::Value;
use ncdb_core::change::{Change, Kind};
use ncdb_core::ids::SpaceId;
use ncdb_core::log::{Appended, ParkReason, Position, State};
use ncdb_core::receive::ReceiveError;

use member::*;

fn folded() -> State {
    State::Folded { parked: vec![] }
}

/// A member that holds the key of generation 0.
fn with_key() -> Member {
    let mut m = Member::default();
    m.receive(&envelope(FOUNDER, 1, 0));
    m
}

#[test]
fn step_1_a_frame_that_does_not_decode_is_not_appended() {
    let mut m = Member::default();
    let mut bytes = sealed(FOUNDER, 1, 0, vec![set_title(1)]);
    bytes.push(0);
    let result = m.log.receive(&bytes, &mut m.control, &mut m.fold);
    assert!(matches!(result, Err(ReceiveError::Decode(_))), "{result:?}");
    assert!(m.log.is_empty());
}

#[test]
fn a_change_of_another_space_is_not_appended() {
    let mut m = Member::default();
    let mut header = sealed_header(FOUNDER, 1, 0, payload(vec![]));
    header.space = SpaceId([9; 16]);
    let result = m
        .log
        .receive(&frame(header, GOOD), &mut m.control, &mut m.fold);
    assert_eq!(result, Err(ReceiveError::WrongSpace));
    assert!(m.log.is_empty());
}

#[test]
fn a_change_that_passes_every_step_is_folded_operation_by_operation() {
    let mut m = with_key();
    let change = m.receive(&sealed(FOUNDER, 2, 0, vec![set_title(1), set_title(2)]));
    assert_eq!(m.state(&change), folded());
    let ids = [change.operation_id(0), change.operation_id(1)];
    assert_eq!(m.fold.folded, ids.into_iter().collect());
}

#[test]
fn a_duplicate_changes_nothing() {
    let mut m = with_key();
    let bytes = sealed(FOUNDER, 2, 0, vec![set_title(1)]);
    m.receive(&bytes);
    let again = m.log.receive(&bytes, &mut m.control, &mut m.fold);
    assert_eq!(again, Ok(Appended::Duplicate(Position(2))));
    assert_eq!(m.fold.folded.len(), 1);
}

fn parked_at_step_2(header: ncdb_core::change::Header, reason: ParkReason) {
    let mut m = with_key();
    let change = m.receive(&frame(header, GOOD));
    assert_eq!(m.state(&change), State::Parked(reason));
    assert!(m.fold.folded.is_empty());
}

#[test]
fn step_2_an_unknown_suite_parks_the_change() {
    let mut header = sealed_header(FOUNDER, 2, 0, payload(vec![set_title(1)]));
    header.suite = 2;
    parked_at_step_2(header, ParkReason::UnknownSuite);
}

#[test]
fn step_2_an_unknown_kind_parks_the_change() {
    let mut header = sealed_header(FOUNDER, 2, 0, payload(vec![set_title(1)]));
    header.kind = Kind(7);
    parked_at_step_2(header, ParkReason::UnknownKind);
}

#[test]
fn step_2_an_unknown_header_format_parks_the_change() {
    let mut header = sealed_header(FOUNDER, 2, 0, payload(vec![set_title(1)]));
    header.format = 2;
    parked_at_step_2(header, ParkReason::UnknownHeaderFormat);
}

#[test]
fn step_2_an_unknown_must_understand_header_key_parks_the_change() {
    let mut header = sealed_header(FOUNDER, 2, 0, payload(vec![set_title(1)]));
    assert!(header.insert_extension(9, Value::Unsigned(1)));
    parked_at_step_2(header, ParkReason::UnknownHeaderKey);
}

#[test]
fn step_2_an_unknown_may_ignore_header_key_does_not_park() {
    let mut m = with_key();
    let mut header = sealed_header(FOUNDER, 2, 0, payload(vec![set_title(1)]));
    assert!(header.insert_extension(64, Value::Unsigned(1)));
    let change = m.receive(&frame(header, GOOD));
    assert_eq!(m.state(&change), folded());
}

#[test]
fn a_raw_append_carries_no_member_state() {
    // The relay's log holds no states (RFC 0002 Amendment 1): only a
    // change the member received through its checks has one.
    let mut m = Member::default();
    let change = Change::decode(&sealed(FOUNDER, 1, 0, vec![])).unwrap();
    m.log.append(change.clone()).unwrap();
    assert_eq!(m.log.state(change.id()), None);
}
