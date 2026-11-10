//! Each way `seal::verify` and `seal::open` fail maps to the step and the
//! reason RFC 0002 names ("What a member does with a change it receives";
//! NOC-48): one test per row of the
//! table in `src/checks.rs`.

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::change::ChangeError;
use ncdb_core::log::{ParkReason, RejectReason};
use ncdb_core::receive::Stop;
use ncdb_core::seal::OpenError;

fn stops(error: OpenError, step: u8, stop: Stop) {
    let mapped = Stop::from(error);
    assert_eq!(mapped, stop, "{error:?}");
    assert_eq!(mapped.step(), step, "{error:?}");
}

#[test]
fn a_change_that_is_not_sealed_parks_at_step_2_as_unknown_kind() {
    stops(
        OpenError::NotSealed(1),
        2,
        Stop::Parked(ParkReason::UnknownKind),
    );
}

#[test]
fn an_unknown_suite_parks_at_step_2() {
    stops(
        OpenError::UnknownSuite(2),
        2,
        Stop::Parked(ParkReason::UnknownSuite),
    );
}

#[test]
fn an_author_who_is_not_the_member_of_the_keys_rejects_at_step_3_as_bad_signature() {
    stops(
        OpenError::AuthorMismatch,
        3,
        Stop::Rejected(RejectReason::BadSignature),
    );
}

#[test]
fn an_invalid_author_key_rejects_at_step_3_as_bad_signature() {
    stops(
        OpenError::AuthorKey,
        3,
        Stop::Rejected(RejectReason::BadSignature),
    );
}

#[test]
fn a_signature_that_does_not_verify_rejects_at_step_3_as_bad_signature() {
    stops(
        OpenError::Signature,
        3,
        Stop::Rejected(RejectReason::BadSignature),
    );
}

#[test]
fn a_payload_that_does_not_decrypt_rejects_at_step_5_as_undecryptable() {
    stops(
        OpenError::Decryption,
        5,
        Stop::Rejected(RejectReason::Undecryptable),
    );
}

#[test]
fn a_plaintext_that_is_no_payload_parks_at_step_6_as_unknown_payload_format() {
    stops(
        OpenError::Payload(ChangeError::NotAMap),
        6,
        Stop::Parked(ParkReason::UnknownPayloadFormat),
    );
}
