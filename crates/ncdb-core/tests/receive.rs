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
use ncdb_core::change::Operation;
use ncdb_core::ids::ObjectId;
use ncdb_core::log::{
    Appended, OperationParkReason, ParkReason, ParkedOperation, Position, RejectReason, State,
};
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

fn rejected(m: &mut Member, bytes: &[u8], reason: RejectReason) {
    let change = m.receive(bytes);
    assert_eq!(m.state(&change), State::Rejected(reason));
    assert!(m.fold.folded.is_empty());
}

#[test]
fn step_3_an_author_no_membership_entry_names_is_rejected_as_unknown_author() {
    rejected(
        &mut with_key(),
        &sealed(5, 1, 0, vec![set_title(1)]),
        RejectReason::UnknownAuthor,
    );
}

#[test]
fn step_3_a_signature_that_does_not_verify_is_rejected_as_bad_signature() {
    let header = sealed_header(FOUNDER, 2, 0, payload(vec![set_title(1)]));
    rejected(&mut with_key(), &frame(header, BAD), RejectReason::BadSignature);
}

#[test]
fn step_4_an_author_who_is_not_a_member_at_the_generation_is_rejected() {
    let mut m = with_key();
    m.receive(&membership(FOUNDER, 2, 5, 1));
    rejected(
        &mut m,
        &sealed(5, 1, 0, vec![set_title(1)]),
        RejectReason::NotMember,
    );
}

#[test]
fn step_4_a_control_change_of_a_non_member_is_rejected() {
    let mut m = Member::default();
    m.receive(&membership(FOUNDER, 1, 5, 1));
    rejected(&mut m, &envelope(5, 1, 0), RejectReason::NotMember);
    assert!(m.control.keys.is_empty(), "a rejected control change is not added");
}

#[test]
fn step_5_a_change_without_a_key_envelope_is_rejected_as_no_key() {
    rejected(
        &mut Member::default(),
        &sealed(FOUNDER, 1, 0, vec![set_title(1)]),
        RejectReason::NoKey,
    );
}

#[test]
fn step_5_a_payload_that_does_not_decrypt_is_rejected_as_undecryptable() {
    let header = sealed_header(FOUNDER, 2, 0, GARBLED.to_vec());
    rejected(&mut with_key(), &frame(header, GOOD), RejectReason::Undecryptable);
}

fn parked_at_step_6(plaintext: Vec<u8>, reason: ParkReason) {
    let mut m = with_key();
    let change = m.receive(&frame(sealed_header(FOUNDER, 2, 0, plaintext), GOOD));
    assert_eq!(m.state(&change), State::Parked(reason));
    assert!(m.fold.folded.is_empty());
}

#[test]
fn step_6_an_unknown_payload_format_parks_the_change() {
    let mut p = ncdb_core::change::Payload::decode(&payload(vec![set_title(1)])).unwrap();
    p.format = 2;
    parked_at_step_6(p.encode(), ParkReason::UnknownPayloadFormat);
}

#[test]
fn step_6_a_payload_this_core_cannot_read_parks_the_change_as_unknown_format() {
    // A newer payload format may not have the keys of format 1 at all.
    parked_at_step_6(vec![0xa0], ParkReason::UnknownPayloadFormat);
}

#[test]
fn step_6_an_unknown_must_understand_payload_key_parks_the_change() {
    let mut p = ncdb_core::change::Payload::decode(&payload(vec![set_title(1)])).unwrap();
    assert!(p.insert_extension(9, Value::Unsigned(1)));
    parked_at_step_6(p.encode(), ParkReason::UnknownPayloadKey);
}

#[test]
fn the_steps_run_in_order_and_the_first_failing_one_decides() {
    // Unknown suite (step 2) beats an unknown author (step 3).
    let mut header = sealed_header(5, 1, 0, GARBLED.to_vec());
    header.suite = 2;
    parked_at_step_2(header, ParkReason::UnknownSuite);
    // An unknown author (step 3) beats a missing key (step 5).
    rejected(
        &mut Member::default(),
        &frame(sealed_header(5, 1, 0, GARBLED.to_vec()), BAD),
        RejectReason::UnknownAuthor,
    );
    // Not a member (step 4) beats a payload that does not decrypt (step 5).
    let mut m = with_key();
    m.receive(&membership(FOUNDER, 2, 5, 1));
    rejected(
        &mut m,
        &frame(sealed_header(5, 1, 0, GARBLED.to_vec()), GOOD),
        RejectReason::NotMember,
    );
}

