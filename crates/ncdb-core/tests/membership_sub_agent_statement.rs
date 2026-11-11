//! A sub-agent's certificate carries the account key statement of its
//! parent's account (RFC 0003, "Member certificate"; RFC 0008, "What this
//! RFC uses from RFC 0003"). The fold accepts a sub-agent's add only when
//! its key 6 matches its parent's: both carry a statement under the same
//! recovery key, or neither carries one.

#[path = "support/space.rs"]
mod space;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::change::Change;
use ncdb_core::keys::RecoveryKey;
use ncdb_core::membership::{
    AccountKeyStatement, Certificate, MemberKind, Rejection, Role, Verdict, resolve,
};

use space::{Space, account, id, key};

const ALICE: u8 = 1;
const BOB: u8 = 2;
const CAROL: u8 = 3;
const DAN: u8 = 4;
const ERIN: u8 = 5;

fn statement(code: u8) -> AccountKeyStatement {
    let recovery = RecoveryKey::from_code(&[code; 16]);
    AccountKeyStatement::sign(&recovery, account().verifying_key().to_bytes(), 1)
}

/// Bob, an agent of the space's account whose certificate carries the
/// statement under recovery code 7, added by Alice after the genesis.
fn with_parent_bob(s: &mut Space) -> (Change, Change) {
    let g = s.genesis.clone();
    let certificate =
        Certificate::sign(&account(), id(BOB), MemberKind::Agent).with_statement(statement(7));
    let bob = s.add_certified(ALICE, BOB, Role::Writer, certificate, &[&g]);
    (g, bob)
}

fn sub_agent_of_bob(subject: u8, statement: Option<AccountKeyStatement>) -> Certificate {
    let account_key = account().verifying_key().to_bytes();
    let certificate = Certificate::sign_sub_agent(&key(BOB), account_key, id(subject));
    match statement {
        Some(statement) => certificate.with_statement(statement),
        None => certificate,
    }
}

const NOT_VALID: Option<&Verdict> = Some(&Verdict::Rejected(Rejection::NotValidWhenWritten));

#[test]
fn a_sub_agent_carrying_its_parents_statement_is_added() {
    let mut s = Space::new(ALICE);
    let (g, bob) = with_parent_bob(&mut s);
    let certificate = sub_agent_of_bob(CAROL, Some(statement(7)));
    let carol = s.add_certified(ALICE, CAROL, Role::Writer, certificate, &[&bob]);
    let state = resolve(s.space, [&g, &bob, &carol]);
    assert_eq!(state.verdict(carol.id()), Some(&Verdict::Accepted));
}

#[test]
fn a_sub_agent_without_a_statement_under_a_parent_with_one_is_not_added() {
    let mut s = Space::new(ALICE);
    let (g, bob) = with_parent_bob(&mut s);
    let dan = s.add_certified(
        ALICE,
        DAN,
        Role::Writer,
        sub_agent_of_bob(DAN, None),
        &[&bob],
    );
    let state = resolve(s.space, [&g, &bob, &dan]);
    assert_eq!(state.verdict(dan.id()), NOT_VALID);
}

#[test]
fn a_sub_agent_carrying_another_recovery_key_is_not_added() {
    // The statement holds and names the same account key, but under a
    // recovery key that is not its parent's.
    let mut s = Space::new(ALICE);
    let (g, bob) = with_parent_bob(&mut s);
    let other = sub_agent_of_bob(ERIN, Some(statement(8)));
    let erin = s.add_certified(ALICE, ERIN, Role::Writer, other, &[&bob]);
    let state = resolve(s.space, [&g, &bob, &erin]);
    assert_eq!(state.verdict(erin.id()), NOT_VALID);
}

#[test]
fn a_sub_agent_carrying_a_statement_under_a_parent_without_one_is_not_added() {
    // Alice, the creator, is an agent whose certificate carries no
    // statement.
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let account_key = account().verifying_key().to_bytes();
    let certificate = Certificate::sign_sub_agent(&key(ALICE), account_key, id(CAROL))
        .with_statement(statement(7));
    let carol = s.add_certified(ALICE, CAROL, Role::Writer, certificate, &[&g]);
    let state = resolve(s.space, [&g, &carol]);
    assert_eq!(state.verdict(carol.id()), NOT_VALID);
}
