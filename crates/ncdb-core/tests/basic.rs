//! Basic values (RFC 0004, "Basic values"): the last writer wins on read,
//! ties break by author member id and then operation id, and a write that
//! lost to a write its author had not seen stays readable as a conflict.
//! The contract C1 to C7 is checked in `basic_contract.rs`.

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::basic::{BasicValue, Entry, Kind, State, Write};
use ncdb_core::cbor::Value;
use ncdb_core::clock::Hlc;
use ncdb_core::field::{FieldType, Outcome};
use ncdb_core::ids::{ChangeId, MemberId, OperationId};

fn op(n: u8) -> OperationId {
    OperationId {
        change: ChangeId([n; 32]),
        position: 0,
    }
}

fn member(n: u8) -> MemberId {
    MemberId([n; 32])
}

fn text(s: &str) -> Value {
    Value::Text(s.to_owned())
}

fn write(id: OperationId, author: u8, stamp: u64, value: Value, replaces: &[OperationId]) -> Write {
    Write {
        id,
        author: member(author),
        stamp: Hlc(stamp),
        value,
        replaces: replaces.iter().copied().collect(),
    }
}

const TEXT: BasicValue = BasicValue { kind: Kind::Text };

/// Applies `writes` in the given order to a fresh state.
fn fold(field: &BasicValue, writes: &[&Write]) -> State {
    let mut state = State::default();
    for w in writes {
        field.apply(&mut state, w);
    }
    state
}

/// Every order of `writes`, as index permutations.
fn orders(n: usize) -> Vec<Vec<usize>> {
    if n == 0 {
        return vec![vec![]];
    }
    let mut all = Vec::new();
    for rest in orders(n - 1) {
        for at in 0..=rest.len() {
            let mut order = rest.clone();
            order.insert(at, n - 1);
            all.push(order);
        }
    }
    all
}

/// Folds `writes` in every order and asserts every copy reads the same;
/// returns that read.
fn read_in_every_order(field: &BasicValue, writes: &[Write]) -> (State, ncdb_core::basic::Read) {
    let mut result: Option<(State, ncdb_core::basic::Read)> = None;
    for order in orders(writes.len()) {
        let ordered: Vec<&Write> = order.iter().map(|&i| &writes[i]).collect();
        let state = fold(field, &ordered);
        let read = field.read(&state);
        if let Some((first, first_read)) = &result {
            assert_eq!(&state, first, "order {order:?}");
            assert_eq!(&read, first_read, "order {order:?}");
        } else {
            result = Some((state, read));
        }
    }
    result.expect("at least one order")
}

fn values(entries: &[Entry]) -> Vec<Value> {
    entries.iter().map(|e| e.value.clone()).collect()
}

#[test]
fn a_field_nobody_wrote_reads_as_unset() {
    let read = TEXT.read(&State::default());
    assert_eq!(read.value, None);
    assert!(read.conflicts.is_empty());
    assert_eq!(TEXT.outcome(&State::default(), op(1)), None);
}

#[test]
fn the_write_with_the_highest_stamp_wins() {
    let writes = [
        write(op(1), 1, 10, text("old"), &[]),
        write(op(2), 2, 20, text("new"), &[]),
    ];
    let (state, read) = read_in_every_order(&TEXT, &writes);
    let winner = read.value.expect("a winner");
    assert_eq!(winner.value, text("new"));
    assert_eq!((winner.id, winner.author, winner.stamp), (op(2), member(2), Hlc(20)));
    assert_eq!(values(&read.conflicts), vec![text("old")]);
    assert_eq!(TEXT.outcome(&state, op(2)), Some(Outcome::Visible));
    assert_eq!(TEXT.outcome(&state, op(1)), Some(Outcome::Lost));
}

