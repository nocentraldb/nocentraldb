//! `db.account().identity()` (RFC 0015, "In the SDK"; RFC 0010, "The
//! account's identity"): the account's recovery key in lowercase
//! hexadecimal, public, the same on every device and agent of the account
//! and after a recovery; none for a database waiting to be paired (names
//! as the owner decided them, Carsten, 2026-11-23, NOC-493).

mod support;

use ncdb_core::recovery::RecoveryCode;
use nocentraldb::{Db, OpenOptions, PairKind, PairingAnswer};
use support::{LOCK, TempDir, now};

fn at(dir: &TempDir, name: &str) -> OpenOptions {
    OpenOptions::new(LOCK)
        .path(dir.join(name))
        .key_file(dir.join(&format!("{name}.keys")))
}

fn open(options: OpenOptions) -> Db {
    now(Db::open(options)).unwrap()
}

fn words(db: &Db) -> String {
    now(db.account().recovery_code())
        .unwrap()
        .expect("the account's code")
        .words()
        .join(" ")
}

/// The recovery key that `words` derive, in lowercase hexadecimal.
fn recovery_hex(words: &str) -> String {
    let key = RecoveryCode::parse(words).unwrap().recovery_key().public();
    key.iter().map(|b| format!("{b:02x}")).collect()
}

fn pair(device: &Db, new: &Db, kind: PairKind) {
    let invitation = now(new.invitation()).to_string().parse().unwrap();
    let paired = now(device.account().pair(kind, &invitation)).unwrap();
    let answer: PairingAnswer = paired.answer.to_string().parse().unwrap();
    now(new.account().complete_pairing(&answer)).unwrap();
}

#[test]
fn the_identity_is_the_recovery_key_of_the_accounts_code() {
    let dir = TempDir::new("identity");
    let laptop = open(at(&dir, "laptop"));
    let identity = laptop.account().identity().expect("an account");
    assert_eq!(identity, recovery_hex(&words(&laptop)));
    assert_eq!(identity.len(), 64);
    drop(laptop);
    let laptop = open(at(&dir, "laptop"));
    assert_eq!(laptop.account().identity(), Some(identity));
}

#[test]
fn a_database_waiting_to_be_paired_has_none_until_it_is_paired() {
    let dir = TempDir::new("identity-pairing");
    let laptop = open(at(&dir, "laptop"));
    let phone = open(at(&dir, "phone").pairing());
    let agent = open(at(&dir, "agent").pairing().agent());
    assert_eq!(phone.account().identity(), None);
    assert_eq!(agent.account().identity(), None);
    pair(&laptop, &phone, PairKind::Device);
    pair(&laptop, &agent, PairKind::Agent);
    assert_eq!(phone.account().identity(), laptop.account().identity());
    assert_eq!(agent.account().identity(), laptop.account().identity());
}

#[test]
fn a_recovered_account_keeps_its_identity() {
    let dir = TempDir::new("identity-recover");
    let laptop = open(at(&dir, "laptop"));
    let words = words(&laptop);
    let third = open(at(&dir, "third").recover(&words));
    assert_eq!(third.account().identity(), laptop.account().identity());
}
