//! Moving an object to another space (RFC 0016, "Formats"): the move is
//! operation 6 on the object, with no field, and the arguments of a
//! delete, `{1: destination, 2: replaces}`. The destination is the new
//! space's 16-byte id, or `null` to take a move back. Operation 5 belongs
//! to RFC 0013, so the move takes 6 (NOC-474).

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod examples;

use ncdb_core::cbor::{Key, Value};
use ncdb_core::change::Operation;
use ncdb_core::field_op::{self, MOVE};
use ncdb_core::log::OperationParkReason::{self, MalformedArguments, UnknownArgument};
use ncdb_core::object::{MOVED_FROM, ObjectOperation};

use examples::*;

/// The new space's id.
const NEW: [u8; 16] = [0x5a; 16];

fn args(entries: &[(u64, Value)]) -> Value {
    Value::Map(
        entries
            .iter()
            .map(|(key, value)| (Key::Unsigned(*key), value.clone()))
            .collect(),
    )
}

fn op(arguments: Option<Value>) -> Operation {
    Operation::new(T1, "tasks".to_owned(), None, MOVE, arguments)
}

fn parks(arguments: Option<Value>) -> OperationParkReason {
    field_op::decode_object(&op(arguments), origin()).unwrap_err()
}

#[test]
fn the_move_is_operation_6() {
    assert_eq!(MOVE, 6);
}

#[test]
fn a_move_encodes_as_an_operation_on_the_object_and_decodes_back() {
    let moved = write(Value::Bytes(NEW.to_vec()), &[id(A, 3)]);
    let operation = field_op::move_object(T1, "tasks", &moved);
    assert_eq!(
        operation,
        op(Some(args(&[
            (1, Value::Bytes(NEW.to_vec())),
            (
                2,
                Value::Array(vec![Value::Array(vec![
                    Value::Bytes(A.0.to_vec()),
                    Value::Unsigned(3)
                ])])
            ),
        ])))
    );
    assert_eq!(operation.field, None);
    let decoded = Operation::decode(&operation.encode()).unwrap();
    assert_eq!(decoded, operation);
    assert_eq!(
        field_op::decode_object(&decoded, origin()),
        Ok(ObjectOperation::Move(moved))
    );
}

#[test]
fn a_move_taken_back_writes_null() {
    let back = write(Value::Null, &[id(A, 0)]);
    assert_eq!(
        field_op::decode_object(&field_op::move_object(T1, "tasks", &back), origin()),
        Ok(ObjectOperation::Move(back))
    );
}

#[test]
fn a_move_without_its_arguments_is_malformed() {
    assert_eq!(parks(None), MalformedArguments);
    assert_eq!(parks(Some(Value::Null)), MalformedArguments);
    assert_eq!(
        parks(Some(args(&[(1, Value::Bytes(NEW.to_vec()))]))),
        MalformedArguments
    );
    assert_eq!(
        parks(Some(args(&[(2, Value::Array(vec![]))]))),
        MalformedArguments
    );
}

#[test]
fn a_move_with_an_argument_from_4_to_63_is_an_unknown_argument() {
    let moved = args(&[
        (1, Value::Bytes(NEW.to_vec())),
        (2, Value::Array(vec![])),
        (4, Value::Null),
    ]);
    assert_eq!(parks(Some(moved)), UnknownArgument);
}

#[test]
fn a_destination_of_the_wrong_kind_is_read_not_parked() {
    // As for a delete: the value is applied, and reads as not moved.
    let moved = args(&[(1, Value::Text("x".into())), (2, Value::Array(vec![]))]);
    assert_eq!(
        field_op::decode_object(&op(Some(moved)), origin()),
        Ok(ObjectOperation::Move(write(Value::Text("x".into()), &[])))
    );
}

/// The old space's id.
const OLD: [u8; 16] = [0x0d; 16];

/// `{1: old space id}`, the value of `$movedFrom`.
fn from_old() -> Value {
    args(&[(1, Value::Bytes(OLD.to_vec()))])
}

#[test]
fn moved_from_is_a_set_on_the_hidden_field_and_folds_into_the_object() {
    // RFC 0016, "Formats": `$movedFrom` is a hidden field set in the new
    // space's change, `{1: old space id}`, merged as a basic value.
    assert_eq!(MOVED_FROM, "$movedFrom");
    let set = write(from_old(), &[]);
    let operation = field_op::moved_from(T1, "tasks", &set);
    assert_eq!(operation.field.as_deref(), Some(MOVED_FROM));
    assert_eq!(operation.op, field_op::SET);
    assert_eq!(
        field_op::decode_moved_from(&Operation::decode(&operation.encode()).unwrap(), origin()),
        Ok(ObjectOperation::MovedFrom(set))
    );
}

#[test]
fn moved_from_takes_only_a_set() {
    let mut operation = field_op::moved_from(T1, "tasks", &write(from_old(), &[]));
    operation.op = field_op::ADD;
    assert_eq!(
        field_op::decode_moved_from(&operation, origin()),
        Err(OperationParkReason::UnknownOperation)
    );
}
