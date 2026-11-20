//! RFC 0008's M-C1 and M-C2 with the convergence harness (NOC-9, NOC-19):
//! the same control changes, in any order of arrival and with duplicates,
//! resolve to the same membership state and the same verdicts on every
//! copy. Simulated members write genesis-based membership entries, key
//! chain entries, key envelopes and entries of the other types from their
//! own view of the space, so that concurrent grants, revocations, cutoffs,
//! cycles of removals, rotations and re-grants under a fresh generation
//! arise (NOC-71). Admins merge concurrent rotations once their current
//! generation is exposed, so every copy must also end in the same current
//! generation, with the same exposure (NOC-86).
//!
//! Members also equivocate (NOC-226): they write a second control change
//! or sealed change under an author sequence they already used, with
//! dependencies chosen apart from the first one's. RFC 0002 keeps and folds
//! both changes of an equivocation, so every copy must still agree on the
//! state, on every control change's verdict, on the membership verdict of
//! every sealed change. A shred a copy accepted stays accepted (M-C8).

mod support;

#[path = "support/control.rs"]
mod control;
#[path = "support/space.rs"]
mod space;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::change::Change;
use ncdb_core::ids::SpaceId;
use ncdb_core::membership::{Rejection, Resolved, Role, Verdict, resolve};
use support::convergence::{self, Config};

use control::{Copy, Members};

#[test]
fn m_c1_the_same_control_changes_resolve_to_the_same_state_in_any_order() {
    let config = Config {
        members: 3,
        max_changes_per_member: 3,
        cases: 48,
        ..Config::default()
    };
    let result = convergence::check::<Copy, _>(
        &config,
        &Members {
            forks: false,
            shreds: false,
        },
    );
    assert!(result.is_ok(), "{}", result.unwrap_err());
}

/// NOC-226: members, admins included, equivocate with control and sealed
/// changes, under dependencies chosen apart from the first change's. Every
/// copy that holds the same changes still reaches the same state, the same
/// verdicts and the same sealed verdicts.
#[test]
fn m_c1_equivocations_of_control_and_sealed_changes_converge_in_any_order() {
    let config = Config {
        members: 3,
        max_changes_per_member: 4,
        cases: 64,
        ..Config::default()
    };
    let result = convergence::check::<Copy, _>(
        &config,
        &Members {
            forks: true,
            shreds: false,
        },
    );
    assert!(result.is_ok(), "{}", result.unwrap_err());
}

/// RFC 0008's M-C8 with NOC-229: shreds meet revocations of their
/// performers that their copies did not see, cutoffs that leave them out
/// and equivocations, and no copy ever rejects a shred it accepted.
#[test]
fn m_c8_an_accepted_shred_stays_accepted_on_every_copy_whatever_arrives_next() {
    let config = Config {
        members: 3,
        max_changes_per_member: 4,
        cases: 64,
        ..Config::default()
    };
    let result = convergence::check::<Copy, _>(
        &config,
        &Members {
            forks: true,
            shreds: true,
        },
    );
    assert!(result.is_ok(), "{}", result.unwrap_err());
}

/// Every order of `items`.
fn permutations<T: Clone>(items: &[T]) -> Vec<Vec<T>> {
    if items.len() <= 1 {
        return vec![items.to_vec()];
    }
    let mut all = Vec::new();
    for i in 0..items.len() {
        let mut rest = items.to_vec();
        let first = rest.remove(i);
        for mut tail in permutations(&rest) {
            tail.insert(0, first.clone());
            all.push(tail);
        }
    }
    all
}

/// The state of `base` and then `changes`, these arriving in every order:
/// one state, or the test fails.
fn in_every_order(space: SpaceId, base: &[Change], changes: &[Change]) -> Resolved {
    let first = resolve(space, base.iter().chain(changes));
    for order in permutations(changes) {
        let state = resolve(space, base.iter().chain(&order));
        assert_eq!(state.encode(), first.encode());
    }
    first
}

