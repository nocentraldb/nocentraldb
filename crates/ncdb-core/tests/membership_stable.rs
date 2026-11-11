//! When a control change is stable (RFC 0008, "Effects that wait until a
//! change is stable", M20/M21): the smallest set closed under its rule. A
//! change is stable once the holder holds, for every admin and every former
//! admin, that member's changes from sequence 1 up to a control change of
//! that member that saw it, or up to the cutoff of a stable revocation that
//! took the role. A waiting shred names the members that block it.

#[path = "support/space.rs"]
mod space;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::{BTreeMap, BTreeSet};

use ncdb_core::change::Change;
use ncdb_core::ids::MemberId;
use ncdb_core::membership::{Role, Verdict, resolve};

use space::{Space, id, other_account};

const ALICE: u8 = 1;
const BOB: u8 = 2;
const CAROL: u8 = 3;
const DAN: u8 = 4;

/// Alice admin, Bob writer, Carol admin of a second account; Bob writes a
/// person-key entry and shreds it with Carol's account's sign-off.
fn shredded() -> (Space, Vec<Change>, Change) {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let bob = s.add(ALICE, BOB, Role::Writer, &[&g]);
    s.certify_by(CAROL, other_account(1));
    let carol = s.add(ALICE, CAROL, Role::Admin, &[&bob]);
    let entry = s.person_key(BOB, 1, 0xc1, &[&carol]);
    let shred = s.shred(BOB, 1, &other_account(1), &[&entry]);
    let held = vec![g, bob, carol, entry];
    (s, held, shred)
}

/// The holder holds every member's run up to what `s` wrote.
fn all_of(s: &Space) -> impl Fn(MemberId) -> u64 + '_ {
    move |member| {
        [ALICE, BOB, CAROL, DAN]
            .into_iter()
            .find(|who| id(*who) == member)
            .map_or(0, |who| s.sequence(who))
    }
}

fn runs(entries: &[(u8, u64)]) -> impl Fn(MemberId) -> u64 {
    let runs: BTreeMap<MemberId, u64> = entries.iter().map(|(who, n)| (id(*who), *n)).collect();
    move |member| runs.get(&member).copied().unwrap_or(0)
}

fn ids(who: &[u8]) -> BTreeSet<MemberId> {
    who.iter().map(|w| id(*w)).collect()
}

#[test]
fn a_shred_no_admin_wrote_after_waits_for_every_admin() {
    let (s, held, shred) = shredded();
    let changes: Vec<&Change> = held.iter().chain([&shred]).collect();
    let state = resolve(s.space, changes.iter().copied());
    assert_eq!(state.verdict(shred.id()), Some(&Verdict::Accepted));
    let stability = state.stability(changes.iter().copied(), all_of(&s));
    assert!(!stability.is_stable(shred.id()));
    assert_eq!(stability.blocking(shred.id()), Some(&ids(&[ALICE, CAROL])));
    assert_eq!(stability.shreds_waiting().count(), 1);
}

#[test]
fn a_shred_is_stable_once_every_admin_wrote_a_control_change_that_saw_it() {
    let (mut s, held, shred) = shredded();
    let by_alice = s.envelope(ALICE, BOB, &[&shred]);
    let by_carol = s.envelope(CAROL, BOB, &[&shred]);
    let changes: Vec<&Change> = held.iter().chain([&shred, &by_alice, &by_carol]).collect();
    let state = resolve(s.space, changes.iter().copied());
    let stability = state.stability(changes.iter().copied(), all_of(&s));
    assert!(stability.is_stable(shred.id()));
    assert_eq!(stability.blocking(shred.id()), None);
    assert_eq!(stability.shreds_waiting().count(), 0);
}

#[test]
fn one_admin_that_did_not_write_after_it_blocks_the_shred() {
    let (mut s, held, shred) = shredded();
    let by_alice = s.envelope(ALICE, BOB, &[&shred]);
    let changes: Vec<&Change> = held.iter().chain([&shred, &by_alice]).collect();
    let state = resolve(s.space, changes.iter().copied());
    let stability = state.stability(changes.iter().copied(), all_of(&s));
    assert!(!stability.is_stable(shred.id()));
    assert_eq!(stability.blocking(shred.id()), Some(&ids(&[CAROL])));
}

#[test]
fn the_holder_must_hold_the_admins_run_without_a_gap() {
    // Carol's change that saw the shred follows sealed changes the holder
    // does not hold, so her run is not held up to it.
    let (mut s, held, shred) = shredded();
    let by_alice = s.envelope(ALICE, BOB, &[&shred]);
    s.skip(CAROL, 3);
    let by_carol = s.envelope(CAROL, BOB, &[&shred]);
    let changes: Vec<&Change> = held.iter().chain([&shred, &by_alice, &by_carol]).collect();
    let state = resolve(s.space, changes.iter().copied());
    let gap = runs(&[(ALICE, s.sequence(ALICE)), (CAROL, 1)]);
    let stability = state.stability(changes.iter().copied(), gap);
    assert!(!stability.is_stable(shred.id()));
    assert_eq!(stability.blocking(shred.id()), Some(&ids(&[CAROL])));
    let whole = state.stability(changes.iter().copied(), all_of(&s));
    assert!(whole.is_stable(shred.id()));
}

#[test]
fn a_former_admin_is_bounded_by_the_cutoff_of_its_stable_demotion() {
    // Carol is demoted after she saw the demotion's past; she wrote a
    // change that saw the demotion, which makes it stable, and its cutoff
    // then bounds her run for the later shred.
    let (mut s, held, _) = shredded();
    let carol_seen = s.envelope(CAROL, BOB, &[&held[3]]);
    let cutoff = s.sequence(CAROL);
    let demote = s.set_role(ALICE, CAROL, Role::Writer, cutoff, &[&carol_seen]);
    let carol_ack = s.envelope(CAROL, BOB, &[&demote]);
    let entry = s.person_key(BOB, 2, 0xc2, &[&demote]);
    // Dan, of a third account, is the second admin who signs off.
    s.certify_by(DAN, other_account(2));
    let dan = s.add(ALICE, DAN, Role::Admin, &[&entry]);
    let shred = s.shred(BOB, 2, &other_account(2), &[&dan]);
    let by_alice = s.envelope(ALICE, BOB, &[&shred]);
    let by_dan = s.envelope(DAN, BOB, &[&shred]);
    let changes: Vec<&Change> = held
        .iter()
        .chain([
            &carol_seen,
            &demote,
            &carol_ack,
            &entry,
            &dan,
            &shred,
            &by_alice,
            &by_dan,
        ])
        .collect();
    let state = resolve(s.space, changes.iter().copied());
    assert_eq!(state.verdict(shred.id()), Some(&Verdict::Accepted));
    assert_eq!(state.role(id(CAROL)), Some(Role::Writer));
    // Alice and Dan saw the demotion only through the shred's past and
    // their later envelopes, so it is stable; Carol never saw the shred.
    let stability = state.stability(changes.iter().copied(), all_of(&s));
    assert!(stability.is_stable(demote.id()));
    assert!(stability.is_stable(shred.id()));
}
