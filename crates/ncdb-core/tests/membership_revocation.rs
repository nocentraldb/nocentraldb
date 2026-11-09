//! Revocations in `resolve` (RFC 0008, "The function", steps 2 to 5): the
//! cutoff, concurrent grants and revocations, seniority and cycles of
//! revocations, each as RFC 0008's examples describe them.

#[path = "support/space.rs"]
mod space;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::change::Change;
use ncdb_core::membership::{self, Rejection, Role, Verdict, resolve};

use space::{Space, id};

const ALICE: u8 = 1;
const BOB: u8 = 2;
const CAROL: u8 = 3;
const DAN: u8 = 4;
const MALLORY: u8 = 5;

fn excluded_by(revocation: &Change) -> Option<&'static Verdict> {
    // A verdict that names the revocation, leaked to compare by reference.
    Some(Box::leak(Box::new(Verdict::Rejected(Rejection::Excluded(
        revocation.id(),
    )))))
}

#[test]
fn a_removal_excludes_the_subjects_changes_above_its_cutoff() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let bob = s.add(ALICE, BOB, Role::Writer, &[&g]);
    let seen = s.other(BOB, membership::ACKNOWLEDGEMENT, &[&bob]);
    let remove = s.remove(ALICE, BOB, s.sequence(BOB), &[&seen]);
    // Bob had not seen his removal when he wrote this.
    let unseen = s.other(BOB, membership::ACKNOWLEDGEMENT, &[&seen]);
    let state = resolve(s.space, [&g, &bob, &seen, &remove, &unseen]);
    assert_eq!(state.verdict(seen.id()), Some(&Verdict::Accepted));
    assert_eq!(state.verdict(remove.id()), Some(&Verdict::Accepted));
    assert_eq!(state.verdict(unseen.id()), excluded_by(&remove));
    assert_eq!(state.role(id(BOB)), None);
    let revocations = &state.member(id(BOB)).unwrap().revocations;
    assert_eq!(revocations.len(), 1);
    assert_eq!(revocations[0].cutoff, 1);
}

#[test]
fn a_junior_admins_removal_excludes_a_senior_admins_concurrent_change() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let bob = s.add(ALICE, BOB, Role::Admin, &[&g]);
    let carol = s.add(BOB, CAROL, Role::Admin, &[&bob]);
    // Carol held Bob's first change; Bob's add of Dan comes after it.
    let remove = s.remove(CAROL, BOB, 1, &[&carol]);
    let dan = s.add(BOB, DAN, Role::Writer, &[&carol]);
    let state = resolve(s.space, [&g, &bob, &carol, &remove, &dan]);
    assert_eq!(state.seniority()[..3], [id(ALICE), id(BOB), id(CAROL)]);
    assert_eq!(state.verdict(remove.id()), Some(&Verdict::Accepted));
    assert_eq!(state.verdict(dan.id()), excluded_by(&remove));
    assert_eq!(state.role(id(DAN)), None);
    assert_eq!(state.role(id(BOB)), None);
}

#[test]
fn a_demotion_keeps_the_rights_below_the_role_it_sets() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let bob = s.add(ALICE, BOB, Role::Admin, &[&g]);
    let demote = s.set_role(ALICE, BOB, Role::Writer, 0, &[&bob]);
    let person_key = s.other(BOB, membership::PERSON_KEY, &[&bob]);
    let carol = s.add(BOB, CAROL, Role::Reader, &[&person_key]);
    let state = resolve(s.space, [&g, &bob, &demote, &person_key, &carol]);
    assert_eq!(state.verdict(person_key.id()), Some(&Verdict::Accepted));
    assert_eq!(state.verdict(carol.id()), excluded_by(&demote));
    assert_eq!(state.role(id(BOB)), Some(Role::Writer));
}

#[test]
fn a_grant_that_saw_the_revocation_covers_the_changes_after_it() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let bob = s.add(ALICE, BOB, Role::Admin, &[&g]);
    let demote = s.set_role(ALICE, BOB, Role::Writer, 0, &[&bob]);
    let raise = s.set_role(ALICE, BOB, Role::Admin, 0, &[&demote]);
    // Bob wrote this before he saw the raise, and this after it.
    let early = s.add(BOB, CAROL, Role::Reader, &[&bob]);
    let late = s.add(BOB, DAN, Role::Reader, &[&raise, &early]);
    let state = resolve(s.space, [&g, &bob, &demote, &raise, &early, &late]);
    assert_eq!(state.verdict(raise.id()), Some(&Verdict::Accepted));
    assert_eq!(state.verdict(early.id()), excluded_by(&demote));
    assert_eq!(state.verdict(late.id()), Some(&Verdict::Accepted));
    assert_eq!(state.role(id(BOB)), Some(Role::Admin));
    assert_eq!(state.role(id(CAROL)), None);
    assert_eq!(state.role(id(DAN)), Some(Role::Reader));
}

