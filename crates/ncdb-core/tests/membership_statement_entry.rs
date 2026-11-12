//! The account key statement entry of RFC 0008 (control entry type 7;
//! Carsten, 2026-11-11, NOC-187): its bytes against the vector of
//! tests/vectors/membership/, which generate.py writes with cbor2 and
//! cryptography independently of this crate; when it is valid when
//! written; and what a superseded account key no longer does in step 1:
//! an add whose certificate names it, and a sign-off by it, are not valid
//! when written. Statement entries also count when two account keys are
//! compared as one account (NOC-201).

#[path = "support/space.rs"]
mod space;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::BTreeMap;

use ed25519_dalek::SigningKey;
use ncdb_core::cbor::{self, Key, Value};
use ncdb_core::change::Change;
use ncdb_core::ids::ChangeId;
use ncdb_core::keys::RecoveryKey;
use ncdb_core::membership::{
    ACCOUNT_KEY_STATEMENT, AccountKeyStatement, BodyError, Certificate, ControlBody, Entry,
    MemberKind, Rejection, Resolved, Role, Verdict, resolve,
};

use space::{Space, Who, account, dependencies, id, other_account};

const BODY: &str = include_str!("vectors/membership/statement-entry.body.hex");
const STATEMENT_2: &str = include_str!("vectors/membership/statement-2.hex");
const ADD_ID: &str = include_str!("vectors/membership/add.id.hex");
const RECOVERY_PUBLIC: &str = include_str!("vectors/membership/recovery-public.hex");

/// The space's creator, an admin.
const ALICE: Who = 1;
/// A member of the account that changes its account key.
const BOB: Who = 2;
/// A member added after the statement.
const CAROL: Who = 3;
/// A reader.
const DAVE: Who = 4;
/// A second member of the account.
const ERIN: Who = 5;

/// The recovery code of the account that changes its account key.
const CODE: u8 = 0x51;
/// The recovery code of an account with no member in the space.
const STRANGER: u8 = 0x52;

const ACCEPTED: Option<&Verdict> = Some(&Verdict::Accepted);
const NOT_VALID: Option<&Verdict> = Some(&Verdict::Rejected(Rejection::NotValidWhenWritten));

fn hex(text: &str) -> Vec<u8> {
    let text: String = text.split_whitespace().collect();
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

/// The account key of the account before and after the lost device.
fn old_key() -> SigningKey {
    other_account(1)
}

fn new_key() -> SigningKey {
    other_account(2)
}

/// The statement under recovery code `code` that `account` is the
/// account's key with `number`.
fn statement(code: u8, account: &SigningKey, number: u64) -> AccountKeyStatement {
    let recovery = RecoveryKey::from_code(&[code; 16]);
    AccountKeyStatement::sign(&recovery, account.verifying_key().to_bytes(), number)
}

/// `who`'s certificate by `account`, carrying the statement `number` of
/// `account` under recovery code `code`.
fn certified(who: Who, account: &SigningKey, code: u8, number: u64) -> Certificate {
    Certificate::sign(account, id(who), MemberKind::Device)
        .with_statement(statement(code, account, number))
}

/// A space where Alice, the creator, adds Bob as a writer, certified by
/// the old account key with statement 1, and Dave as a reader.
struct Lost {
    s: Space,
    genesis: Change,
    bob: Change,
    dave: Change,
}

impl Lost {
    fn new() -> Lost {
        let mut s = Space::new(ALICE);
        let genesis = s.genesis.clone();
        let bob = s.add_certified(
            ALICE,
            BOB,
            Role::Writer,
            certified(BOB, &old_key(), CODE, 1),
            &[&genesis],
        );
        let dave = s.add(ALICE, DAVE, Role::Reader, &[&bob]);
        Lost {
            s,
            genesis,
            bob,
            dave,
        }
    }

    /// `author` writes a statement entry after `deps`.
    fn carry(&mut self, author: Who, statement: AccountKeyStatement, deps: &[&Change]) -> Change {
        self.s
            .write(author, Entry::AccountKeyStatement(statement), deps)
    }

    fn verdicts<'a>(&'a self, more: impl IntoIterator<Item = &'a Change>) -> Resolved {
        let mut all = vec![&self.genesis, &self.bob, &self.dave];
        all.extend(more);
        resolve(self.s.space, all)
    }
}

