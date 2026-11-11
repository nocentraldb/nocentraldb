//! Who signs off an erasure (RFC 0008, "Roles and rights" and "Shred
//! entry"; the owner's NOC-228, question 2 A, Carsten, 2026-11-11): another
//! account that holds an admin member in the state of the shred's causal
//! past, or, when exactly one account holds an admin member there and it
//! is the performer's account, another account that holds a writer member.
//! A reader's account never signs off.

#[path = "support/space.rs"]
mod space;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::change::Change;
use ncdb_core::membership::{Rejection, Role, Verdict, resolve};

use space::{Space, other_account};

/// Alice, of the default account, creates the space and is its only admin.
const ALICE: u8 = 1;
/// A writer of Alice's account.
const BOB: u8 = 2;
/// A member of a second account, with the role each test gives.
const CAROL: u8 = 3;
/// An admin of a third account.
const DAN: u8 = 4;
/// A writer of a fourth account.
const ERIN: u8 = 5;

/// The verdict of a shred of person 1 by `performer`, signed off by
/// Carol's account, in a space where Carol has `role`, after `setup` added
/// further members.
fn verdict(
    performer: u8,
    role: Role,
    setup: impl FnOnce(&mut Space, &Change) -> Change,
) -> Verdict {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let bob = s.add(ALICE, BOB, Role::Writer, &[&g]);
    s.certify_by(CAROL, other_account(1));
    let carol = s.add(ALICE, CAROL, role, &[&bob]);
    let last = setup(&mut s, &carol);
    let entry = s.person_key(BOB, 1, 0xc1, &[&last]);
    let shred = s.shred(performer, 1, &other_account(1), &[&entry]);
    let mut changes = vec![g, bob, carol];
    if last.id() != changes[2].id() {
        changes.push(last);
    }
    changes.extend([entry, shred.clone()]);
    resolve(s.space, &changes)
        .verdict(shred.id())
        .cloned()
        .expect("a verdict")
}

fn none(_: &mut Space, last: &Change) -> Change {
    last.clone()
}

#[test]
fn another_accounts_writer_signs_off_when_the_performers_account_is_the_only_admin_account() {
    assert_eq!(verdict(BOB, Role::Writer, none), Verdict::Accepted);
    assert_eq!(verdict(ALICE, Role::Writer, none), Verdict::Accepted);
}

#[test]
fn a_readers_account_never_signs_off() {
    assert_eq!(
        verdict(BOB, Role::Reader, none),
        Verdict::Rejected(Rejection::NotValidWhenWritten)
    );
}

#[test]
fn a_writers_account_does_not_sign_off_when_another_account_holds_an_admin() {
    let dan = |s: &mut Space, last: &Change| {
        s.certify_by(DAN, other_account(2));
        s.add(ALICE, DAN, Role::Admin, &[last])
    };
    assert_eq!(
        verdict(BOB, Role::Writer, dan),
        Verdict::Rejected(Rejection::NotValidWhenWritten)
    );
}

/// The only admin account is Alice's, but Erin, who performs, belongs to a
/// fourth account: the exception does not apply, and a writer's account
/// does not sign off.
#[test]
fn a_writers_account_does_not_sign_off_when_the_only_admin_account_is_not_the_performers() {
    let erin = |s: &mut Space, last: &Change| {
        s.certify_by(ERIN, other_account(3));
        s.add(ALICE, ERIN, Role::Writer, &[last])
    };
    assert_eq!(
        verdict(ERIN, Role::Writer, erin),
        Verdict::Rejected(Rejection::NotValidWhenWritten)
    );
}

#[test]
fn another_accounts_admin_still_signs_off() {
    assert_eq!(verdict(BOB, Role::Admin, none), Verdict::Accepted);
}

/// With no admin left, no account holds one: the exception needs exactly
/// one, so a writer's account does not sign off.
#[test]
fn a_writers_account_does_not_sign_off_in_a_space_without_an_admin() {
    let alice_leaves = |s: &mut Space, last: &Change| {
        let cutoff = s.sequence(ALICE);
        s.remove(ALICE, ALICE, cutoff, &[last])
    };
    assert_eq!(
        verdict(BOB, Role::Writer, alice_leaves),
        Verdict::Rejected(Rejection::NotValidWhenWritten)
    );
}
