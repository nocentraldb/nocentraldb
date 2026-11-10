//! A reference in a change (RFC 0004 Amendment 4, "The value and its
//! encoding"): `ncdb/ref` travels as a set (1) whose value is
//! `[space id, object id]`, and encodes to exactly the bytes of examples
//! E15 to E17 and decodes back to the same operations. Add, remove and
//! delete are not defined on it, so they park as unknown operations.

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod examples;

use ncdb_core::cbor::Value;
use ncdb_core::change::Operation;
use ncdb_core::field_op::{self, ADD, DELETE, Encoding, REMOVE, SET};
use ncdb_core::ids::ObjectId;
use ncdb_core::log::OperationParkReason;
use ncdb_core::reference;

use examples::*;

/// `op` on `customer` of `T1` in `orders` encodes to the bytes of
/// `example`, and those bytes decode back to `op`.
fn check(op: &ncdb_core::basic::Write, example: &str) {
    let operation = field_op::encode(&customer(), op, T1, "orders", "customer");
    assert_eq!(to_hex(&operation.encode()), to_hex(&hex(example)));

    let decoded = Operation::decode(&hex(example)).unwrap();
    assert_eq!(decoded, operation);
    assert_eq!(customer().decode(&decoded, origin()).as_ref(), Ok(op));
    assert_eq!(to_hex(&decoded.encode()), to_hex(&hex(example)));
}

#[test]
fn e15_the_first_write_to_a_reference() {
    check(&write(reference(C1), &[]), E15);
}

#[test]
fn e16_a_reference_that_replaces_a_winner_and_a_conflict() {
    // Given in the order opposite to the bytes: the encoding sorts them.
    check(&write(reference(C2), &[id(B, 1), id(A, 0)]), E16);
}

#[test]
fn e17_clearing_a_reference() {
    check(&write(Value::Null, &[id(A, 4)]), E17);
}

#[test]
fn the_value_is_an_array_of_two_16_byte_strings_and_takes_35_bytes() {
    let value = reference(C1);
    assert_eq!(
        value,
        Value::Array(vec![Value::Bytes(vec![0x33; 16]), Value::Bytes(vec![0x44; 16])])
    );
    assert_eq!(ncdb_core::cbor::encode(&value).len(), 35);
    let target = reference::target(&value).expect("a reference names a target");
    assert_eq!((target.space, target.object), (S, C1));
}

#[test]
fn a_value_that_does_not_fit_names_no_target() {
    let bare = Value::Bytes(C1.0.to_vec());
    let short = Value::Array(vec![Value::Bytes(vec![0x33; 16]), Value::Bytes(vec![0x44; 15])]);
    let three = Value::Array(vec![
        Value::Bytes(vec![0x33; 16]),
        Value::Bytes(vec![0x44; 16]),
        Value::Bytes(vec![0x55; 16]),
    ]);
    let text = Value::Array(vec![Value::Bytes(vec![0x33; 16]), text("C1")]);
    for value in [bare, short, three, text, Value::Null] {
        assert_eq!(reference::target(&value), None, "{value:?}");
    }
}

#[test]
fn a_value_that_does_not_fit_is_decoded_and_kept_as_written() {
    // Decision 17: a buggy or hostile writer's bare object id is applied
    // as every basic value is; it is not parked.
    let bare = write(Value::Bytes(ObjectId([0x44; 16]).0.to_vec()), &[]);
    let operation = field_op::encode(&customer(), &bare, T1, "orders", "customer");
    assert_eq!(customer().decode(&operation, origin()), Ok(bare));
}

#[test]
fn set_is_the_only_operation_of_a_reference() {
    assert_eq!(customer().arguments(&write(reference(C1), &[])).0, SET);
    for number in [ADD, REMOVE, DELETE, 5, 63] {
        let mut operation = field_op::encode(
            &customer(),
            &write(reference(C1), &[]),
            T1,
            "orders",
            "customer",
        );
        operation.op = number;
        assert_eq!(
            customer().decode(&operation, origin()),
            Err(OperationParkReason::UnknownOperation),
            "operation {number}"
        );
    }
}
