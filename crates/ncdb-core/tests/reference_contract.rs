//! A reference (RFC 0004, "References") merges as a basic value: it meets
//! RFC 0004's plugin contract C1 to C7, checked by the convergence harness
//! with at least three copies, random orders of arrival, duplicate
//! deliveries and malformed writes, whose values do not fit.

mod support;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::basic::{self as core_basic, State, Write};
use ncdb_core::cbor::Value;
use ncdb_core::clock::Hlc;
use ncdb_core::field::{FieldType, Outcome};
use ncdb_core::ids::{ChangeId, MemberId, ObjectId, OperationId};
use ncdb_core::reference::{self, Target};
use support::basic::{self, Basic, OTHER_SPACE, Ref, SPACE, Schema};
use support::convergence::{self, Config};

/// The seeds every check runs with.
const SEEDS: core::ops::Range<u64> = 0..16;

#[test]
fn a_reference_meets_the_contract() {
    for seed in SEEDS {
        let config = Config {
            seed,
            ..Config::default()
        };
        let result = convergence::check::<Basic<Ref>, _>(&config, &basic::writes::<Ref>());
        assert!(result.is_ok(), "{}", result.unwrap_err());
    }
}

#[test]
fn a_reference_meets_the_contract_with_more_members_copies_and_duplicates() {
    let config = Config {
        members: 4,
        copies: 5,
        max_duplicates: 3,
        max_learned: 4,
        cases: 64,
        ..Config::default()
    };
    let result = convergence::check::<Basic<Ref>, _>(&config, &basic::writes::<Ref>());
    assert!(result.is_ok(), "{}", result.unwrap_err());
}

fn op(n: u8) -> OperationId {
    OperationId {
        change: ChangeId([n; 32]),
        position: 0,
    }
}

fn target(space: ncdb_core::ids::SpaceId, n: u8) -> Target {
    Target {
        space,
        object: ObjectId([n; 16]),
    }
}

#[test]
fn set_writes_a_target_in_any_space_and_replaces_what_the_copy_holds() {
    let field = Ref::field();
    let mut state = State::default();
    let first = field.set(
        &state,
        op(1),
        MemberId([1; 32]),
        Hlc(1),
        Some(target(SPACE, 1)),
    );
    assert_eq!(first.value, reference::value(target(SPACE, 1)));
    assert!(first.replaces.is_empty());
    field.apply(&mut state, &first);

    // Decision 19: the writer's own space is a space like any other.
    let second = field.set(
        &state,
        op(2),
        MemberId([2; 32]),
        Hlc(2),
        Some(target(OTHER_SPACE, 2)),
    );
    assert_eq!(second.replaces, [op(1)].into());
    field.apply(&mut state, &second);
    assert_eq!(field.outcome(&state, op(1)), Some(Outcome::Replaced(op(2))));

    let clear = field.set(&state, op(3), MemberId([1; 32]), Hlc(3), None);
    assert_eq!(clear.value, Value::Null);
    field.apply(&mut state, &clear);
    let read = field.read(&state);
    assert_eq!(
        read.value.map(|e| (e.value, e.known)),
        Some((Value::Null, true))
    );
}

#[test]
fn concurrent_references_read_as_the_winner_and_a_conflict() {
    let field = Ref::field();
    let write = |n: u8, stamp: u64, value: Value| Write {
        id: op(n),
        author: MemberId([n; 32]),
        stamp: Hlc(stamp),
        value,
        replaces: Default::default(),
    };
    let a = write(1, 5, reference::value(target(SPACE, 1)));
    let b = write(2, 5, reference::value(target(OTHER_SPACE, 2)));
    let (mut one, mut two) = (State::default(), State::default());
    field.apply(&mut one, &a);
    field.apply(&mut one, &b);
    field.apply(&mut two, &b);
    field.apply(&mut two, &a);
    assert_eq!(field.state_bytes(&one), field.state_bytes(&two));
    let read: core_basic::Read = field.read(&one);
    // Same stamp: the higher author member id wins (RFC 0004, "Tie-break").
    assert_eq!(read.value.unwrap().value, b.value);
    assert_eq!(read.conflicts.len(), 1);
    assert_eq!(read.conflicts[0].value, a.value);
    assert_eq!(field.outcome(&one, op(1)), Some(Outcome::Lost));
}

#[test]
fn a_value_that_does_not_fit_is_kept_as_written_and_read_as_unknown() {
    let field = Ref::field();
    let mut state = State::default();
    let bare = Value::Bytes(vec![0x44; 16]);
    field.apply(
        &mut state,
        &Write {
            id: op(1),
            author: MemberId([1; 32]),
            stamp: Hlc(1),
            value: bare.clone(),
            replaces: Default::default(),
        },
    );
    let winner = field.read(&state).value.unwrap();
    assert_eq!((winner.value, winner.known), (bare, false));
}