#[test]
fn the_entry_decodes_from_the_vector_and_encodes_to_the_same_bytes() {
    let bytes = hex(BODY);
    assert_eq!(bytes.len(), 180);
    let body = ControlBody::decode(&bytes).unwrap();
    assert_eq!(ACCOUNT_KEY_STATEMENT, 7);
    assert_eq!(body.entry_type(), ACCOUNT_KEY_STATEMENT);
    let add = ChangeId(hex(ADD_ID).try_into().unwrap());
    assert_eq!(
        body.dependencies.iter().copied().collect::<Vec<_>>(),
        vec![add]
    );
    let Entry::AccountKeyStatement(statement) = &body.entry else {
        panic!("not a statement entry: {:?}", body.entry);
    };
    assert_eq!(statement.encode(), hex(STATEMENT_2));
    assert_eq!(statement.encode().len(), 140);
    assert_eq!(statement.recovery_key.to_vec(), hex(RECOVERY_PUBLIC));
    assert_eq!(
        statement.account_key,
        SigningKey::from_bytes(&[0x12; 32])
            .verifying_key()
            .to_bytes()
    );
    assert_eq!(statement.number, 2);
    assert!(statement.verify());
    assert_eq!(body.encode(), bytes);
    let built = ControlBody::new(dependencies(&[]), body.entry.clone());
    assert_eq!(built.entry_type(), 7);
}

/// The vector's body with key `key` set to `value`, or removed with `None`.
fn changed(key: u64, value: Option<Value>) -> Vec<u8> {
    let Value::Map(mut map) = cbor::decode(&hex(BODY)).unwrap() else {
        panic!("a body is a map");
    };
    match value {
        Some(value) => map.insert(Key::Unsigned(key), value),
        None => map.remove(&Key::Unsigned(key)),
    };
    cbor::encode(&Value::Map(map))
}

#[test]
fn an_entry_without_a_statement_or_with_a_broken_one_is_malformed() {
    let missing = ControlBody::decode(&changed(8, None));
    assert!(
        matches!(missing, Err(BodyError::Malformed(_))),
        "{missing:?}"
    );
    let not_a_map = ControlBody::decode(&changed(8, Some(Value::Unsigned(2))));
    assert!(
        matches!(not_a_map, Err(BodyError::Malformed(_))),
        "{not_a_map:?}"
    );
    let Value::Map(statement) = cbor::decode(&hex(STATEMENT_2)).unwrap() else {
        panic!("a statement is a map");
    };
    let mut zero = statement.clone();
    zero.insert(Key::Unsigned(3), Value::Unsigned(0));
    let numbered_zero = ControlBody::decode(&changed(8, Some(Value::Map(zero))));
    assert!(
        matches!(numbered_zero, Err(BodyError::Malformed(_))),
        "{numbered_zero:?}"
    );
}

#[test]
fn an_unknown_must_understand_key_is_not_understood_and_a_may_ignore_key_is_kept() {
    let unknown = ControlBody::decode(&changed(9, Some(Value::Unsigned(1))));
    assert!(
        matches!(unknown, Err(BodyError::NotUnderstood)),
        "{unknown:?}"
    );
    let extended = changed(64, Some(Value::Unsigned(1)));
    let body = ControlBody::decode(&extended).unwrap();
    assert!(matches!(body.entry, Entry::AccountKeyStatement(_)));
    assert_eq!(body.encode(), extended);
}

#[test]
fn a_newer_statement_for_a_members_recovery_key_is_valid_and_a_reader_may_carry_it() {
    let mut l = Lost::new();
    let dave = l.dave.clone();
    let by_admin = l.carry(ALICE, statement(CODE, &new_key(), 2), &[&dave]);
    let by_reader = l.carry(DAVE, statement(CODE, &new_key(), 3), &[&dave]);
    let state = l.verdicts([&by_admin, &by_reader]);
    assert_eq!(state.verdict(by_admin.id()), ACCEPTED);
    assert_eq!(state.verdict(by_reader.id()), ACCEPTED);
}

#[test]
fn a_statement_for_a_recovery_key_no_certificate_names_is_not_valid() {
    let mut l = Lost::new();
    let dave = l.dave.clone();
    let entry = l.carry(ALICE, statement(STRANGER, &new_key(), 2), &[&dave]);
    assert_eq!(l.verdicts([&entry]).verdict(entry.id()), NOT_VALID);
}

#[test]
fn a_statement_whose_signature_does_not_verify_is_not_valid() {
    let mut l = Lost::new();
    let dave = l.dave.clone();
    let mut forged = statement(CODE, &new_key(), 2);
    forged.signature[0] ^= 1;
    let entry = l.carry(ALICE, forged, &[&dave]);
    assert_eq!(l.verdicts([&entry]).verdict(entry.id()), NOT_VALID);
}