#[test]
fn a_re_grant_and_revocations_end_in_the_same_membership_in_any_order() {
    const ALICE: u8 = 1;
    const BOB: u8 = 2;
    const DAN: u8 = 4;
    let mut s = space::Space::with_nonce(ALICE, [9; 16]);
    let g = s.genesis.clone();
    let first = s.first_generation;
    let bob = s.add(ALICE, BOB, Role::Admin, &[&g]);
    let dan = s.add(ALICE, DAN, Role::Writer, &[&bob]);
    let cutoff = s.sequence(DAN);
    // Alice removes Dan, rotates, re-adds him under the fresh generation
    // and wraps it to him. Bob removes Dan too: once concurrently, once
    // after he has seen the re-add.
    let remove = s.remove(ALICE, DAN, cutoff, &[&dan]);
    let (rotation, fresh) = s.rotate(ALICE, &[first], &[&remove]);
    let again = s.add_at(ALICE, DAN, Role::Writer, fresh, &[&rotation]);
    let to_dan = s.envelope_at(ALICE, DAN, fresh, &[&again]);
    let concurrent = s.remove(BOB, DAN, cutoff, &[&dan]);
    let after = s.remove(BOB, DAN, cutoff, &[&again]);
    let base = [g, bob, dan];
    let set = |extra: &[&Change]| -> Vec<Change> {
        let tail = [&remove, &rotation, &again, &to_dan];
        tail.into_iter()
            .chain(extra.iter().copied())
            .cloned()
            .collect()
    };

    let re_granted = in_every_order(s.space, &base, &set(&[]));
    assert_eq!(re_granted.verdict(again.id()), Some(&Verdict::Accepted));
    assert_eq!(re_granted.role(space::id(DAN)), Some(Role::Writer));

    let lost = in_every_order(s.space, &base, &set(&[&concurrent]));
    assert_eq!(lost.verdict(again.id()), Some(&Verdict::Lost));
    assert_eq!(lost.role(space::id(DAN)), None);

    // Bob's later removal saw the re-add and revoked it on purpose, so the
    // re-add is not lost (RFC 0008: lost only for a concurrent cap).
    let removed = in_every_order(s.space, &base, &set(&[&after]));
    assert_eq!(removed.verdict(again.id()), Some(&Verdict::Accepted));
    assert_eq!(removed.verdict(after.id()), Some(&Verdict::Accepted));
    assert_eq!(removed.role(space::id(DAN)), None);
}

#[test]
fn m_c10_shreds_and_re_wraps_end_in_one_shred_in_any_order() {
    const ALICE: u8 = 1;
    const BOB: u8 = 2;
    const CAROL: u8 = 3;
    let mut s = space::Space::with_nonce(ALICE, [9; 16]);
    let g = s.genesis.clone();
    let bob = s.add(ALICE, BOB, Role::Writer, &[&g]);
    s.certify_by(CAROL, space::other_account(1));
    let carol = s.add(ALICE, CAROL, Role::Admin, &[&bob]);
    let entry = s.person_key(BOB, 1, 1, &[&carol]);
    let by_bob = s.shred(BOB, 1, &space::other_account(1), &[&entry]);
    let by_alice = s.shred(ALICE, 1, &space::other_account(1), &[&entry]);
    let rewrap = s.person_key(ALICE, 1, 1, &[&entry]);
    let base = [g, bob, carol, entry];
    let state = in_every_order(s.space, &base, &[by_bob, by_alice.clone(), rewrap.clone()]);
    let key = state.person_key(space::person(1)).expect("the person key");
    assert_eq!(key.shred, Some(by_alice.id()));
    assert!(matches!(
        state.verdict(rewrap.id()),
        Some(Verdict::Rejected(Rejection::Shredded(_)))
    ));
}

#[test]
fn m_c10_a_shred_holds_in_any_order_when_a_concurrent_first_entry_takes_its_person_key() {
    const ALICE: u8 = 1;
    const BOB: u8 = 2;
    const CAROL: u8 = 3;
    let mut s = space::Space::with_nonce(ALICE, [9; 16]);
    let g = s.genesis.clone();
    let bob = s.add(ALICE, BOB, Role::Writer, &[&g]);
    s.certify_by(CAROL, space::other_account(1));
    let carol = s.add(ALICE, CAROL, Role::Admin, &[&bob]);
    let bobs = s.person_key(BOB, 1, 1, &[&carol]);
    let shred = s.shred(BOB, 1, &space::other_account(1), &[&bobs]);
    let alices = s.person_key(ALICE, 1, 2, &[&carol]);
    let base = [g, bob, carol];
    let state = in_every_order(s.space, &base, &[bobs, shred.clone(), alices]);
    let key = state
        .person_key(space::person(1))
        .expect("the shredded key");
    assert_eq!(key.shred, Some(shred.id()));
    assert_eq!(key.commitment, None);
}
