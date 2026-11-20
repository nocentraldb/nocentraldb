//! RFC 0008's M-C12, "The checks fire": broken fixtures the checks must
//! reject (NOC-382). Each fixture takes a small space, computes the outcome
//! under a broken rule that RFC 0008 did not take, and shows that the
//! property the convergence harness checks fails under it, while `resolve`
//! keeps the property on the same changes. The other broken fixtures of
//! M-C12 live with their properties: `the_last_rotation_to_arrive_would_diverge`
//! (`membership_merge_convergence.rs`), M-C13's
//! `letting_the_last_decision_to_arrive_win_would_diverge`
//! (`membership_erasure_convergence.rs`) and M-C14's
//! `judging_the_add_by_arrival_would_diverge`
//! (`membership_statement_concurrent.rs`).

#[path = "support/space.rs"]
mod space;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::BTreeSet;

use ncdb_core::change::{Change, Header};
use ncdb_core::ids::{ChangeId, MemberId};
use ncdb_core::membership::{
    Action, ControlBody, Entry, MembershipEntry, Resolved, Role, Verdict, resolve,
};

use space::{Space, id};

const ALICE: u8 = 1;
const BOB: u8 = 2;
const CAROL: u8 = 3;
const DAN: u8 = 4;

fn writes(role: Option<Role>) -> bool {
    role >= Some(Role::Writer)
}

/// M-C7's check on one sealed change: admitted above the cutoff of an
/// accepted revocation of its author's right to write that no accepted
/// grant of its author saw.
fn admitted_without_the_right(resolved: &Resolved, header: &Header, admitted: bool) -> bool {
    let Some(state) = resolved.member(header.author) else {
        return admitted;
    };
    let saw = |grant: ChangeId, revocation: ChangeId| {
        resolved
            .seen()
            .get(&grant)
            .is_some_and(|seen| seen.contains(&revocation))
    };
    admitted
        && state
            .revocations
            .iter()
            .filter(|r| !writes(r.role))
            .any(|r| {
                header.sequence > r.cutoff
                    && !state
                        .grants
                        .iter()
                        .filter(|g| writes(Some(g.role)))
                        .any(|g| saw(g.change, r.change))
            })
}

/// Broken: add wins for membership. A grant keeps the role it gives
/// whatever concurrent revocation capped it, as an addition wins in RFC
/// 0004's sets, so a sealed change is admitted under any grant that writes.
/// Alice removes Dan's stolen laptop while Bob, who has not seen that,
/// raises it to admin; the laptop keeps writing. M-C7 rejects the rule:
/// the laptop's change above Alice's cutoff is admitted.
#[test]
fn add_wins_for_membership_admits_a_write_without_the_right() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let bob = s.add(ALICE, BOB, Role::Admin, &[&g]);
    let dan = s.add(ALICE, DAN, Role::Writer, &[&bob]);
    let cutoff = s.sequence(DAN);
    let remove = s.remove(ALICE, DAN, cutoff, &[&dan]);
    let raise = s.set_role(BOB, DAN, Role::Admin, cutoff, &[&dan]);
    let written = s.sealed(DAN, vec![]);
    let state = resolve(s.space, [&g, &bob, &dan, &remove, &raise]);
    assert_eq!(state.verdict(raise.id()), Some(&Verdict::Lost));

    let header = written.header();
    let add_wins = state.member(id(DAN)).is_some_and(|dan| {
        dan.grants.iter().any(|grant| {
            writes(Some(grant.role))
                && grant
                    .from_generation
                    .is_some_and(|from| state.descends(header.key_generation, from))
        })
    });
    assert!(admitted_without_the_right(&state, header, add_wins));
    assert!(!admitted_without_the_right(
        &state,
        header,
        state.admits(header)
    ));
}

/// Broken: a revocation without its cutoff. Without a cutoff, a holder
/// can only exclude what arrives after the revocation, so Dan's change
/// written before Alice removed him counts where it arrived first and not
/// where it arrived last. M-C1 rejects the rule: copies with the same
/// changes reach different verdicts.
#[test]
fn a_revocation_without_its_cutoff_splits_the_copies() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let dan = s.add(ALICE, DAN, Role::Writer, &[&g]);
    let written = s.sealed(DAN, vec![]);
    let remove = s.remove(ALICE, DAN, 0, &[&dan]);
    let orders = [
        vec![&written, &remove], // the change first: kept
        vec![&remove, &written], // the removal first: excluded
    ];
    let without_cutoff: BTreeSet<bool> = orders
        .iter()
        .map(|order| order[0].id() == written.id())
        .collect();
    assert_eq!(without_cutoff.len(), 2, "the broken rule diverges");

    let with_cutoff: BTreeSet<bool> = orders
        .iter()
        .map(|order| {
            let changes = [&g, &dan].into_iter().chain(order.iter().copied());
            resolve(s.space, changes).admits(written.header())
        })
        .collect();
    assert_eq!(with_cutoff.len(), 1);
}

