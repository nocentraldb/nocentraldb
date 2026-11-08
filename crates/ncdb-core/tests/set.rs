//! Sets (RFC 0004, "Sets"): add wins over a concurrent removal; a removal
//! names the tags of the additions its author had observed; a removal that
//! lost to an addition its author had not seen stays readable. The contract
//! C1 to C7 is checked in `set_contract.rs`.

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::BTreeSet;

use ncdb_core::basic::{Kind, OutsideSchema};
use ncdb_core::cbor::Value;
use ncdb_core::clock::Hlc;
use ncdb_core::field::{FieldType, Outcome};
use ncdb_core::ids::{ChangeId, MemberId, OperationId};
use ncdb_core::set::{Action, Edit, Read, Removal, Set, State};

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

fn ids(ids: &[OperationId]) -> BTreeSet<OperationId> {
    ids.iter().copied().collect()
}

fn add(id: OperationId, author: u8, stamp: u64, element: Value, replaces: &[OperationId]) -> Edit {
    Edit {
        id,
        author: member(author),
        stamp: Hlc(stamp),
        element,
        action: Action::Add {
            replaces: ids(replaces),
        },
    }
}

fn remove(
    id: OperationId,
    author: u8,
    stamp: u64,
    element: Value,
    observed: &[OperationId],
) -> Edit {
    Edit {
        id,
        author: member(author),
        stamp: Hlc(stamp),
        element,
        action: Action::Remove {
            observed: ids(observed),
        },
    }
}

const TAGS: Set = Set { kind: Kind::Text };

fn fold(field: &Set, edits: &[&Edit]) -> State {
    let mut state = State::default();
    for e in edits {
        field.apply(&mut state, e);
    }
    state
}

/// Every order of `n` items, as index permutations.
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

/// Folds `edits` in every order, asserts every copy has the same state
/// bytes, reads and outcomes, and returns them.
fn read_in_every_order(field: &Set, edits: &[Edit]) -> (State, Read) {
    let mut result: Option<(State, Read)> = None;
    for order in orders(edits.len()) {
        let ordered: Vec<&Edit> = order.iter().map(|&i| &edits[i]).collect();
        let state = fold(field, &ordered);
        let read = field.read(&state);
        if let Some((first, first_read)) = &result {
            assert_eq!(
                field.state_bytes(&state),
                field.state_bytes(first),
                "order {order:?}"
            );
            assert_eq!(&read, first_read, "order {order:?}");
            for e in edits {
                assert_eq!(field.outcome(&state, e.id), field.outcome(first, e.id));
            }
        } else {
            result = Some((state, read));
        }
    }
    result.expect("at least one order")
}

fn elements(read: &Read) -> Vec<Value> {
    read.elements.iter().map(|e| e.value.clone()).collect()
}

#[test]
fn a_set_nobody_wrote_is_empty() {
    assert!(TAGS.read(&State::default()).elements.is_empty());
    assert_eq!(TAGS.outcome(&State::default(), op(1)), None);
}

#[test]
fn elements_read_in_ascending_order_of_their_canonical_bytes() {
    // "b" and "a" encode as one length byte and the text; "aa" is longer,
    // so its length byte sorts it last.
    let edits = [
        add(op(1), 1, 10, text("aa"), &[]),
        add(op(2), 2, 10, text("b"), &[]),
        add(op(3), 3, 10, text("a"), &[]),
    ];
    let (state, read) = read_in_every_order(&TAGS, &edits);
    assert_eq!(elements(&read), vec![text("a"), text("b"), text("aa")]);
    assert!(
        read.elements
            .iter()
            .all(|e| e.known && e.lost_removals.is_empty())
    );
    assert_eq!(TAGS.outcome(&state, op(1)), Some(Outcome::Visible));
}

#[test]
fn two_additions_of_one_element_are_one_element() {
    let edits = [
        add(op(1), 1, 10, text("urgent"), &[]),
        add(op(2), 2, 20, text("urgent"), &[]),
    ];
    let (state, read) = read_in_every_order(&TAGS, &edits);
    assert_eq!(elements(&read), vec![text("urgent")]);
    assert_eq!(TAGS.outcome(&state, op(1)), Some(Outcome::Visible));
    assert_eq!(TAGS.outcome(&state, op(2)), Some(Outcome::Visible));
}

#[test]
fn a_removal_that_observed_the_addition_removes_the_element() {
    let edits = [
        add(op(1), 1, 10, text("urgent"), &[]),
        remove(op(2), 2, 20, text("urgent"), &[op(1)]),
    ];
    let (state, read) = read_in_every_order(&TAGS, &edits);
    assert!(read.elements.is_empty());
    assert_eq!(TAGS.outcome(&state, op(1)), Some(Outcome::Replaced(op(2))));
    assert_eq!(TAGS.outcome(&state, op(2)), Some(Outcome::Visible));
}