#[test]
fn equal_stamps_break_by_author_member_id_before_operation_id() {
    // Member 2 has the higher member id but the lower operation id.
    let writes = [
        write(op(9), 1, 10, text("by member 1"), &[]),
        write(op(1), 2, 10, text("by member 2"), &[]),
    ];
    let (state, read) = read_in_every_order(&TEXT, &writes);
    assert_eq!(read.value.expect("a winner").value, text("by member 2"));
    assert_eq!(TEXT.outcome(&state, op(1)), Some(Outcome::Visible));
    assert_eq!(TEXT.outcome(&state, op(9)), Some(Outcome::Lost));
}

#[test]
fn equal_stamps_and_authors_break_by_operation_id() {
    let low = OperationId {
        change: ChangeId([5; 32]),
        position: 0,
    };
    let high = OperationId {
        change: ChangeId([5; 32]),
        position: 1,
    };
    let writes = [
        write(high, 1, 10, text("second"), &[]),
        write(low, 1, 10, text("first"), &[]),
    ];
    let (_, read) = read_in_every_order(&TEXT, &writes);
    assert_eq!(read.value.expect("a winner").value, text("second"));
    assert_eq!(values(&read.conflicts), vec![text("first")]);
}

#[test]
fn a_write_replaces_what_its_author_had_seen() {
    let writes = [
        write(op(1), 1, 10, text("draft"), &[]),
        write(op(2), 2, 20, text("final"), &[op(1)]),
    ];
    let (state, read) = read_in_every_order(&TEXT, &writes);
    assert_eq!(read.value.expect("a winner").value, text("final"));
    assert!(read.conflicts.is_empty());
    assert_eq!(TEXT.outcome(&state, op(1)), Some(Outcome::Replaced(op(2))));
}

#[test]
fn a_late_change_that_loses_stays_identifiable() {
    // Member 3 was offline: it had seen only write 1 and wrote at stamp 15.
    // Meanwhile member 2 replaced write 1 at stamp 20. Member 3's write
    // lost to a write its author had not seen.
    let writes = [
        write(op(1), 1, 10, text("start"), &[]),
        write(op(2), 2, 20, text("online"), &[op(1)]),
        write(op(3), 3, 15, text("offline"), &[op(1)]),
    ];
    let (state, read) = read_in_every_order(&TEXT, &writes);
    assert_eq!(read.value.expect("a winner").value, text("online"));
    let lost = read.conflicts.first().expect("the late write is a conflict");
    assert_eq!(read.conflicts.len(), 1);
    assert_eq!(
        (lost.id, lost.author, lost.stamp, lost.value.clone()),
        (op(3), member(3), Hlc(15), text("offline"))
    );
    assert_eq!(TEXT.outcome(&state, op(3)), Some(Outcome::Lost));
    assert_eq!(TEXT.outcome(&state, op(1)), Some(Outcome::Replaced(op(2))));

    // The conflict ends only when someone writes having seen it.
    let resolved = write(op(4), 1, 30, text("merged"), &[op(2), op(3)]);
    let mut writes = writes.to_vec();
    writes.push(resolved);
    let (state, read) = read_in_every_order(&TEXT, &writes);
    assert_eq!(read.value.expect("a winner").value, text("merged"));
    assert!(read.conflicts.is_empty());
    assert_eq!(TEXT.outcome(&state, op(3)), Some(Outcome::Replaced(op(4))));
}

#[test]
fn a_late_change_that_comes_last_wins_and_keeps_what_it_did_not_see() {
    let writes = [
        write(op(1), 1, 10, text("start"), &[]),
        write(op(2), 2, 20, text("online"), &[op(1)]),
        write(op(3), 3, 25, text("offline"), &[op(1)]),
    ];
    let (state, read) = read_in_every_order(&TEXT, &writes);
    assert_eq!(read.value.expect("a winner").value, text("offline"));
    assert_eq!(values(&read.conflicts), vec![text("online")]);
    assert_eq!(TEXT.outcome(&state, op(2)), Some(Outcome::Lost));
}

