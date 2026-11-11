//! Field operations in a change (RFC 0004, "How field operations travel in
//! a change"): the operations of
//! basic values, sets and links encode to exactly the bytes of examples E1
//! to E9 and decode back to the same operations. The lists of operation ids
//! are written sorted by bytes, without repeats, and `[]` when empty
//! (RFC 0004, "The arguments of each operation"). Parking is in `field_op_park.rs`.

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

mod examples;

use ncdb_core::cbor::{Key, Value};
use ncdb_core::change::Operation;
use ncdb_core::field_op::{self, ADD, Encoding, REMOVE, SET};

use examples::*;

/// `op` on `field` of `T1` in `tasks` encodes to the bytes of `example`,
/// and those bytes decode back to `op`.
fn check<T: Encoding>(field_type: &T, field: &str, op: &T::Operation, example: &str)
where
    T::Operation: PartialEq + std::fmt::Debug,
{
    let operation = field_op::encode(field_type, op, T1, "tasks", field);
    assert_eq!(to_hex(&operation.encode()), to_hex(&hex(example)));

    let decoded = Operation::decode(&hex(example)).unwrap();
    assert_eq!(decoded, operation);
    assert_eq!(field_type.decode(&decoded, origin()).as_ref(), Ok(op));
    assert_eq!(to_hex(&decoded.encode()), to_hex(&hex(example)));
}

#[test]
fn e1_the_first_write_to_a_string() {
    check(&title(), "title", &write(text("Ship v1"), &[]), E1);
}

#[test]
fn e2_an_integer_that_replaces_a_winner_and_a_conflict() {
    // Given in the order opposite to the bytes: the encoding sorts them.
    let op = write(Value::Unsigned(3), &[id(B, 1), id(A, 0)]);
    check(&priority(), "priority", &op, E2);
}

#[test]
fn e3_clearing_a_field() {
    check(&title(), "title", &write(Value::Null, &[id(A, 0)]), E3);
}

#[test]
fn e4_a_fresh_addition_to_a_set() {
    check(&tags(), "tags", &add(text("urgent"), &[]), E4);
}

#[test]
fn e5_a_removal_from_a_set() {
    check(&tags(), "tags", &remove(text("urgent"), &[id(A, 2)]), E5);
}

#[test]
fn e6_a_deliberate_re_add() {
    check(&tags(), "tags", &add(text("urgent"), &[id(B, 0)]), E6);
}

#[test]
fn e7_a_link_to_one_target() {
    check(&parent(), "parent", &write(target(T2), &[]), E7);
}

#[test]
fn e8_a_link_to_one_more_target() {
    check(&depends_on(), "dependsOn", &add(target(T2), &[]), E8);
}

#[test]
fn e9_removing_a_link_target() {
    check(
        &depends_on(),
        "dependsOn",
        &remove(target(T2), &[id(A, 1)]),
        E9,
    );
}

#[test]
fn the_operation_numbers_are_one_table() {
    assert_eq!((SET, ADD, REMOVE), (1, 2, 3));
    assert_eq!(title().arguments(&write(text("x"), &[])).0, SET);
    assert_eq!(parent().arguments(&write(target(T2), &[])).0, SET);
    assert_eq!(tags().arguments(&add(text("x"), &[])).0, ADD);
    assert_eq!(tags().arguments(&remove(text("x"), &[])).0, REMOVE);
    assert_eq!(depends_on().arguments(&add(target(T2), &[])).0, ADD);
    assert_eq!(depends_on().arguments(&remove(target(T2), &[])).0, REMOVE);
}

/// The list in argument `key` of `op`'s arguments.
fn list(arguments: &Value, key: u64) -> &Value {
    let Value::Map(entries) = arguments else {
        panic!("the arguments are a map");
    };
    &entries[&Key::Unsigned(key)]
}

fn item(change: ncdb_core::ids::ChangeId, position: u64) -> Value {
    Value::Array(vec![
        Value::Bytes(change.0.to_vec()),
        Value::Unsigned(position),
    ])
}

#[test]
fn a_list_is_written_sorted_by_bytes_change_id_first_then_position() {
    // Position 24 takes two bytes (18 18) and still sorts after 3 (03).
    let op = write(text("x"), &[id(B, 0), id(A, 24), id(A, 3)]);
    let (_, arguments) = title().arguments(&op);
    assert_eq!(
        list(&arguments, 2),
        &Value::Array(vec![item(A, 3), item(A, 24), item(B, 0)])
    );
}

#[test]
fn an_empty_list_is_written_as_the_empty_array() {
    let (_, arguments) = tags().arguments(&remove(text("x"), &[]));
    assert_eq!(list(&arguments, 3), &Value::Array(Vec::new()));
    let (_, arguments) = tags().arguments(&add(text("x"), &[]));
    assert_eq!(list(&arguments, 2), &Value::Array(Vec::new()));
}

/// E2 with its `replaces` as written by a non-canonical writer: unsorted
/// and with `[B, 1]` twice.
const E2_UNSORTED: &str = "a501501111111111111111111111111111111102657461736b7303687072696f\
    72697479040105a201030283825820bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\
    bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb01825820aaaaaaaaaaaaaaaaaaaaaaaaaa\
    aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa00825820bbbbbbbbbbbbbbbbbb\
    bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb01";

#[test]
fn a_non_canonical_list_is_read_as_a_set_and_never_written_back() {
    // A reader accepts it (RFC 0004, "The arguments of each operation"):
    // the state depends on the set of
    // ids only.
    let decoded = Operation::decode(&hex(E2_UNSORTED)).unwrap();
    let op = priority().decode(&decoded, origin()).unwrap();
    assert_eq!(op, write(Value::Unsigned(3), &[id(A, 0), id(B, 1)]));
    // The writer refuses the non-canonical form: it writes E2.
    let written = field_op::encode(&priority(), &op, T1, "tasks", "priority");
    assert_ne!(to_hex(&written.encode()), to_hex(&hex(E2_UNSORTED)));
    assert_eq!(to_hex(&written.encode()), to_hex(&hex(E2)));
}
