//! Creating and deleting an object (RFC 0004 Amendment 1, "Creating and
//! deleting an object"): create (operation 0) and delete (operation 4)
//! encode to exactly the bytes of examples E12 to E14 and decode back to
//! the same operations; an object operation that does not decode says why
//! it parks, as field operations do (`field_op_park.rs`).

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod examples;

use ncdb_core::cbor::{Key, Value};
use ncdb_core::change::Operation;
use ncdb_core::field_op::{self, CREATE, DELETE};
use ncdb_core::log::OperationParkReason::{
    self, MalformedArguments, UnknownArgument, UnknownOperation,
};
use ncdb_core::object::{Create, ObjectOperation};

use examples::*;

/// `operation` encodes to the bytes of `example`, and those bytes decode
/// back to `operation` and to `expected`.
fn check(operation: Operation, expected: ObjectOperation, example: &str) {
    assert_eq!(to_hex(&operation.encode()), to_hex(&hex(example)));
    let decoded = Operation::decode(&hex(example)).unwrap();
    assert_eq!(decoded, operation);
    assert_eq!(field_op::decode_object(&decoded, origin()), Ok(expected));
    assert_eq!(to_hex(&decoded.encode()), to_hex(&hex(example)));
}

#[test]
fn the_object_operations_are_numbers_0_and_4() {
    assert_eq!((CREATE, DELETE), (0, 4));
    assert_eq!(CREATE, Operation::CREATE);
}

#[test]
fn e12_creating_an_object() {
    let origin = origin();
    let create = Create {
        id: origin.id,
        author: origin.author,
        stamp: origin.stamp,
        collection: "tasks".to_owned(),
    };
    check(
        field_op::create(T1, "tasks"),
        ObjectOperation::Create(create),
        E12,
    );
}

#[test]
fn e13_deleting_an_object() {
    let delete = write(Value::Bool(true), &[]);
    check(
        field_op::delete(T1, "tasks", &delete),
        ObjectOperation::Delete(delete),
        E13,
    );
}

#[test]
fn e14_restoring_an_object() {
    let restore = write(Value::Bool(false), &[id(A, 3)]);
    check(
        field_op::delete(T1, "tasks", &restore),
        ObjectOperation::Delete(restore),
        E14,
    );
}

/// An operation on `T1` in `tasks` without a field.
fn op(number: u64, arguments: Option<Value>) -> Operation {
    Operation::new(T1, "tasks".to_owned(), None, number, arguments)
}

fn args(entries: &[(u64, Value)]) -> Value {
    Value::Map(
        entries
            .iter()
            .map(|(key, value)| (Key::Unsigned(*key), value.clone()))
            .collect(),
    )
}

fn empty() -> Value {
    Value::Array(Vec::new())
}

fn parks(number: u64, arguments: Option<Value>) -> OperationParkReason {
    field_op::decode_object(&op(number, arguments), origin()).unwrap_err()
}

#[test]
fn a_number_other_than_create_or_delete_is_an_unknown_operation() {
    let set = args(&[(1, Value::Bool(true)), (2, empty())]);
    for number in [1, 2, 3, 5, 9, 63, 64] {
        assert_eq!(parks(number, Some(set.clone())), UnknownOperation);
        assert_eq!(parks(number, None), UnknownOperation);
    }
}

#[test]
fn a_create_with_arguments_is_malformed() {
    // Keys 3 and 5 are absent from a create (RFC 0004 Amendment 1,
    // "Operation numbers"), so it has exactly one encoding.
    assert_eq!(parks(CREATE, Some(args(&[]))), MalformedArguments);
    assert_eq!(parks(CREATE, Some(Value::Null)), MalformedArguments);
}

#[test]
fn a_delete_without_its_arguments_is_malformed() {
    assert_eq!(parks(DELETE, None), MalformedArguments);
    assert_eq!(parks(DELETE, Some(Value::Bool(true))), MalformedArguments);
    assert_eq!(
        parks(DELETE, Some(args(&[(1, Value::Bool(true))]))),
        MalformedArguments
    );
    assert_eq!(parks(DELETE, Some(args(&[(2, empty())]))), MalformedArguments);
    assert_eq!(
        parks(DELETE, Some(args(&[(1, Value::Bool(true)), (2, Value::Null)]))),
        MalformedArguments
    );
}

#[test]
fn a_delete_with_key_3_is_malformed() {
    let delete = args(&[(1, Value::Bool(true)), (2, empty()), (3, empty())]);
    assert_eq!(parks(DELETE, Some(delete)), MalformedArguments);
}

#[test]
fn a_delete_with_an_argument_from_4_to_63_is_an_unknown_argument() {
    for key in [4, 63] {
        let delete = args(&[(1, Value::Bool(true)), (2, empty()), (key, Value::Null)]);
        assert_eq!(parks(DELETE, Some(delete)), UnknownArgument);
    }
}

#[test]
fn a_delete_reads_an_argument_from_64_up_as_if_it_were_absent() {
    let delete = args(&[(1, Value::Bool(true)), (2, empty()), (64, text("hint"))]);
    assert_eq!(
        field_op::decode_object(&op(DELETE, Some(delete)), origin()),
        Ok(ObjectOperation::Delete(write(Value::Bool(true), &[])))
    );
}

#[test]
fn a_delete_of_the_wrong_kind_is_read_not_parked() {
    // RFC 0004 Amendment 1: a value other than `true` reads as not
    // deleted; the value is applied, not parked.
    let delete = args(&[(1, text("yes")), (2, empty())]);
    assert_eq!(
        field_op::decode_object(&op(DELETE, Some(delete)), origin()),
        Ok(ObjectOperation::Delete(write(text("yes"), &[])))
    );
}
