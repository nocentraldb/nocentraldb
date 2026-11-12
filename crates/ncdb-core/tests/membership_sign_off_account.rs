//! The account of an erasure's sign-off (RFC 0008, "Shred entry"; the
//! owner's answer A on NOC-201): a sign-off counts only if its account
//! differs from the performer's by account key and by every recovery key
//! the certificates in the shred's causal past attach to either account
//! key. Any shared recovery key means the same account, so a statement,
//! forged or not, can only block a sign-off, never enable one. Without any
//! statement this is the comparison of account keys alone. Every
//! certificate counts, the performer's and a former member's included
//! (pm, within NOC-201 A). A certificate in the state carries only a
//! statement that names its own key 1 and verifies, so each attaches its
//! recovery key to its own account key.
//!
//! When the sign-off's account is the performer's, and every member of
//! the shred's causal past belongs to that one account, the shred is
//! valid all the same, but provisional (Carsten, 2026-11-11, NOC-230):
//! [`PROVISIONAL`] stands for that outcome.

#[path = "support/space.rs"]
mod space;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ed25519_dalek::SigningKey;
use ncdb_core::change::Change;
use ncdb_core::keys::RecoveryKey;
use ncdb_core::membership::{AccountKeyStatement, Certificate, MemberKind, Role, Verdict, resolve};

use space::{Space, account, id, other_account, person};

/// The space's creator, of the first account, an admin.
const ALICE: u8 = 1;
/// The performer, of the first account, a writer.
const BOB: u8 = 2;
/// An admin of the second account, who signs off.
const CAROL: u8 = 3;
/// A second admin of the first account.
const ERIN: u8 = 5;

const ACCEPTED: Option<&Verdict> = Some(&Verdict::Accepted);

/// A shred accepted as provisional: the sign-off's account is the
/// performer's, and every member belongs to it.
const PROVISIONAL: Outcome = (Some(Verdict::Accepted), true);
/// A shred accepted as final.
const FINAL: Outcome = (Some(Verdict::Accepted), false);

/// A shred's verdict, and whether it is provisional.
type Outcome = (Option<Verdict>, bool);

/// The outcome of `shred` in `state`.
fn outcome(state: &ncdb_core::membership::Resolved, shred: &Change) -> Outcome {
    let provisional = state
        .person_key(person(1))
        .is_some_and(|key| key.provisional);
    (state.verdict(shred.id()).copied(), provisional)
}

/// A statement under recovery code `code` that names `account`'s key.
fn statement(code: u8, account: &SigningKey) -> AccountKeyStatement {
    let recovery = RecoveryKey::from_code(&[code; 16]);
    AccountKeyStatement::sign(&recovery, account.verifying_key().to_bytes(), 1)
}

/// `who`'s certificate by `account`, carrying a statement under recovery
/// code `code`.
fn certified(who: u8, account: &SigningKey, code: u8) -> Certificate {
    Certificate::sign(account, id(who), MemberKind::Device).with_statement(statement(code, account))
}

/// Bob writes and shreds person 1's key, signed off by the second account,
/// in a space where Carol, admin of the second account, carries a
/// statement under `carol`, and Erin, admin of the first account, under
/// `erin`. The verdict of the shred.
fn sign_off(carol: u8, erin: u8) -> Outcome {
    let mut s = Space::new(ALICE);
    let genesis = s.genesis.clone();
    let bob = s.add(ALICE, BOB, Role::Writer, &[&genesis]);
    let carol = s.add_certified(
        ALICE,
        CAROL,
        Role::Admin,
        certified(CAROL, &other_account(1), carol),
        &[&bob],
    );
    let erin = s.add_certified(
        ALICE,
        ERIN,
        Role::Admin,
        certified(ERIN, &account(), erin),
        &[&carol],
    );
    let entry = s.person_key(BOB, 1, 0xc1, &[&erin]);
    let shred = s.shred(BOB, 1, &other_account(1), &[&entry]);
    let all: Vec<&Change> = vec![&genesis, &bob, &carol, &erin, &entry, &shred];
    let state = resolve(s.space, all);
    assert_eq!(state.verdict(erin.id()), ACCEPTED);
    if state.verdict(shred.id()) == ACCEPTED {
        assert_eq!(state.person_key(person(1)).unwrap().shred, Some(shred.id()));
    }
    outcome(&state, &shred)
}

#[test]
fn two_account_keys_under_different_recovery_keys_are_two_accounts() {
    assert_eq!(sign_off(7, 8), FINAL);
}

#[test]
fn two_account_keys_under_one_recovery_key_are_one_account() {
    // After a recovery, one person's old and new account keys: the second
    // account's admin is the performer's own account. Every member then
    // belongs to one account, so the own sign-off makes it provisional.
    assert_eq!(sign_off(7, 7), PROVISIONAL);
}

/// Bob, the performer, carries a statement under recovery code 7; Carol,
/// admin of the second account, carries `carol`; Frank, of the first
/// account, carries `frank` and is removed before the shred if
/// `frank_removed`. The verdict of Bob's shred signed off by the second
/// account.
fn with_bob_and_frank(carol: u8, frank: Option<(u8, bool)>, bob: Option<u8>) -> Outcome {
    const FRANK: u8 = 6;
    let mut s = Space::new(ALICE);
    let genesis = s.genesis.clone();
    let bob_add = match bob {
        Some(code) => s.add_certified(
            ALICE,
            BOB,
            Role::Writer,
            certified(BOB, &account(), code),
            &[&genesis],
        ),
        None => s.add(ALICE, BOB, Role::Writer, &[&genesis]),
    };
    let carol_add = s.add_certified(
        ALICE,
        CAROL,
        Role::Admin,
        certified(CAROL, &other_account(1), carol),
        &[&bob_add],
    );
    let mut all = vec![genesis, bob_add, carol_add.clone()];
    let mut last = carol_add;
    if let Some((code, removed)) = frank {
        let add = s.add_certified(
            ALICE,
            FRANK,
            Role::Admin,
            certified(FRANK, &account(), code),
            &[&last],
        );
        all.push(add.clone());
        last = add;
        if removed {
            let removal = s.remove(ALICE, FRANK, 0, &[&last]);
            all.push(removal.clone());
            last = removal;
        }
    }
    let entry = s.person_key(BOB, 1, 0xc1, &[&last]);
    let shred = s.shred(BOB, 1, &other_account(1), &[&entry]);
    all.extend([entry, shred.clone()]);
    outcome(&resolve(s.space, all.iter()), &shred)
}

#[test]
fn the_performers_own_statement_counts() {
    // Bob is a writer, not an admin, but his certificate attaches recovery
    // code 7 to the first account, and Carol's attaches it to the second.
    assert_eq!(with_bob_and_frank(7, None, Some(7)), PROVISIONAL);
    assert_eq!(with_bob_and_frank(7, None, Some(8)), FINAL);
}

#[test]
fn a_former_admins_statement_counts() {
    // Frank, an admin of the first account under recovery code 7, is
    // removed before the shred; his certificate still names the account.
    assert_eq!(with_bob_and_frank(7, Some((7, true)), None), PROVISIONAL);
    assert_eq!(with_bob_and_frank(7, Some((8, true)), None), FINAL);
}
