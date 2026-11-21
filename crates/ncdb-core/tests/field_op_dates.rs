//! A day and a point in time in a change (RFC 0004, "Days and points in
//! time"): each travels as a set (1) whose value is a plain CBOR integer,
//! and encodes to exactly the bytes of examples E18 to E20 and decodes back
//! to the same operations. A value that does not fit is kept as written;
//! add, remove and delete are not defined on either type, so they park as
//! unknown operations.

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod examples;

use ncdb_core::basic::{BasicValue, Write};
use ncdb_core::cbor::Value;
use ncdb_core::change::Operation;
use ncdb_core::field_op::{self, ADD, DELETE, Encoding, REMOVE, SET};
use ncdb_core::log::OperationParkReason;

use examples::*;

/// `op` on `field` (of `field_type`) of `T1` in `events` encodes to the
/// bytes of `example`, and those bytes decode back to `op`.
fn check(field_type: &BasicValue, field: &str, op: &Write, example: &str) {
    let operation = field_op::encode(field_type, op, T1, "events", field);
    assert_eq!(to_hex(&operation.encode()), to_hex(&hex(example)));

    let decoded = Operation::decode(&hex(example)).unwrap();
    assert_eq!(decoded, operation);
    assert_eq!(field_type.decode(&decoded, origin()).as_ref(), Ok(op));
    assert_eq!(to_hex(&decoded.encode()), to_hex(&hex(example)));
}

#[test]
fn e18_a_day() {
    check(
        &done_on(),
        "doneOn",
        &write(Value::Unsigned(20_778), &[]),
        E18,
    );
}

#[test]
fn e19_a_day_before_1970() {
    let op = write(Value::Negative(0), &[id(A, 0)]);
    check(&done_on(), "doneOn", &op, E19);
}

#[test]
fn e20_a_point_in_time() {
    let op = write(Value::Unsigned(1_795_288_124_636), &[]);
    check(&occurred_at(), "occurredAt", &op, E20);
}

#[test]
fn a_value_that_does_not_fit_is_decoded_and_kept_as_written() {
    // As every basic value's (NOC-16): applied, not parked.
    for value in [
        Value::Unsigned(2_932_897),
        Value::Text("2026-11-21".into()),
        Value::Float(20_778.0),
    ] {
        let op = write(value.clone(), &[]);
        let operation = field_op::encode(&done_on(), &op, T1, "events", "doneOn");
        assert_eq!(done_on().decode(&operation, origin()), Ok(op));
        assert!(!done_on().kind.allows(&value), "{value:?}");
    }
}

#[test]
fn set_is_the_only_operation_of_a_day_and_a_point_in_time() {
    for (field_type, field) in [(done_on(), "doneOn"), (occurred_at(), "occurredAt")] {
        let op = write(Value::Unsigned(1), &[]);
        assert_eq!(field_type.arguments(&op).0, SET);
        for number in [ADD, REMOVE, DELETE, 5, 63] {
            let mut operation = field_op::encode(&field_type, &op, T1, "events", field);
            operation.op = number;
            assert_eq!(
                field_type.decode(&operation, origin()),
                Err(OperationParkReason::UnknownOperation),
                "{field}: operation {number}"
            );
        }
    }
}