#[test]
fn a_concurrent_addition_wins_over_a_removal_and_the_removal_stays_readable() {
    // Alice removes "urgent", having seen Bob's first addition. Offline,
    // Carol adds "urgent" again; Alice never saw that addition.
    let edits = [
        add(op(1), 2, 10, text("urgent"), &[]),
        remove(op(2), 1, 20, text("urgent"), &[op(1)]),
        add(op(3), 3, 15, text("urgent"), &[]),
    ];
    let (state, read) = read_in_every_order(&TAGS, &edits);
    assert_eq!(elements(&read), vec![text("urgent")]);
    assert_eq!(
        read.elements[0].lost_removals,
        vec![Removal {
            id: op(2),
            author: member(1),
            stamp: Hlc(20),
        }]
    );
    assert_eq!(TAGS.outcome(&state, op(2)), Some(Outcome::Lost));
    assert_eq!(TAGS.outcome(&state, op(3)), Some(Outcome::Visible));
    assert_eq!(TAGS.outcome(&state, op(1)), Some(Outcome::Replaced(op(2))));
}

#[test]
fn a_removal_wins_over_an_addition_its_author_had_seen() {
    // Concurrency decides, not stamps: the removal has the lower stamp but
    // observed the addition, so the element goes.
    let edits = [
        add(op(1), 1, 30, text("urgent"), &[]),
        remove(op(2), 2, 10, text("urgent"), &[op(1)]),
    ];
    let (state, read) = read_in_every_order(&TAGS, &edits);
    assert!(read.elements.is_empty());
    assert_eq!(TAGS.outcome(&state, op(2)), Some(Outcome::Visible));
}

#[test]
fn a_deliberate_re_add_replaces_the_removal_it_had_seen() {
    let edits = [
        add(op(1), 1, 10, text("urgent"), &[]),
        remove(op(2), 2, 20, text("urgent"), &[op(1)]),
        add(op(3), 3, 30, text("urgent"), &[op(2)]),
    ];
    let (state, read) = read_in_every_order(&TAGS, &edits);
    assert_eq!(elements(&read), vec![text("urgent")]);
    assert!(read.elements[0].lost_removals.is_empty());
    assert_eq!(TAGS.outcome(&state, op(2)), Some(Outcome::Replaced(op(3))));
}

#[test]
fn a_lost_removal_ends_when_someone_removes_again_having_seen_the_addition() {
    let edits = [
        add(op(1), 1, 10, text("urgent"), &[]),
        remove(op(2), 2, 20, text("urgent"), &[op(1)]),
        add(op(3), 3, 15, text("urgent"), &[]),
        remove(op(4), 1, 40, text("urgent"), &[op(3)]),
    ];
    let (state, read) = read_in_every_order(&TAGS, &edits);
    assert!(read.elements.is_empty());
    assert_eq!(TAGS.outcome(&state, op(2)), Some(Outcome::Visible));
    assert_eq!(TAGS.outcome(&state, op(3)), Some(Outcome::Replaced(op(4))));
}

#[test]
fn a_removal_that_arrives_before_its_addition_still_removes_it() {
    let added = add(op(1), 1, 10, text("x"), &[]);
    let removed = remove(op(2), 2, 20, text("x"), &[op(1)]);
    let state = fold(&TAGS, &[&removed, &added]);
    assert!(TAGS.read(&state).elements.is_empty());
}

#[test]
fn applying_an_operation_twice_changes_nothing() {
    let a = add(op(1), 1, 10, text("x"), &[]);
    let r = remove(op(2), 2, 20, text("x"), &[op(1)]);
    assert_eq!(fold(&TAGS, &[&a, &r, &a, &r]), fold(&TAGS, &[&a, &r]));
}

#[test]
fn every_nan_is_one_element_and_the_two_zeros_are_two() {
    let field = Set { kind: Kind::Float };
    let other_nan = f64::from_bits(f64::NAN.to_bits() ^ 1);
    let edits = [
        add(op(1), 1, 10, Value::Float(f64::NAN), &[]),
        add(op(2), 2, 10, Value::Float(other_nan), &[]),
        add(op(3), 3, 10, Value::Float(0.0), &[]),
        add(op(4), 1, 10, Value::Float(-0.0), &[]),
    ];
    let (_, read) = read_in_every_order(&field, &edits);
    assert_eq!(read.elements.len(), 3);
    // A removal of one NaN removes the other, since it is the same element.
    let mut edits = edits.to_vec();
    edits.push(remove(
        op(5),
        2,
        20,
        Value::Float(other_nan),
        &[op(1), op(2)],
    ));
    let (_, read) = read_in_every_order(&field, &edits);
    assert_eq!(read.elements.len(), 2);
}

