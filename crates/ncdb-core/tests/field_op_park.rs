//! Field operations a member does not understand (RFC 0004 Amendment 1,
//! "Operations a member does not understand"): an operation number the
//! field's type does not define parks with "unknown operation" (E10), an
//! unknown "must understand" argument with "unknown argument", malformed
//! arguments with "malformed arguments" (decision 12); a "may ignore"
//! argument is read as if it were absent (E11).

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod examples;

use ncdb_core::cbor::{Key, Value};
use ncdb_core::change::Operation;
use ncdb_core::field_op::Encoding;
use ncdb_core::log::OperationParkReason::{
    self, MalformedArguments, UnknownArgument, UnknownOperation,
};

use examples::*;

/// An operation on `field` of `T1` with `number` and `arguments`.
fn op(field: &str, number: u64, arguments: Option<Value>) -> Operation {
    Operation::new(
        T1,
        "tasks".to_owned(),
        Some(field.to_owned()),
        number,
        arguments,
    )
}

/// An arguments map with unsigned keys.
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

fn item(change: Value, position: Value) -> Value {
    Value::Array(vec![change, position])
}

fn vec_of(item: Value) -> Value {
    Value::Array(vec![item])
}

fn a0() -> Value {
    item(Value::Bytes(A.0.to_vec()), Value::Unsigned(0))
}

/// Why `title` parks a set with `arguments`.
fn set_parks(arguments: Value) -> OperationParkReason {
    set_parks_with(1, arguments)
}

/// Why `title` parks an operation `number` with `arguments`.
fn set_parks_with(number: u64, arguments: Value) -> OperationParkReason {
    title()
        .decode(&op("title", number, Some(arguments)), origin())
        .unwrap_err()
}

/// Why `tags` parks an operation `number` with `arguments`.
fn edit_parks(number: u64, arguments: Value) -> OperationParkReason {
    tags()
        .decode(&op("tags", number, Some(arguments)), origin())
        .unwrap_err()
}

#[test]
fn e10_an_operation_number_the_type_does_not_define_is_an_unknown_operation() {
    let e10 = operation(E10);
    assert_eq!(e10.op, 9);
    assert_eq!(title().decode(&e10, origin()), Err(UnknownOperation));
}

#[test]
fn a_number_of_another_built_in_type_is_an_unknown_operation_not_misread() {
    let element = args(&[(1, text("x")), (2, empty())]);
    let removal = args(&[(1, text("x")), (3, empty())]);
    // remove (3) and add (2) on a basic value, set (1) on a set.
    assert_eq!(set_parks_with(3, removal.clone()), UnknownOperation);
    assert_eq!(set_parks_with(2, element.clone()), UnknownOperation);
    assert_eq!(edit_parks(1, element.clone()), UnknownOperation);
    // create (0) and delete (4) belong to the object; 64 to plugin types.
    for number in [0, 4, 5, 63, 64] {
        assert_eq!(set_parks_with(number, element.clone()), UnknownOperation);
        assert_eq!(edit_parks(number, element.clone()), UnknownOperation);
        let link = parent().decode(&op("parent", number, Some(element.clone())), origin());
        assert_eq!(link, Err(UnknownOperation));
        let links = depends_on().decode(&op("dependsOn", number, Some(removal.clone())), origin());
        assert_eq!(links, Err(UnknownOperation));
    }
}

#[test]
fn an_unknown_operation_parks_even_when_its_arguments_are_malformed() {
    assert_eq!(
        title().decode(&op("title", 9, None), origin()),
        Err(UnknownOperation)
    );
}

#[test]
fn e11_a_may_ignore_argument_is_read_exactly_as_e1() {
    let e1 = title().decode(&operation(E1), origin());
    assert_eq!(title().decode(&operation(E11), origin()), e1);
    assert_eq!(e1, Ok(write(text("Ship v1"), &[])));
}

#[test]
fn every_argument_from_64_up_may_be_ignored_by_every_operation() {
    let hint = |mut entries: Vec<(u64, Value)>| {
        entries.push((64, text("hint")));
        entries.push((u64::MAX, Value::Null));
        args(&entries)
    };
    let add = hint(vec![(1, text("x")), (2, empty())]);
    let remove = hint(vec![(1, text("x")), (3, vec_of(a0()))]);
    assert!(tags().decode(&op("tags", 2, Some(add)), origin()).is_ok());
    assert!(
        tags()
            .decode(&op("tags", 3, Some(remove)), origin())
            .is_ok()
    );
}

#[test]
fn an_argument_from_4_to_63_is_an_unknown_argument() {
    for key in [4, 5, 63] {
        let set = args(&[(1, text("x")), (2, empty()), (key, Value::Null)]);
        assert_eq!(set_parks(set), UnknownArgument);
        let remove = args(&[(1, text("x")), (3, empty()), (key, Value::Null)]);
        assert_eq!(edit_parks(3, remove), UnknownArgument);
    }
}

