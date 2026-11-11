//! A shred is final when accepted (RFC 0008, "A shred is final when
//! accepted"; the owner's NOC-224, Carsten, 2026-11-11): its verdict reads
//! only its causal past. No revocation excludes it, and step 5 does not ask
//! whether its author still holds the right in the state so far, so no
//! change that arrives later rejects it. A shred whose own causal past
//! holds the revocation is still rejected: it is not valid when written.

#[path = "support/space.rs"]
mod space;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::change::Change;
use ncdb_core::membership::{Rejection, Resolved, Role, Verdict, resolve};

use space::{Space, id, other_account};

const ALICE: u8 = 1;
const BOB: u8 = 2;
const CAROL: u8 = 3;
const DAN: u8 = 4;

/// Alice creates the space and adds Carol, an admin of a second account,
/// then Bob as a writer. Bob writes a person-key entry and shreds it under
/// Carol's account's sign-off.
struct Shred {
    s: Space,
    changes: Vec<Change>,
    bob: Change,
    carol: Change,
    shred: Change,
}

fn shred() -> Shred {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    s.certify_by(CAROL, other_account(1));
    let carol = s.add(ALICE, CAROL, Role::Admin, &[&g]);
    let bob = s.add(ALICE, BOB, Role::Writer, &[&carol]);
    let entry = s.person_key(BOB, 1, 0xc1, &[&bob]);
    let shred = s.shred(BOB, 1, &other_account(1), &[&entry]);
    let changes = vec![g, carol.clone(), bob.clone(), entry, shred.clone()];
    Shred {
        s,
        changes,
        bob,
        carol,
        shred,
    }
}

fn with(changes: &[Change], late: &[&Change]) -> Vec<Change> {
    changes
        .iter()
        .cloned()
        .chain(late.iter().map(|c| (*c).clone()))
        .collect()
}

fn state(s: &Space, changes: &[Change]) -> Resolved {
    resolve(s.space, changes)
}

#[test]
fn a_concurrent_removal_of_the_performer_does_not_reject_the_shred() {
    let Shred {
        mut s,
        changes,
        bob,
        shred,
        ..
    } = shred();
    let before_shred = shred.header().sequence - 1;
    let removal = s.remove(ALICE, BOB, before_shred, &[&bob]);

    let after = state(&s, &with(&changes, &[&removal]));

    assert_eq!(after.verdict(removal.id()), Some(&Verdict::Accepted));
    assert_eq!(after.role(id(BOB)), None);
    assert_eq!(after.verdict(shred.id()), Some(&Verdict::Accepted));
    let key = after.person_key(space::person(1)).expect("the person key");
    assert_eq!(key.shred, Some(shred.id()));
}

#[test]
fn a_concurrent_demotion_of_the_performer_to_reader_does_not_reject_the_shred() {
    let Shred {
        mut s,
        changes,
        bob,
        shred,
        ..
    } = shred();
    let before_shred = shred.header().sequence - 1;
    let demotion = s.set_role(ALICE, BOB, Role::Reader, before_shred, &[&bob]);

    let after = state(&s, &with(&changes, &[&demotion]));

    assert_eq!(after.role(id(BOB)), Some(Role::Reader));
    assert_eq!(after.verdict(shred.id()), Some(&Verdict::Accepted));
}

/// The right the performer's grant rests on goes: Carol, who added Bob in
/// this variant, is removed by Alice with a cutoff below that add. Bob's
/// grant is excluded, but the shred stands.
#[test]
fn a_revocation_that_takes_the_performers_grant_away_does_not_reject_the_shred() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    s.certify_by(CAROL, other_account(1));
    let carol = s.add(ALICE, CAROL, Role::Admin, &[&g]);
    s.certify_by(DAN, other_account(2));
    let dan = s.add(ALICE, DAN, Role::Admin, &[&carol]);
    let bob = s.add(CAROL, BOB, Role::Writer, &[&dan]);
    let entry = s.person_key(BOB, 1, 0xc1, &[&bob]);
    let shred = s.shred(BOB, 1, &other_account(2), &[&entry]);
    let before_add = bob.header().sequence - 1;
    let removal = s.remove(ALICE, CAROL, before_add, &[&dan]);

    let after = state(&s, &[g, carol, dan, bob.clone(), entry, shred.clone(), removal]);

    assert_ne!(after.verdict(bob.id()), Some(&Verdict::Accepted));
    assert_eq!(after.verdict(shred.id()), Some(&Verdict::Accepted));
}

/// The sign-off rests on Carol's account holding an admin member in the
/// shred's causal past. A concurrent removal of Carol does not change that.
#[test]
fn a_concurrent_removal_of_the_sign_off_accounts_admin_does_not_reject_the_shred() {
    let Shred {
        mut s,
        changes,
        carol,
        shred,
        ..
    } = shred();
    let removal = s.remove(ALICE, CAROL, s.sequence(CAROL), &[&carol]);

    let after = state(&s, &with(&changes, &[&removal]));

    assert_eq!(after.role(id(CAROL)), None);
    assert_eq!(after.verdict(shred.id()), Some(&Verdict::Accepted));
}

/// A shred written after its author saw its own removal is not valid when
/// written: finality covers what a shred's past allowed, nothing more.
#[test]
fn a_shred_whose_past_holds_its_authors_removal_is_rejected() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    s.certify_by(CAROL, other_account(1));
    let carol = s.add(ALICE, CAROL, Role::Admin, &[&g]);
    let bob = s.add(ALICE, BOB, Role::Writer, &[&carol]);
    let entry = s.person_key(BOB, 1, 0xc1, &[&bob]);
    let removal = s.remove(ALICE, BOB, s.sequence(BOB), &[&entry]);
    let shred = s.shred(BOB, 1, &other_account(1), &[&removal]);

    let after = state(&s, &[g, carol, bob, entry, removal, shred.clone()]);

    assert_eq!(
        after.verdict(shred.id()),
        Some(&Verdict::Rejected(Rejection::NotValidWhenWritten))
    );
}
