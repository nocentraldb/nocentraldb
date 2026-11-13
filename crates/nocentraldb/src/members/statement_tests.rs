//! The SDK carries a newer account key statement into its spaces (RFC
//! 0008, "Account key statement entry"; Carsten, 2026-11-11, NOC-187):
//! given the statement, it writes one entry per space and number, into
//! every space where it is a member and a certificate names the
//! statement's recovery key. What calls it, pairing or recovery, is RFC
//! 0010's ticket (pm, NOC-233).

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so these tests run on both targets (NOC-175).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ed25519_dalek::SigningKey;
use ncdb_core::keys::RecoveryKey;
use ncdb_core::membership::{AccountKeyStatement, Certificate, MemberKind, Resolved, Verdict};

use super::tests::{copy, now, of_account, space};
use super::{AddMember, Role};
use crate::db::Db;
use crate::error::ErrorCode;
use crate::space::Space;

/// The recovery code of the member's account.
const CODE: u8 = 0x51;

fn statement(account: &SigningKey, number: u64) -> AccountKeyStatement {
    let recovery = RecoveryKey::from_code(&[CODE; 16]);
    AccountKeyStatement::sign(&recovery, account.verifying_key().to_bytes(), number)
}

fn resolved(db: &Db, space: &Space) -> Resolved {
    crate::db::lock(&db.inner).spaces[&space.id().0]
        .checks
        .resolved()
        .clone()
}

/// An admin's space, into which the admin adds a member of another
/// account, whose certificate carries statement 1 of that account under
/// [`CODE`]. The member's database, which holds the space, and the space.
fn held(account: &SigningKey) -> (Db, Db, Space) {
    let admin = of_account(1, &SigningKey::from_bytes(&[7; 32]));
    let project = space(&admin);
    let member = of_account(2, account);
    let mut invitation = now(member.invitation());
    invitation.form = crate::members::invitation::Form::Certified(
        Certificate::sign(account, invitation.key.id(), MemberKind::Device)
            .with_statement(statement(account, 1)),
    );
    let options = AddMember {
        role: Role::Writer,
        fingerprint: Some(invitation.fingerprint()),
    };
    now(project.members().add(&invitation, options)).unwrap();
    copy(&admin, &member, &project);
    (admin, member, project)
}

#[test]
fn a_newer_statement_is_written_once_into_each_space_that_names_its_recovery_key() {
    let account = SigningKey::from_bytes(&[9; 32]);
    let (admin, member, project) = held(&account);
    let other = space(&member);
    let newer = statement(&SigningKey::from_bytes(&[10; 32]), 2);

    let written = now(member.carry_statement(&newer)).unwrap();

    // The member's own space names no recovery key; only the project does.
    assert_eq!(written, vec![project.id()]);
    let state = resolved(&member, &project);
    assert_eq!(state.statements().collect::<Vec<_>>(), vec![&newer]);
    assert!(resolved(&member, &other).statements().next().is_none());
    copy(&member, &admin, &project);
    let at_admin = resolved(&admin, &project);
    assert_eq!(at_admin.statements().count(), 1);
    assert!(
        at_admin
            .verdicts()
            .values()
            .all(|verdict| *verdict == Verdict::Accepted)
    );

    // Once per space and number: the same statement again writes nothing.
    assert!(now(member.carry_statement(&newer)).unwrap().is_empty());
    assert_eq!(resolved(&member, &project).statements().count(), 1);
}

#[test]
fn a_statement_the_space_already_holds_or_an_older_one_writes_nothing() {
    let account = SigningKey::from_bytes(&[9; 32]);
    let (_, member, _) = held(&account);
    // The certificate in the space carries number 1 already.
    assert!(
        now(member.carry_statement(&statement(&account, 1)))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn a_statement_whose_signature_does_not_verify_is_refused() {
    let account = SigningKey::from_bytes(&[9; 32]);
    let (_, member, project) = held(&account);
    let mut forged = statement(&SigningKey::from_bytes(&[10; 32]), 2);
    forged.signature[0] ^= 1;

    let refused = now(member.carry_statement(&forged)).unwrap_err();

    assert_eq!(refused.code(), ErrorCode::InvalidValue);
    assert!(resolved(&member, &project).statements().next().is_none());
}