/// Two admins of equal depth, Bob and Carol, remove each other at once.
fn mutual_removal() -> (Space, Vec<Change>, Change, Change) {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let bob = s.add(ALICE, BOB, Role::Admin, &[&g]);
    let carol = s.add(ALICE, CAROL, Role::Admin, &[&g]);
    let bob_removes = s.remove(BOB, CAROL, s.sequence(CAROL), &[&bob, &carol]);
    let carol_removes = s.remove(CAROL, BOB, s.sequence(BOB), &[&bob, &carol]);
    (s, vec![g, bob, carol], bob_removes, carol_removes)
}

/// The subject of a membership entry.
fn subject(change: &Change) -> MemberId {
    match ControlBody::decode(&change.header().payload).map(|b| b.entry) {
        Ok(Entry::Membership(MembershipEntry { subject, .. })) => subject,
        _ => panic!("a membership entry"),
    }
}

/// Broken: seniority by arrival. The admin whose grant a copy received
/// first ranks first, so of two mutual removals the one by that admin
/// wins. M-C1 rejects the rule: copies that received the grants in other
/// orders keep different admins.
#[test]
fn seniority_by_arrival_splits_the_copies() {
    let (s, base, bob_removes, carol_removes) = mutual_removal();
    let [g, bob, carol] = [&base[0], &base[1], &base[2]];
    let grant_orders = [vec![g, bob, carol], vec![g, carol, bob]];
    let survivors: BTreeSet<MemberId> = grant_orders
        .iter()
        .map(|order| {
            let senior = id(match order[1].id() == bob.id() {
                true => BOB,
                false => CAROL,
            });
            let winner = match senior == id(BOB) {
                true => &bob_removes,
                false => &carol_removes,
            };
            match subject(winner) == id(BOB) {
                true => id(CAROL),
                false => id(BOB),
            }
        })
        .collect();
    assert_eq!(survivors.len(), 2, "the broken rule diverges");

    let removals = [&bob_removes, &carol_removes];
    let resolved: BTreeSet<Vec<u8>> = grant_orders
        .iter()
        .map(|order| resolve(s.space, order.iter().copied().chain(removals)).encode())
        .collect();
    assert_eq!(resolved.len(), 1);
}

/// Broken: arrival order instead of the computed order. A copy applies
/// control changes as they arrive, and a change whose author no longer
/// holds the right when it arrives is rejected. Of two mutual removals,
/// the first to arrive wins. M-C1 rejects the rule.
#[test]
fn arrival_order_instead_of_the_computed_order_splits_the_copies() {
    let (s, base, bob_removes, carol_removes) = mutual_removal();
    let orders = [
        vec![&bob_removes, &carol_removes],
        vec![&carol_removes, &bob_removes],
    ];
    let removed: BTreeSet<MemberId> = orders.iter().map(|order| subject(order[0])).collect();
    assert_eq!(removed.len(), 2, "the broken rule diverges");

    let resolved: BTreeSet<Vec<u8>> = orders
        .iter()
        .map(|order| resolve(s.space, base.iter().chain(order.iter().copied())).encode())
        .collect();
    assert_eq!(resolved.len(), 1);
}