#[test]
fn d10_an_unknown_operation_is_parked_on_its_own_and_the_rest_is_folded() {
    let mut m = with_key();
    let unknown_collection = Operation::new(ObjectId([1; 16]), "later".into(), None, 0, None);
    let mut unknown_field = set_title(1);
    unknown_field.field = Some("due".into());
    let mut unknown_key = set_title(1);
    assert!(unknown_key.insert_extension(9, Value::Unsigned(1)));
    let create = Operation::new(ObjectId([2; 16]), COLLECTION.into(), None, 0, None);
    let change = m.receive(&sealed(
        FOUNDER,
        2,
        0,
        vec![
            set_title(1),
            unknown_collection,
            unknown_field,
            unknown_key,
            create,
        ],
    ));

    let parked = |position, reason| ParkedOperation { position, reason };
    assert_eq!(
        m.state(&change),
        State::Folded {
            parked: vec![
                parked(1, OperationParkReason::UnknownCollection),
                parked(2, OperationParkReason::UnknownField),
                parked(3, OperationParkReason::UnknownKey),
            ]
        }
    );
    let ids = [change.operation_id(0), change.operation_id(4)];
    assert_eq!(m.fold.folded, ids.into_iter().collect());
}

#[test]
fn d11_a_change_without_its_key_is_folded_when_the_envelope_arrives() {
    let mut m = Member::default();
    let change = m.receive(&sealed(FOUNDER, 1, 0, vec![set_title(1)]));
    assert_eq!(m.state(&change), State::Rejected(RejectReason::NoKey));
    m.receive(&envelope(FOUNDER, 2, 0));
    assert_eq!(m.state(&change), folded());
    assert_eq!(m.fold.folded, [change.operation_id(0)].into_iter().collect());
}

#[test]
fn d11_a_change_of_an_unknown_author_is_folded_when_its_membership_arrives() {
    let mut m = with_key();
    let change = m.receive(&sealed(5, 1, 0, vec![set_title(1)]));
    assert_eq!(m.state(&change), State::Rejected(RejectReason::UnknownAuthor));
    m.receive(&membership(FOUNDER, 2, 5, 0));
    assert_eq!(m.state(&change), folded());
}

#[test]
fn d11_the_check_again_takes_the_reason_of_the_step_it_now_stops_at() {
    let mut m = with_key();
    let change = m.receive(&sealed(5, 1, 0, vec![set_title(1)]));
    m.receive(&membership(FOUNDER, 2, 5, 1));
    assert_eq!(m.state(&change), State::Rejected(RejectReason::NotMember));
}

#[test]
fn d11_a_control_change_that_now_passes_lets_the_changes_waiting_for_it_pass() {
    // Member 5 sends the key envelope of generation 0 before the founder's
    // entry that makes 5 a member arrives.
    let mut m = Member::default();
    let content = m.receive(&sealed(5, 2, 0, vec![set_title(1)]));
    let key = m.receive(&envelope(5, 1, 0));
    assert_eq!(m.state(&key), State::Rejected(RejectReason::UnknownAuthor));
    m.receive(&membership(FOUNDER, 1, 5, 0));
    assert_eq!(m.state(&key), folded());
    assert_eq!(m.state(&content), folded(), "in the same call");
}

#[test]
fn d11_a_parked_change_waits_for_an_upgrade_not_for_a_control_change() {
    let mut m = Member::default();
    let mut header = sealed_header(FOUNDER, 1, 0, payload(vec![set_title(1)]));
    header.suite = 2;
    let change = m.receive(&frame(header, GOOD));
    m.receive(&envelope(FOUNDER, 2, 0));
    assert_eq!(m.state(&change), State::Parked(ParkReason::UnknownSuite));
}