#[test]
fn an_unknown_argument_parks_as_unknown_even_when_the_rest_is_malformed() {
    // An upgrade may define key 4 and make the rest well formed; a
    // malformed operation would stay parked on every version.
    assert_eq!(set_parks(args(&[(4, Value::Null)])), UnknownArgument);
}

#[test]
fn missing_or_non_map_arguments_are_malformed() {
    let parks = title().decode(&op("title", 1, None), origin());
    assert_eq!(parks, Err(MalformedArguments));
    assert_eq!(set_parks(vec_of(text("x"))), MalformedArguments);
    assert_eq!(set_parks(text("x")), MalformedArguments);
}

#[test]
fn a_missing_required_argument_is_malformed() {
    // set needs 1 and 2, add 1 and 2, remove 1 and 3.
    assert_eq!(set_parks(args(&[(1, text("x"))])), MalformedArguments);
    assert_eq!(set_parks(args(&[(2, empty())])), MalformedArguments);
    assert_eq!(set_parks(args(&[])), MalformedArguments);
    assert_eq!(edit_parks(2, args(&[(1, text("x"))])), MalformedArguments);
    assert_eq!(edit_parks(2, args(&[(2, empty())])), MalformedArguments);
    assert_eq!(edit_parks(3, args(&[(1, text("x"))])), MalformedArguments);
    assert_eq!(edit_parks(3, args(&[(3, empty())])), MalformedArguments);
}

#[test]
fn an_argument_of_another_operation_is_malformed() {
    // observed on a set or an add, replaces on a remove.
    let set = args(&[(1, text("x")), (2, empty()), (3, empty())]);
    assert_eq!(set_parks(set), MalformedArguments);
    let add = args(&[(1, text("x")), (3, empty())]);
    assert_eq!(edit_parks(2, add), MalformedArguments);
    let remove = args(&[(1, text("x")), (2, empty()), (3, empty())]);
    assert_eq!(edit_parks(3, remove), MalformedArguments);
    // Key 0 is no argument of any operation.
    let zero = args(&[(0, Value::Null), (1, text("x")), (2, empty())]);
    assert_eq!(set_parks(zero), MalformedArguments);
}

#[test]
fn a_text_argument_key_is_malformed() {
    let mut entries = match args(&[(1, text("x")), (2, empty())]) {
        Value::Map(entries) => entries,
        _ => unreachable!(),
    };
    entries.insert(Key::Text("hint".to_owned()), Value::Null);
    assert_eq!(set_parks(Value::Map(entries)), MalformedArguments);
}

#[test]
fn a_list_that_is_not_an_array_is_malformed() {
    assert_eq!(
        set_parks(args(&[(1, text("x")), (2, Value::Null)])),
        MalformedArguments
    );
    assert_eq!(
        set_parks(args(&[(1, text("x")), (2, a0())])),
        MalformedArguments
    );
    let remove = args(&[(1, text("x")), (3, Value::Map(Default::default()))]);
    assert_eq!(edit_parks(3, remove), MalformedArguments);
}

#[test]
fn an_item_that_is_not_a_32_byte_change_id_and_a_position_is_malformed() {
    let a = || Value::Bytes(A.0.to_vec());
    let bad_items = [
        // Not an array.
        a(),
        Value::Unsigned(0),
        // Not two items.
        Value::Array(vec![a()]),
        Value::Array(vec![a(), Value::Unsigned(0), Value::Unsigned(0)]),
        Value::Array(Vec::new()),
        // A change id of the wrong length or kind.
        item(Value::Bytes(vec![0xaa; 31]), Value::Unsigned(0)),
        item(Value::Bytes(vec![0xaa; 33]), Value::Unsigned(0)),
        item(text("aa"), Value::Unsigned(0)),
        // A position that is not an unsigned integer.
        item(a(), Value::Negative(0)),
        item(a(), Value::Float(0.0)),
        item(a(), Value::Null),
        // The two the other way round.
        item(Value::Unsigned(0), a()),
    ];
    for bad in bad_items {
        let list = Value::Array(vec![a0(), bad]);
        let set = args(&[(1, text("x")), (2, list.clone())]);
        assert_eq!(set_parks(set), MalformedArguments);
        let remove = args(&[(1, text("x")), (3, list)]);
        assert_eq!(edit_parks(3, remove), MalformedArguments);
    }
}

#[test]
fn a_value_outside_the_fields_kind_is_read_not_parked() {
    // RFC 0004 Amendment 1, "The value": the field type decides how the
    // read shows it.
    let set = args(&[(1, Value::Unsigned(3)), (2, empty())]);
    let read = title().decode(&op("title", 1, Some(set)), origin());
    assert_eq!(read, Ok(write(Value::Unsigned(3), &[])));
    let short = args(&[(1, Value::Bytes(vec![0x22; 3])), (2, empty())]);
    let read = depends_on().decode(&op("dependsOn", 2, Some(short)), origin());
    assert_eq!(read, Ok(add(Value::Bytes(vec![0x22; 3]), &[])));
}