/// Broken: a re-grant without a fresh generation. Alice demotes Dan to a
/// reader with a cutoff; Dan, offline, keeps writing under the first
/// generation; Alice then raises him again from that same generation.
/// Under the broken rule the raise is accepted, and the changes Dan wrote
/// before it, above the cutoff, count again. M-C7 rejects the rule: a
/// write without the right comes back. `resolve` refuses the raise, as
/// RFC 0008's own rules for a re-grant require a fresh generation.
#[test]
fn a_re_grant_without_a_fresh_generation_brings_back_writes_without_the_right() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let first = s.first_generation;
    let dan = s.add(ALICE, DAN, Role::Writer, &[&g]);
    let cutoff = s.sequence(DAN);
    let demote = s.set_role(ALICE, DAN, Role::Reader, cutoff, &[&dan]);
    let offline = s.sealed(DAN, vec![]);
    let raise = s.set_role_at(ALICE, DAN, Role::Writer, first, cutoff, &[&demote]);
    let state = resolve(s.space, [&g, &dan, &demote, &raise]);
    assert!(matches!(
        state.verdict(raise.id()),
        Some(Verdict::Rejected(_))
    ));
    assert!(!state.admits(offline.header()));

    // Under the broken rule the raise is an accepted grant that saw the
    // demotion, from the first generation, so it admits every change of
    // Dan's under that generation, the offline one too. Dan wrote it
    // before the raise existed.
    let header = offline.header();
    let broken_admits = header.key_generation == first;
    let above_cutoff = header.sequence > cutoff;
    let dan_had_seen_the_raise = false;
    assert!(
        broken_admits && above_cutoff && !dan_had_seen_the_raise,
        "a write without the right comes back"
    );
}

/// Broken: exposure that counts only accepted envelopes. Alice wraps the
/// first generation's key to Carol, who was never added; the envelope is
/// rejected but may have been delivered. Counting only accepted envelopes,
/// the first generation is not exposed, so no admin rotates, and Carol,
/// outside the state, holds the key members keep writing with. M-C9
/// rejects the rule; `resolve` counts the envelope and exposes it.
#[test]
fn exposure_that_counts_only_accepted_envelopes_leaves_an_outsider_holding_the_key() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let first = s.first_generation;
    let to_carol = s.envelope(ALICE, CAROL, &[&g]);
    let state = resolve(s.space, [&g, &to_carol]);
    assert!(matches!(
        state.verdict(to_carol.id()),
        Some(Verdict::Rejected(_))
    ));
    assert_eq!(state.current_generation(), Some(first));

    // Under the broken rule, who may hold the first generation is its
    // creator and the recipients of accepted envelopes for it.
    let members: BTreeSet<MemberId> = state.members().map(|(m, _)| m).collect();
    let envelopes = [&to_carol];
    let accepted_recipients = envelopes
        .iter()
        .filter(|e| state.verdict(e.id()) == Some(&Verdict::Accepted))
        .map(|_| id(CAROL));
    let broken_holders: BTreeSet<MemberId> =
        [id(ALICE)].into_iter().chain(accepted_recipients).collect();
    let broken_exposed = broken_holders.iter().any(|m| !members.contains(m));
    let outsider_holds_it = !members.contains(&id(CAROL));
    assert!(
        !broken_exposed && outsider_holds_it,
        "nobody rotates while an outsider holds the current generation"
    );
    assert!(state.exposed(first));
}

/// Broken: a revocation that excludes a shred. Dan, a writer, shreds
/// person 1, signed off by Carol's account, while Alice removes Dan with a
/// cutoff below the shred. A copy that receives the shred first accepts it
/// and destroys the person key; under the broken rule the removal, arriving
/// next, rejects it. M-C8 rejects the rule: an accepted shred was rejected
/// later. `resolve` keeps it accepted.
#[test]
fn a_revocation_that_excludes_a_shred_rejects_an_accepted_shred() {
    let mut s = Space::with_nonce(ALICE, [9; 16]);
    let g = s.genesis.clone();
    s.certify_by(CAROL, space::other_account(1));
    let carol = s.add(ALICE, CAROL, Role::Admin, &[&g]);
    let dan = s.add(ALICE, DAN, Role::Writer, &[&carol]);
    let person = s.person_key(ALICE, 1, 1, &[&dan]);
    let cutoff = s.sequence(DAN);
    let shred = s.shred(DAN, 1, &space::other_account(1), &[&person]);
    let remove = s.remove(ALICE, DAN, cutoff, &[&person]);
    let base = [&g, &carol, &dan, &person];

    let first = resolve(s.space, base.into_iter().chain([&shred]));
    let then = resolve(s.space, base.into_iter().chain([&shred, &remove]));
    assert_eq!(first.verdict(shred.id()), Some(&Verdict::Accepted));
    assert_eq!(then.verdict(remove.id()), Some(&Verdict::Accepted));

    let excluded_by_removal = shred.header().sequence > cutoff
        && matches!(
            ControlBody::decode(&remove.header().payload).map(|b| b.entry),
            Ok(Entry::Membership(MembershipEntry {
                action: Action::Remove { .. },
                ..
            }))
        );
    assert!(
        excluded_by_removal,
        "the broken rule rejects the shred later"
    );
    assert_eq!(then.verdict(shred.id()), Some(&Verdict::Accepted));
}