#[test]
fn a_statement_whose_number_is_not_higher_than_every_one_in_its_causal_past_is_not_valid() {
    let mut l = Lost::new();
    let dave = l.dave.clone();
    // Bob's certificate carries number 1.
    let same_as_certificate = l.carry(ALICE, statement(CODE, &new_key(), 1), &[&dave]);
    let second = l.carry(ALICE, statement(CODE, &new_key(), 2), &[&dave]);
    let again = l.carry(DAVE, statement(CODE, &new_key(), 2), &[&second]);
    let lower = l.carry(DAVE, statement(CODE, &old_key(), 1), &[&second]);
    let state = l.verdicts([&same_as_certificate, &second, &again, &lower]);
    assert_eq!(state.verdict(same_as_certificate.id()), NOT_VALID);
    assert_eq!(state.verdict(second.id()), ACCEPTED);
    assert_eq!(state.verdict(again.id()), NOT_VALID);
    assert_eq!(state.verdict(lower.id()), NOT_VALID);
}

#[test]
fn two_concurrent_statements_with_the_same_number_are_both_valid() {
    let mut l = Lost::new();
    let dave = l.dave.clone();
    let one = l.carry(ALICE, statement(CODE, &new_key(), 2), &[&dave]);
    let other = l.carry(DAVE, statement(CODE, &other_account(3), 2), &[&dave]);
    let state = l.verdicts([&one, &other]);
    assert_eq!(state.verdict(one.id()), ACCEPTED);
    assert_eq!(state.verdict(other.id()), ACCEPTED);
}

#[test]
fn an_add_whose_certificate_names_a_superseded_account_key_is_not_valid() {
    let mut l = Lost::new();
    let dave = l.dave.clone();
    let entry = l.carry(DAVE, statement(CODE, &new_key(), 2), &[&dave]);
    let old = l.s.add_certified(
        ALICE,
        CAROL,
        Role::Writer,
        certified(CAROL, &old_key(), CODE, 1),
        &[&entry],
    );
    let new = l.s.add_certified(
        ALICE,
        ERIN,
        Role::Writer,
        certified(ERIN, &new_key(), CODE, 2),
        &[&entry],
    );
    let state = l.verdicts([&entry, &old, &new]);
    assert_eq!(state.verdict(old.id()), NOT_VALID);
    assert_eq!(state.verdict(new.id()), ACCEPTED);
    // A member admitted under the older certificate stays a member.
    assert_eq!(state.role(id(BOB)), Some(Role::Writer));
}

#[test]
fn a_newer_statement_in_a_certificate_supersedes_too() {
    let mut l = Lost::new();
    let dave = l.dave.clone();
    let erin = l.s.add_certified(
        ALICE,
        ERIN,
        Role::Writer,
        certified(ERIN, &new_key(), CODE, 2),
        &[&dave],
    );
    let old = l.s.add_certified(
        ALICE,
        CAROL,
        Role::Writer,
        certified(CAROL, &old_key(), CODE, 1),
        &[&erin],
    );
    let state = l.verdicts([&erin, &old]);
    assert_eq!(state.verdict(erin.id()), ACCEPTED);
    assert_eq!(state.verdict(old.id()), NOT_VALID);
}

#[test]
fn an_add_whose_own_certificate_carries_an_older_statement_is_not_valid() {
    // No member of the space names the old key: Erin's certificate already
    // carries statement 2. A thief's add certifies Carol by the old key,
    // with the old statement 1 in the certificate itself.
    let mut s = Space::new(ALICE);
    let genesis = s.genesis.clone();
    let erin = s.add_certified(
        ALICE,
        ERIN,
        Role::Admin,
        certified(ERIN, &new_key(), CODE, 2),
        &[&genesis],
    );
    let thief = s.add_certified(
        ERIN,
        CAROL,
        Role::Writer,
        certified(CAROL, &old_key(), CODE, 1),
        &[&erin],
    );
    let state = resolve(s.space, [&genesis, &erin, &thief]);
    assert_eq!(state.verdict(erin.id()), ACCEPTED);
    assert_eq!(state.verdict(thief.id()), NOT_VALID);
}