#[test]
fn add_and_remove_name_what_the_authors_copy_holds() {
    let state = fold(
        &TAGS,
        &[
            &add(op(1), 1, 10, text("x"), &[]),
            &add(op(2), 2, 10, text("x"), &[]),
            &add(op(3), 2, 10, text("y"), &[]),
        ],
    );
    let removal = TAGS
        .remove(&state, op(4), member(3), Hlc(20), text("x"))
        .expect("a text");
    assert_eq!(removal, remove(op(4), 3, 20, text("x"), &[op(1), op(2)]));

    let mut state = state;
    TAGS.apply(&mut state, &removal);
    let again = TAGS
        .add(&state, op(5), member(1), Hlc(30), text("x"))
        .expect("a text");
    assert_eq!(again, add(op(5), 1, 30, text("x"), &[op(4)]));
    let fresh = TAGS
        .add(&state, op(6), member(1), Hlc(30), text("z"))
        .expect("a text");
    assert_eq!(fresh, add(op(6), 1, 30, text("z"), &[]));
}

#[test]
fn a_re_add_does_not_name_a_removal_another_addition_already_replaced() {
    let state = fold(
        &TAGS,
        &[
            &add(op(1), 1, 10, text("x"), &[]),
            &remove(op(2), 2, 20, text("x"), &[op(1)]),
            &add(op(3), 1, 30, text("x"), &[op(2)]),
            &remove(op(4), 2, 40, text("x"), &[op(3)]),
        ],
    );
    let again = TAGS
        .add(&state, op(5), member(3), Hlc(50), text("x"))
        .expect("a text");
    assert_eq!(again, add(op(5), 3, 50, text("x"), &[op(4)]));
}

#[test]
fn add_and_remove_refuse_an_element_the_kind_does_not_allow() {
    let state = State::default();
    for element in [Value::Null, Value::Unsigned(1), Value::Bytes(vec![1])] {
        assert_eq!(
            TAGS.add(&state, op(1), member(1), Hlc(1), element.clone()),
            Err(OutsideSchema)
        );
        assert_eq!(
            TAGS.remove(&state, op(1), member(1), Hlc(1), element),
            Err(OutsideSchema)
        );
    }
    let choice = Set {
        kind: Kind::Choice(["a".to_owned()].into()),
    };
    assert_eq!(
        choice.add(&state, op(1), member(1), Hlc(1), text("b")),
        Err(OutsideSchema)
    );
    assert!(
        choice
            .add(&state, op(1), member(1), Hlc(1), text("a"))
            .is_ok()
    );
}

#[test]
fn an_element_from_a_newer_schema_is_kept_and_read_as_unknown() {
    let field = Set {
        kind: Kind::Choice(["open".to_owned()].into()),
    };
    let edits = [
        add(op(1), 1, 10, text("open"), &[]),
        add(op(2), 2, 10, text("archived"), &[]),
    ];
    let (state, read) = read_in_every_order(&field, &edits);
    let known: Vec<(Value, bool)> = read
        .elements
        .iter()
        .map(|e| (e.value.clone(), e.known))
        .collect();
    assert_eq!(known, vec![(text("open"), true), (text("archived"), false)]);
    assert_eq!(field.outcome(&state, op(2)), Some(Outcome::Visible));
}

#[test]
fn malformed_removals_and_additions_converge() {
    // A removal naming tags nobody added, an addition of another element
    // naming a removal, and a removal of "x" naming an addition of "y".
    let edits = [
        add(op(1), 1, 10, text("x"), &[op(9)]),
        add(op(2), 2, 10, text("y"), &[op(3)]),
        remove(op(3), 3, 10, text("x"), &[op(2), op(8)]),
    ];
    let (state, read) = read_in_every_order(&TAGS, &edits);
    assert_eq!(elements(&read), vec![text("x")]);
    assert_eq!(TAGS.outcome(&state, op(2)), Some(Outcome::Replaced(op(3))));
    assert_eq!(TAGS.outcome(&state, op(3)), Some(Outcome::Replaced(op(2))));
}

#[test]
fn lost_removals_read_in_stamp_order() {
    let edits = [
        add(op(1), 1, 10, text("x"), &[]),
        remove(op(2), 2, 30, text("x"), &[]),
        remove(op(3), 3, 20, text("x"), &[]),
    ];
    let (_, read) = read_in_every_order(&TAGS, &edits);
    let lost: Vec<OperationId> = read.elements[0]
        .lost_removals
        .iter()
        .map(|r| r.id)
        .collect();
    assert_eq!(lost, vec![op(3), op(2)]);
}