#[test]
fn a_write_that_arrives_after_its_replacer_never_shows_as_a_conflict() {
    let first = write(op(1), 1, 10, text("old"), &[]);
    let second = write(op(2), 2, 20, text("new"), &[op(1)]);
    let state = fold(&TEXT, &[&second, &first]);
    let read = TEXT.read(&state);
    assert!(read.conflicts.is_empty());
    assert_eq!(TEXT.outcome(&state, op(1)), Some(Outcome::Replaced(op(2))));
}

#[test]
fn applying_a_write_twice_changes_nothing() {
    let w = write(op(1), 1, 10, text("once"), &[]);
    assert_eq!(fold(&TEXT, &[&w, &w]), fold(&TEXT, &[&w]));
}

#[test]
fn a_cycle_of_malformed_writes_reads_as_unset_on_every_copy() {
    let writes = [
        write(op(1), 1, 10, text("a"), &[op(2)]),
        write(op(2), 2, 20, text("b"), &[op(1)]),
    ];
    let (state, read) = read_in_every_order(&TEXT, &writes);
    assert_eq!(read.value, None);
    assert!(read.conflicts.is_empty());
    assert_eq!(TEXT.outcome(&state, op(1)), Some(Outcome::Replaced(op(2))));
    assert_eq!(TEXT.outcome(&state, op(2)), Some(Outcome::Replaced(op(1))));
}

#[test]
fn a_conflict_with_the_winners_value_is_not_listed_and_counts_as_visible() {
    let writes = [
        write(op(1), 1, 10, text("same"), &[]),
        write(op(2), 2, 20, text("same"), &[]),
    ];
    let (state, read) = read_in_every_order(&TEXT, &writes);
    assert_eq!(read.value.expect("a winner").id, op(2));
    assert!(read.conflicts.is_empty());
    assert_eq!(TEXT.outcome(&state, op(1)), Some(Outcome::Visible));
}

#[test]
fn concurrent_integer_writes_do_not_add_up() {
    let field = BasicValue { kind: Kind::Integer };
    let writes = [
        write(op(1), 1, 10, Value::Unsigned(1), &[]),
        write(op(2), 2, 11, Value::Unsigned(1), &[op(1)]),
        write(op(3), 3, 12, Value::Unsigned(2), &[op(1)]),
    ];
    let (_, read) = read_in_every_order(&field, &writes);
    assert_eq!(read.value.expect("a winner").value, Value::Unsigned(2));
    assert_eq!(values(&read.conflicts), vec![Value::Unsigned(1)]);
}

#[test]
fn every_nan_is_one_value_and_the_two_zeros_are_two() {
    let field = BasicValue { kind: Kind::Float };
    let other_nan = f64::from_bits(f64::NAN.to_bits() ^ 1);
    let nans = [
        write(op(1), 1, 10, Value::Float(other_nan), &[]),
        write(op(2), 2, 20, Value::Float(f64::NAN), &[]),
    ];
    let (state, read) = read_in_every_order(&field, &nans);
    assert!(read.conflicts.is_empty(), "two NaNs are one value");
    assert_eq!(field.outcome(&state, op(1)), Some(Outcome::Visible));

    let zeros = [
        write(op(1), 1, 10, Value::Float(0.0), &[]),
        write(op(2), 2, 20, Value::Float(-0.0), &[]),
    ];
    let (state, read) = read_in_every_order(&field, &zeros);
    assert_eq!(values(&read.conflicts), vec![Value::Float(0.0)]);
    assert_eq!(field.outcome(&state, op(1)), Some(Outcome::Lost));
}

#[test]
fn null_clears_the_field_and_the_clear_is_a_write_like_any_other() {
    let writes = [
        write(op(1), 1, 10, text("set"), &[]),
        write(op(2), 2, 20, Value::Null, &[op(1)]),
    ];
    let (_, read) = read_in_every_order(&TEXT, &writes);
    assert_eq!(read.value.expect("the clearing write").value, Value::Null);
}
