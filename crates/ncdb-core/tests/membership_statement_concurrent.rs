//! Step 5 of RFC 0008's `resolve` for the account key statement entry
//! (Carsten, 2026-11-11, NOC-187), and harness property M-C14: an add
//! concurrent with an accepted statement entry that supersedes the account
//! key its certificate names is rejected, wherever it falls in the order,
//! on every copy in any order of arrival. Concurrent means neither is in
//! the other's causal past (pm, NOC-233, for the owner's NOC-187 4B), so
//! an add the entry saw stays, and its member with it. A sign-off is
//! judged by its shred's causal past only. The broken fixture judges the
//! add by arrival instead, and the copies diverge.

#[path = "support/space.rs"]
mod space;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::BTreeSet;

use ed25519_dalek::SigningKey;
use ncdb_core::change::Change;
use ncdb_core::ids::SpaceId;
use ncdb_core::keys::RecoveryKey;
use ncdb_core::membership::{
    AccountKeyStatement, Certificate, Entry, MemberKind, Rejection, Resolved, Role, Verdict,
    resolve,
};

use space::{Space, Who, id, other_account};

/// The space's creator, an admin of the first account.
const ALICE: Who = 1;
/// The lost device: an admin of the account that changes its account key.
const BOB: Who = 2;
/// A device the thief adds with the old account key.
const CAROL: Who = 3;
/// A reader who carries the newer statement into the space.
const DAVE: Who = 4;
/// A device the thief's device adds in turn.
const FRANK: Who = 6;

/// The recovery code of Bob's account.
const CODE: u8 = 0x51;

const ACCEPTED: Option<&Verdict> = Some(&Verdict::Accepted);

fn old_key() -> SigningKey {
    other_account(1)
}

fn new_key() -> SigningKey {
    other_account(2)
}

fn statement(account: &SigningKey, number: u64) -> AccountKeyStatement {
    let recovery = RecoveryKey::from_code(&[CODE; 16]);
    AccountKeyStatement::sign(&recovery, account.verifying_key().to_bytes(), number)
}

