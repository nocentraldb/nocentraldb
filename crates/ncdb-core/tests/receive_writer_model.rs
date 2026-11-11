//! Step 6 reads the writer's model in payload key 64 (RFC 0002, "The
//! writer's model", "What a member does with it"): a change without key 64
//! or of the member's own model goes on to step 7, whatever its version; a
//! change of another model, or whose key 64 has the wrong shape, is parked
//! whole as "another model". Inside step 6, the payload format is checked
//! first, then key 64, then unknown "must understand" payload keys (NOC-202).

#[path = "support/member.rs"]
mod member;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::BTreeSet;

use ncdb_core::cbor::Value;
use ncdb_core::change::{Payload, WriterModel};
use ncdb_core::clock::Hlc;
use ncdb_core::log::{ParkReason, State};

use member::*;

/// A member that holds the key of generation 0.
fn with_key() -> Member {
    let mut m = Member::default();
    m.receive(&envelope(FOUNDER, 1, 0));
    m
}

fn plain() -> Payload {
    Payload::new(Hlc(1 << 16), BTreeSet::new(), vec![set_title(1)])
}

fn of(id: [u8; 16], version: u64) -> Payload {
    let mut payload = plain();
    payload.set_writer_model(WriterModel { id, version });
    payload
}

/// The state of a change of `FOUNDER` that carries `payload`.
fn state(payload: &Payload) -> State {
    let mut m = with_key();
    let change = m.receive(&frame(sealed_header(FOUNDER, 2, 0, payload.encode()), GOOD));
    m.state(&change)
}

fn folded() -> State {
    State::Folded { parked: vec![] }
}

#[test]
fn a_change_without_key_64_is_folded() {
    assert_eq!(state(&plain()), folded());
}

#[test]
fn a_change_of_the_members_own_model_is_folded_whatever_its_version() {
    assert_eq!(state(&of(MODEL, 1)), folded());
    assert_eq!(state(&of(MODEL, 9)), folded());
}

#[test]
fn a_change_of_another_model_is_parked_whole() {
    assert_eq!(
        state(&of([0x01; 16], 1)),
        State::Parked(ParkReason::AnotherModel)
    );
}

#[test]
fn a_key_64_of_the_wrong_shape_parks_the_change_as_another_model() {
    let mut payload = plain();
    assert!(payload.insert_extension(64, Value::Unsigned(1)));

    assert_eq!(state(&payload), State::Parked(ParkReason::AnotherModel));
}

#[test]
fn another_model_comes_before_an_unknown_must_understand_payload_key() {
    let mut payload = of([0x01; 16], 1);
    assert!(payload.insert_extension(9, Value::Unsigned(1)));

    assert_eq!(state(&payload), State::Parked(ParkReason::AnotherModel));
}

#[test]
fn an_unknown_payload_format_comes_before_another_model() {
    let mut payload = of([0x01; 16], 1);
    payload.format = 2;

    assert_eq!(
        state(&payload),
        State::Parked(ParkReason::UnknownPayloadFormat)
    );
}