/// A space where Alice (the creator, an admin of [`account`]) adds Bob, a
/// writer of [`account`], and Carol, an admin of the account under
/// recovery code [`CODE`] with the old key, and Bob writes person 1's key.
/// `entry` decides whether Alice carries statement 2 of that account into
/// the space before Bob shreds, signed off by the old key, or concurrently.
fn shred_signed_off_by_the_old_key(entry_before_shred: bool) -> (Option<Verdict>, Option<Verdict>) {
    let mut s = Space::new(ALICE);
    let genesis = s.genesis.clone();
    let bob = s.add(ALICE, BOB, Role::Writer, &[&genesis]);
    let carol = s.add_certified(
        ALICE,
        CAROL,
        Role::Admin,
        certified(CAROL, &old_key(), CODE, 1),
        &[&bob],
    );
    let key = s.person_key(BOB, 1, 0xc1, &[&carol]);
    let entry = s.write(
        ALICE,
        Entry::AccountKeyStatement(statement(CODE, &new_key(), 2)),
        &[&key],
    );
    let shred = match entry_before_shred {
        true => s.shred(BOB, 1, &old_key(), &[&entry]),
        false => s.shred(BOB, 1, &old_key(), &[&key]),
    };
    let state = resolve(s.space, [&genesis, &bob, &carol, &key, &entry, &shred]);
    (
        state.verdict(entry.id()).cloned(),
        state.verdict(shred.id()).cloned(),
    )
}

#[test]
fn a_sign_off_by_an_account_key_superseded_in_the_shreds_causal_past_is_not_valid() {
    let (entry, shred) = shred_signed_off_by_the_old_key(true);
    assert_eq!(entry.as_ref(), ACCEPTED);
    assert_eq!(shred.as_ref(), NOT_VALID);
}

#[test]
fn a_sign_off_is_judged_by_its_shreds_causal_past_only() {
    let (entry, shred) = shred_signed_off_by_the_old_key(false);
    assert_eq!(entry.as_ref(), ACCEPTED);
    assert_eq!(shred.as_ref(), ACCEPTED);
}

/// Bob, a writer of [`account`] whose certificate carries statement 1
/// under [`CODE`], shreds person 1's key, signed off by Carol's account,
/// an admin certified by `carol` without a statement. With `entry`, Alice
/// first carries the statement that `carol` is that recovery key's account
/// key with number 2. The shred's verdict.
fn shred_with_carol(carol_key: &SigningKey, entry: bool) -> Option<Verdict> {
    let mut s = Space::new(ALICE);
    let genesis = s.genesis.clone();
    let bob = s.add_certified(
        ALICE,
        BOB,
        Role::Writer,
        certified(BOB, &account(), CODE, 1),
        &[&genesis],
    );
    let mut before = vec![bob.clone()];
    if entry {
        let carried = s.write(
            ALICE,
            Entry::AccountKeyStatement(statement(CODE, carol_key, 2)),
            &[&bob],
        );
        before.push(carried);
    }
    let last = before.last().unwrap().clone();
    let carol = s.add_certified(
        ALICE,
        CAROL,
        Role::Admin,
        Certificate::sign(carol_key, id(CAROL), MemberKind::Device),
        &[&last],
    );
    let key = s.person_key(BOB, 1, 0xc1, &[&carol]);
    let shred = s.shred(BOB, 1, carol_key, &[&key]);
    let mut all: Vec<&Change> = vec![&genesis];
    all.extend(before.iter());
    all.extend([&carol, &key, &shred]);
    let state = resolve(s.space, all);
    assert_eq!(state.verdict(carol.id()), ACCEPTED);
    state.verdict(shred.id()).cloned()
}

#[test]
fn a_statement_entry_makes_two_account_keys_one_account_for_a_sign_off() {
    let carol = other_account(7);
    assert_eq!(shred_with_carol(&carol, false).as_ref(), ACCEPTED);
    assert_eq!(shred_with_carol(&carol, true).as_ref(), NOT_VALID);
}

#[test]
fn the_state_encodes_its_accepted_statement_entries() {
    let mut l = Lost::new();
    let dave = l.dave.clone();
    let entry = l.carry(DAVE, statement(CODE, &new_key(), 2), &[&dave]);
    let with = l.verdicts([&entry]);
    let without = l.verdicts([]);
    assert_ne!(with.encode(), without.encode());
    let statements: BTreeMap<u64, Vec<[u8; 32]>> = with
        .statements()
        .map(|s| (s.number, vec![s.account_key]))
        .collect();
    assert_eq!(
        statements,
        BTreeMap::from([(2, vec![new_key().verifying_key().to_bytes()])])
    );
}