fn certified(who: Who, account: &SigningKey, number: u64) -> Certificate {
    Certificate::sign(account, id(who), MemberKind::Device)
        .with_statement(statement(account, number))
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

/// Alice creates the space and adds Bob, an admin certified by the old
/// account key with statement 1, and Dave, a reader.
fn base(s: &mut Space) -> Vec<Change> {
    let genesis = s.genesis.clone();
    let bob = s.add_certified(
        ALICE,
        BOB,
        Role::Admin,
        certified(BOB, &old_key(), 1),
        &[&genesis],
    );
    let dave = s.add(ALICE, DAVE, Role::Reader, &[&bob]);
    vec![genesis, bob, dave]
}

#[test]
fn m_c14_an_add_concurrent_with_a_newer_statement_is_rejected_in_any_order() {
    let mut s = Space::with_nonce(ALICE, [9; 16]);
    let base = base(&mut s);
    let dave = base[2].clone();
    // The thief, on Bob's lost device, adds Carol with the old key, and
    // Carol adds Frank; Dave carries statement 2 concurrently.
    let thief = s.add_certified(
        BOB,
        CAROL,
        Role::Admin,
        certified(CAROL, &old_key(), 1),
        &[&dave],
    );
    let frank = s.add(CAROL, FRANK, Role::Writer, &[&thief]);
    let entry = s.write(
        DAVE,
        Entry::AccountKeyStatement(statement(&new_key(), 2)),
        &[&dave],
    );
    let state = in_every_order(
        s.space,
        &base,
        &[thief.clone(), frank.clone(), entry.clone()],
    );
    assert_eq!(state.verdict(entry.id()), ACCEPTED);
    assert_eq!(
        state.verdict(thief.id()),
        Some(&Verdict::Rejected(Rejection::Superseded(entry.id())))
    );
    assert!(matches!(
        state.verdict(frank.id()),
        Some(Verdict::Rejected(_))
    ));
    assert_eq!(state.role(id(CAROL)), None);
    assert_eq!(state.role(id(FRANK)), None);
    // The lost device stays a member until an admin removes it.
    assert_eq!(state.role(id(BOB)), Some(Role::Admin));
}

#[test]
fn an_add_the_statement_entry_saw_stays() {
    let mut s = Space::with_nonce(ALICE, [9; 16]);
    let base = base(&mut s);
    let dave = base[2].clone();
    let before = s.add_certified(
        ALICE,
        CAROL,
        Role::Writer,
        certified(CAROL, &old_key(), 1),
        &[&dave],
    );
    let entry = s.write(
        DAVE,
        Entry::AccountKeyStatement(statement(&new_key(), 2)),
        &[&before],
    );
    let state = in_every_order(s.space, &base, &[before.clone(), entry.clone()]);
    assert_eq!(state.verdict(entry.id()), ACCEPTED);
    assert_eq!(state.verdict(before.id()), ACCEPTED);
    assert_eq!(state.role(id(CAROL)), Some(Role::Writer));
}

#[test]
fn an_add_under_the_newer_key_concurrent_with_the_entry_stays() {
    let mut s = Space::with_nonce(ALICE, [9; 16]);
    let base = base(&mut s);
    let dave = base[2].clone();
    let add = s.add_certified(
        ALICE,
        CAROL,
        Role::Writer,
        certified(CAROL, &new_key(), 2),
        &[&dave],
    );
    let entry = s.write(
        DAVE,
        Entry::AccountKeyStatement(statement(&new_key(), 2)),
        &[&dave],
    );
    let state = in_every_order(s.space, &base, &[add.clone(), entry.clone()]);
    assert_eq!(state.verdict(add.id()), ACCEPTED);
    assert_eq!(state.verdict(entry.id()), ACCEPTED);
}

#[test]
fn m_c14_a_sign_off_concurrent_with_the_entry_is_judged_by_its_shreds_causal_past() {
    let mut s = Space::with_nonce(ALICE, [9; 16]);
    let genesis = s.genesis.clone();
    let writer = s.add(ALICE, FRANK, Role::Writer, &[&genesis]);
    let bob = s.add_certified(
        ALICE,
        BOB,
        Role::Admin,
        certified(BOB, &old_key(), 1),
        &[&writer],
    );
    let key = s.person_key(FRANK, 1, 0xc1, &[&bob]);
    let shred = s.shred(FRANK, 1, &old_key(), &[&key]);
    let entry = s.write(
        ALICE,
        Entry::AccountKeyStatement(statement(&new_key(), 2)),
        &[&key],
    );
    let base = [genesis, writer, bob, key];
    let state = in_every_order(s.space, &base, &[shred.clone(), entry.clone()]);
    assert_eq!(state.verdict(entry.id()), ACCEPTED);
    assert_eq!(state.verdict(shred.id()), ACCEPTED);
}

/// M-C12's broken fixture for M-C14: a rule that accepts an add unless a
/// statement entry superseding its key arrived before it. The orders of
/// arrival tried here end in different verdicts for the same add, where
/// `resolve` gives one (the test above).
#[test]
fn judging_the_add_by_arrival_would_diverge() {
    let mut s = Space::with_nonce(ALICE, [9; 16]);
    let base = base(&mut s);
    let dave = base[2].clone();
    let thief = s.add_certified(
        BOB,
        CAROL,
        Role::Admin,
        certified(CAROL, &old_key(), 1),
        &[&dave],
    );
    let entry = s.write(
        DAVE,
        Entry::AccountKeyStatement(statement(&new_key(), 2)),
        &[&dave],
    );
    let arrived_first = |order: &[Change]| order[0].id() == entry.id();
    let outcomes: BTreeSet<bool> = permutations(&[thief.clone(), entry.clone()])
        .iter()
        .map(|order| arrived_first(order))
        .collect();
    assert_eq!(outcomes.len(), 2, "the broken rule diverges");
    let verdicts: Vec<Option<Verdict>> = permutations(&[thief.clone(), entry])
        .iter()
        .map(|order| {
            resolve(s.space, base.iter().chain(order))
                .verdict(thief.id())
                .cloned()
        })
        .collect();
    assert!(
        verdicts.iter().all(|v| *v == verdicts[0]),
        "resolve gives one verdict"
    );
}