#[test]
fn a_removal_wins_over_a_concurrent_grant_which_is_lost() {
    // RFC 0008, "Concurrent add and remove of one member".
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let bob = s.add(ALICE, BOB, Role::Admin, &[&g]);
    let mallory = s.add(ALICE, MALLORY, Role::Writer, &[&bob]);
    let remove = s.remove(ALICE, MALLORY, 0, &[&mallory]);
    let raise = s.set_role(BOB, MALLORY, Role::Admin, 0, &[&mallory]);
    let mallory_writes = s.add(MALLORY, DAN, Role::Reader, &[&raise]);
    for set in [
        [&g, &bob, &mallory, &remove, &raise, &mallory_writes],
        [&mallory_writes, &raise, &remove, &mallory, &bob, &g],
    ] {
        let state = resolve(s.space, set);
        assert_eq!(state.role(id(MALLORY)), None);
        assert_eq!(state.verdict(raise.id()), Some(&Verdict::Lost));
        assert_eq!(state.verdict(mallory_writes.id()), excluded_by(&remove));
        assert_eq!(state.role(id(DAN)), None);
    }
}

#[test]
fn two_concurrent_role_changes_leave_the_lower_role() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let bob = s.add(ALICE, BOB, Role::Admin, &[&g]);
    let carol = s.add(ALICE, CAROL, Role::Writer, &[&bob]);
    let to_admin = s.set_role(BOB, CAROL, Role::Admin, 0, &[&carol]);
    let to_reader = s.set_role(ALICE, CAROL, Role::Reader, 0, &[&carol]);
    let state = resolve(s.space, [&g, &bob, &carol, &to_admin, &to_reader]);
    assert_eq!(state.role(id(CAROL)), Some(Role::Reader));
    assert_eq!(state.verdict(to_admin.id()), Some(&Verdict::Lost));
    assert_eq!(state.verdict(to_reader.id()), Some(&Verdict::Accepted));
}

#[test]
fn the_senior_admin_wins_when_two_admins_remove_each_other() {
    // RFC 0008, "Two admins who remove each other".
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let bob = s.add(ALICE, BOB, Role::Admin, &[&g]);
    let alice_removes = s.remove(ALICE, BOB, 0, &[&bob]);
    let bob_removes = s.remove(BOB, ALICE, 2, &[&bob]);
    for set in [
        [&g, &bob, &alice_removes, &bob_removes],
        [&bob_removes, &alice_removes, &bob, &g],
    ] {
        let state = resolve(s.space, set);
        assert_eq!(state.verdict(alice_removes.id()), Some(&Verdict::Accepted));
        assert_eq!(state.verdict(bob_removes.id()), excluded_by(&alice_removes));
        assert_eq!(
            state.members().collect::<Vec<_>>(),
            [(id(ALICE), Role::Admin)]
        );
    }
}

#[test]
fn a_ring_of_three_removals_keeps_the_one_whose_edge_closes_the_cycle() {
    // RFC 0008: A removes B, B removes C and C removes A at once, with A
    // the most senior. A's and B's edges are added, C's is skipped.
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let bob = s.add(ALICE, BOB, Role::Admin, &[&g]);
    let carol = s.add(BOB, CAROL, Role::Admin, &[&bob]);
    let a = s.remove(ALICE, BOB, 1, &[&carol]);
    let b = s.remove(BOB, CAROL, 0, &[&carol]);
    let c = s.remove(CAROL, ALICE, 2, &[&carol]);
    let state = resolve(s.space, [&g, &bob, &carol, &a, &b, &c]);
    assert_eq!(state.seniority()[..3], [id(ALICE), id(BOB), id(CAROL)]);
    assert_eq!(state.verdict(a.id()), Some(&Verdict::Accepted));
    assert_eq!(state.verdict(b.id()), excluded_by(&a));
    assert_eq!(state.verdict(c.id()), Some(&Verdict::Accepted));
    assert_eq!(
        state.members().collect::<Vec<_>>(),
        [(id(CAROL), Role::Admin)]
    );
    let order = state.order();
    let place = |change: &Change| order.iter().position(|id| *id == change.id());
    assert!(place(&a) < place(&b) && place(&b) < place(&c));
}

#[test]
fn seniority_follows_the_depth_of_admin_grants() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let bob = s.add(ALICE, BOB, Role::Admin, &[&g]);
    let carol = s.add(BOB, CAROL, Role::Admin, &[&bob]);
    let dan = s.add(ALICE, DAN, Role::Admin, &[&carol]);
    let mallory = s.add(ALICE, MALLORY, Role::Writer, &[&dan]);
    let state = resolve(s.space, [&g, &bob, &carol, &dan, &mallory]);
    // Bob and Dan have depth 1, by change id; Carol depth 2; Mallory no
    // admin grant.
    let (first, second) = if bob.id() < dan.id() {
        (BOB, DAN)
    } else {
        (DAN, BOB)
    };
    assert_eq!(
        state.seniority(),
        [id(ALICE), id(first), id(second), id(CAROL), id(MALLORY)]
    );
}
