//! The accounts a database knows (NOC-168; NOC-154 point 3 A; RFC 0009,
//! "Invitations and members"): once a person confirmed an account's
//! fingerprint in an add, the database remembers the account, and a later
//! add of another member of it needs no fingerprint, also after a reopen.

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so these tests run on both targets (NOC-175).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::atomic::{AtomicU64, Ordering};

use ed25519_dalek::SigningKey;

use super::tests::{now, of_account, space, writer};
use super::{AddMember, Invitation, Role};
#[cfg(not(target_arch = "wasm32"))]
use crate::db::{Db, OpenOptions};
use crate::error::ErrorCode;
#[cfg(not(target_arch = "wasm32"))]
use crate::space::Space;

#[cfg(not(target_arch = "wasm32"))]
const LOCK: &str = include_str!("../../tests/model/model.lock.json");

/// The account of the members the admin adds: another than the admin's.
fn guests() -> SigningKey {
    SigningKey::from_bytes(&[7; 32])
}

/// The options of an add with the fingerprint of `invitation`, which a
/// person compared.
fn confirmed(invitation: &Invitation) -> AddMember {
    AddMember {
        fingerprint: Some(invitation.fingerprint()),
        ..writer()
    }
}

/// The time now, in milliseconds since the Unix epoch, by the clock the
/// SDK reads on every target.
fn millis() -> u64 {
    crate::clock::now_millis()
}

#[test]
fn after_a_confirmed_add_another_member_of_the_account_needs_no_fingerprint() {
    let admin = of_account(1, &SigningKey::from_bytes(&[9; 32]));
    let project = space(&admin);
    let first = now(of_account(2, &guests()).invitation());
    now(project.members().add(&first, confirmed(&first))).unwrap();

    let second = now(of_account(3, &guests()).invitation());
    now(project.members().add(&second, writer())).unwrap();
}

#[test]
fn a_confirmed_account_is_known_in_every_space_of_the_database() {
    let admin = of_account(1, &SigningKey::from_bytes(&[9; 32]));
    let first = now(of_account(2, &guests()).invitation());
    now(space(&admin).members().add(&first, confirmed(&first))).unwrap();

    now(space(&admin).members().add(&first, writer())).unwrap();
}

/// The database keeps the account's key and when it was confirmed, in
/// memory for a database in memory.
#[test]
fn the_database_keeps_the_account_key_and_when_it_was_confirmed() {
    let admin = of_account(1, &SigningKey::from_bytes(&[9; 32]));
    let project = space(&admin);
    let invitation = now(of_account(2, &guests()).invitation());
    // The account is known by its identity, the recovery key its
    // certificate's statement names, which the fingerprint is over.
    let key = invitation.fingerprint().0;

    let before = millis();
    now(project.members().add(&invitation, confirmed(&invitation))).unwrap();
    let after = millis();

    let at = admin.lock().view.account_confirmed_at(&key).unwrap();
    assert!(
        at.is_some_and(|at| (before..=after).contains(&at)),
        "{at:?}"
    );
}

/// The database's own account is known without a confirmation, and is not
/// recorded as one.
#[test]
fn the_databases_own_account_is_not_recorded_as_confirmed() {
    let account = SigningKey::from_bytes(&[9; 32]);
    let admin = of_account(1, &account);
    let project = space(&admin);
    let invitation = now(of_account(2, &account).invitation());
    let options = AddMember {
        fingerprint: Some(invitation.fingerprint()),
        ..writer()
    };
    now(project.members().add(&invitation, options)).unwrap();

    let key = account.verifying_key().to_bytes();
    assert_eq!(admin.lock().view.account_confirmed_at(&key).unwrap(), None);
}

/// An add that is refused records nothing, even with the right
/// fingerprint: the account stays unknown.
#[test]
fn a_refused_add_does_not_confirm_the_account() {
    let account = SigningKey::from_bytes(&[9; 32]);
    let (_, project, member, _) = super::tests::joined(&account, Role::Writer);
    let on_member = now(member.spaces().get(project.id())).unwrap();
    let guest = now(of_account(3, &guests()).invitation());
    let refused = now(on_member.members().add(&guest, confirmed(&guest))).unwrap_err();
    assert_eq!(refused.code(), ErrorCode::NotAllowed);

    let own = now(space(&member).members().add(&guest, writer())).unwrap_err();

    assert_eq!(own.code(), ErrorCode::UnknownAccount);
}

/// A directory of its own, removed when dropped.
#[cfg(not(target_arch = "wasm32"))]
struct Dir(PathBuf);

#[cfg(not(target_arch = "wasm32"))]
impl Dir {
    fn new() -> Dir {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let name = format!("ncdb-known-{}-{n}", std::process::id());
        let dir = Dir(std::env::temp_dir().join(name));
        let _ = std::fs::remove_dir_all(&dir.0);
        dir
    }

    /// The database in this directory.
    fn open(&self) -> Db {
        let options = OpenOptions::new(LOCK)
            .path(self.0.join("data"))
            .key_file(self.0.join("member.key"));
        now(Db::open(options)).expect("the database opens")
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A database at a path needs a file system and a keystore, which the
/// browser's database gets in a later slice (NOC-175).
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn a_confirmed_account_stays_known_after_a_reopen() {
    let dir = Dir::new();
    let admin = dir.open();
    let project = space(&admin).id();
    let first = now(of_account(2, &guests()).invitation());
    let on_admin: Space = now(admin.spaces().get(project)).unwrap();
    now(on_admin.members().add(&first, confirmed(&first))).unwrap();
    drop(on_admin);
    drop(admin);

    let admin = dir.open();
    let on_admin = now(admin.spaces().get(project)).unwrap();
    let second = now(of_account(3, &guests()).invitation());
    now(on_admin.members().add(&second, writer())).unwrap();
}
