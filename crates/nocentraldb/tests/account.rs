//! Accounts in the Rust SDK (RFC 0010, "The recovery code" and "Adding a
//! device or an agent to an account"; RFC 0009, "Accounts and devices";
//! the owner's NOC-273 A, Carsten, 2026-11-12): the account's first open
//! hands the app its recovery code until `confirm()`, and a device of the
//! account pairs a new device or an agent by text, with `pair` and
//! `complete_pairing`.

mod support;

use nocentraldb::{Db, ErrorCode, NoticeCode, OpenOptions, PairKind, PairingAnswer, RecoveryCode};
use support::{LOCK, TempDir, now};

/// A database at `dir` with its keys in a key file there.
fn at(dir: &TempDir, name: &str) -> OpenOptions {
    OpenOptions::new(LOCK)
        .path(dir.join(name))
        .key_file(dir.join(&format!("{name}.keys")))
}

fn open(options: OpenOptions) -> Db {
    now(Db::open(options)).unwrap()
}

fn code(db: &Db) -> Option<RecoveryCode> {
    now(db.account().recovery_code()).unwrap()
}

/// Pairs `new` with the account of `device` by text, as kind `kind`, and
/// returns the notices of `complete_pairing`.
fn pair(device: &Db, new: &Db, kind: PairKind) -> Vec<NoticeCode> {
    let invitation = now(new.invitation()).to_string().parse().unwrap();
    let paired = now(device.account().pair(kind, &invitation)).unwrap();
    let answer: PairingAnswer = paired.answer.to_string().parse().unwrap();
    now(new.account().complete_pairing(&answer))
        .unwrap()
        .into_iter()
        .map(|notice| notice.code)
        .collect()
}

#[test]
fn the_first_open_hands_the_recovery_code_until_it_is_confirmed() {
    let dir = TempDir::new("recovery-code");
    let words = {
        let db = open(at(&dir, "laptop"));
        let code = code(&db).expect("the first open creates the code");
        let words = code.words();
        assert_eq!(words.len(), 12);
        words
    };
    // Every open shows it again until the person confirms they stored it.
    let db = open(at(&dir, "laptop"));
    let code = code(&db).expect("the code stays until confirm()");
    assert_eq!(code.words(), words);
    now(code.confirm()).unwrap();
    assert!(self::code(&db).is_none());
    drop(db);
    let db = open(at(&dir, "laptop"));
    assert!(
        self::code(&db).is_none(),
        "confirm() forgets the code for good"
    );
    // The key file no longer holds the words, and never held them as text.
    let keys = std::fs::read(dir.join("laptop.keys")).unwrap();
    assert!(!String::from_utf8_lossy(&keys).contains(words[0]));
}

#[test]
fn the_recovery_code_is_redacted_in_display_and_debug() {
    let db = open(OpenOptions::new(LOCK).in_memory());
    let code = code(&db).unwrap();
    assert_eq!(format!("{code}"), "RecoveryCode(redacted)");
    assert_eq!(format!("{code:?}"), "RecoveryCode(redacted)");
    for word in code.words() {
        assert!(!format!("{code} {code:?}").contains(word));
    }
}

#[test]
fn a_database_opened_for_pairing_has_no_recovery_code_and_cannot_pair() {
    let dir = TempDir::new("pairing-no-code");
    let new = open(at(&dir, "phone").pairing());
    assert!(code(&new).is_none());
    let other = open(at(&dir, "agent").pairing().agent());
    let invitation = now(other.invitation());
    let refused = now(new.account().pair(PairKind::Agent, &invitation)).unwrap_err();
    assert_eq!(refused.code(), ErrorCode::NotAllowed);
}

#[test]
fn an_agent_paired_by_text_gets_a_certificate_and_no_account_key() {
    let dir = TempDir::new("pair-agent");
    let laptop = open(at(&dir, "laptop"));
    let agent = open(at(&dir, "agent").pairing().agent());
    let invitation = now(agent.invitation());
    let paired = now(laptop.account().pair(PairKind::Agent, &invitation)).unwrap();
    assert!(paired.added.is_empty() && paired.ask_admins.is_empty());
    let answer: PairingAnswer = paired.answer.to_string().parse().unwrap();
    let notices = now(agent.account().complete_pairing(&answer)).unwrap();
    assert_eq!(
        notices.iter().map(|notice| notice.code).collect::<Vec<_>>(),
        [NoticeCode::AgentPaired]
    );
    // It acts for the laptop's account...
    assert_eq!(
        now(agent.invitation()).fingerprint(),
        now(laptop.invitation()).fingerprint()
    );
    // ...holds no account key, so it pairs nobody, also after a reopen.
    drop(agent);
    let agent = open(at(&dir, "agent"));
    let other = open(at(&dir, "other").pairing().agent());
    let refused = now(agent
        .account()
        .pair(PairKind::Agent, &now(other.invitation())))
    .unwrap_err();
    assert_eq!(refused.code(), ErrorCode::NotAllowed);
    assert!(code(&agent).is_none());
}

#[test]
fn a_device_paired_by_text_holds_the_account_key() {
    let dir = TempDir::new("pair-device");
    let laptop = open(at(&dir, "laptop"));
    let phone = open(at(&dir, "phone").pairing());
    assert_eq!(
        pair(&laptop, &phone, PairKind::Device),
        [NoticeCode::DevicePaired]
    );
    assert_eq!(
        now(phone.invitation()).fingerprint(),
        now(laptop.invitation()).fingerprint()
    );
    // The code stays on the device that created the account.
    assert!(code(&phone).is_none());
    drop(phone);
    // After a reopen, the phone holds the account key: it pairs an agent.
    let phone = open(at(&dir, "phone"));
    let agent = open(at(&dir, "agent").pairing().agent());
    assert_eq!(
        pair(&phone, &agent, PairKind::Agent),
        [NoticeCode::AgentPaired]
    );
    assert_eq!(
        now(agent.invitation()).fingerprint(),
        now(laptop.invitation()).fingerprint()
    );
}

#[test]
fn an_answer_for_another_database_is_refused_and_changes_nothing() {
    let dir = TempDir::new("pair-other");
    let laptop = open(at(&dir, "laptop"));
    let phone = open(at(&dir, "phone").pairing());
    let thief = open(at(&dir, "thief").pairing());
    let paired = now(laptop
        .account()
        .pair(PairKind::Device, &now(phone.invitation())))
    .unwrap();
    let refused = now(thief.account().complete_pairing(&paired.answer)).unwrap_err();
    assert_eq!(refused.code(), ErrorCode::InvalidCertificate);
    // The thief is still no device of the account.
    assert_ne!(
        now(thief.invitation()).fingerprint(),
        now(laptop.invitation()).fingerprint()
    );
    // The phone still completes it.
    let notices = now(phone.account().complete_pairing(&paired.answer)).unwrap();
    assert_eq!(notices[0].code, NoticeCode::DevicePaired);
}

#[test]
fn a_text_that_is_not_a_pairing_answer_is_invalid() {
    for text in ["", "ncdb-pair-1:", "ncdb-invite-1:AAAA", "ncdb-pair-1:!!"] {
        let refused = text.parse::<PairingAnswer>().unwrap_err();
        assert_eq!(refused.code(), ErrorCode::InvalidValue, "{text}");
    }
}

#[test]
fn a_database_waiting_to_be_paired_shows_version_2_and_joins_no_space() {
    let dir = TempDir::new("waiting");
    let new = open(at(&dir, "phone").pairing());
    let invitation = now(new.invitation());
    let text = invitation.to_string();
    // 66 bytes, as unpadded base64url after the prefix.
    assert_eq!(text.len(), "ncdb-invite-1:".len() + 88);
    assert_eq!(text.parse::<nocentraldb::Invitation>().unwrap(), invitation);
    // It creates no space, and an admin cannot add it to one.
    let refused = now(new.spaces().create()).unwrap_err();
    assert_eq!(refused.code(), ErrorCode::NotAllowed);
    let admin = open(at(&dir, "laptop"));
    let project = now(admin.spaces().create()).unwrap();
    let options = nocentraldb::AddMember {
        role: nocentraldb::Role::Writer,
        fingerprint: Some(invitation.fingerprint()),
    };
    let refused = now(project.members().add(&invitation, options)).unwrap_err();
    assert_eq!(refused.code(), ErrorCode::InvalidCertificate);
}

#[test]
fn an_invitation_that_asks_for_the_other_kind_is_invalid() {
    let dir = TempDir::new("other-kind");
    let laptop = open(at(&dir, "laptop"));
    let agent = open(at(&dir, "agent").pairing().agent());
    let refused = now(laptop
        .account()
        .pair(PairKind::Device, &now(agent.invitation())))
    .unwrap_err();
    assert_eq!(refused.code(), ErrorCode::InvalidValue);
}

#[test]
fn a_paired_device_creates_spaces_under_the_accounts_certificate() {
    let dir = TempDir::new("paired-space");
    let laptop = open(at(&dir, "laptop"));
    let phone = open(at(&dir, "phone").pairing());
    assert_eq!(
        pair(&laptop, &phone, PairKind::Device),
        [NoticeCode::DevicePaired]
    );
    let project = now(phone.spaces().create()).unwrap();
    // The laptop's account is the phone's: it adds the laptop without a
    // fingerprint.
    let options = nocentraldb::AddMember {
        role: nocentraldb::Role::Writer,
        fingerprint: None,
    };
    now(project.members().add(&now(laptop.invitation()), options)).unwrap();
}

/// The words of `db`'s recovery code, as the person stored them.
fn words(db: &Db) -> String {
    code(db).expect("the account's code").words().join(" ")
}

#[test]
fn recovery_on_a_third_device_keeps_the_fingerprint_and_joins_no_space() {
    let dir = TempDir::new("recover");
    let laptop = open(at(&dir, "laptop"));
    let words = words(&laptop);
    now(code(&laptop).unwrap().confirm()).unwrap();
    now(laptop.spaces().create()).unwrap();

    let third = open(at(&dir, "third").recover(&words));
    assert_eq!(
        now(third.invitation()).fingerprint(),
        now(laptop.invitation()).fingerprint()
    );
    assert_eq!(
        now(third.account().recovered())
            .unwrap()
            .map(|notice| notice.code),
        Some(NoticeCode::AccountRecovered)
    );
    assert!(now(third.spaces().list()).unwrap().is_empty());
    // The person holds the code; the device does not hand it again.
    assert!(code(&third).is_none());
    // It holds the new account key: it pairs an agent of the account.
    let agent = open(at(&dir, "agent").pairing().agent());
    assert_eq!(
        pair(&third, &agent, PairKind::Agent),
        [NoticeCode::AgentPaired]
    );
    assert_eq!(
        now(agent.invitation()).fingerprint(),
        now(laptop.invitation()).fingerprint()
    );
    // An ordinary open recovered nothing.
    assert_eq!(now(laptop.account().recovered()).unwrap(), None);
    drop(third);
    let third = open(at(&dir, "third"));
    assert_eq!(now(third.account().recovered()).unwrap(), None);
}

#[test]
fn words_typed_in_any_case_and_spacing_recover() {
    let dir = TempDir::new("recover-typed");
    let laptop = open(at(&dir, "laptop"));
    let typed = format!("  {}\n", words(&laptop).to_uppercase().replace(' ', "   "));
    let third = open(at(&dir, "third").recover(&typed));
    assert_eq!(
        now(third.invitation()).fingerprint(),
        now(laptop.invitation()).fingerprint()
    );
}

#[test]
fn words_that_are_no_code_are_invalid_and_create_nothing() {
    let dir = TempDir::new("recover-invalid");
    let laptop = open(at(&dir, "laptop"));
    let words = words(&laptop);
    let mut swapped: Vec<&str> = words.split(' ').collect();
    swapped.swap(0, 1);
    for bad in [
        "",
        "abandon abandon abandon",
        &words.replacen(words.split(' ').next().unwrap(), "notaword", 1),
        &swapped.join(" "),
    ] {
        let refused = now(Db::open(at(&dir, "third").recover(bad))).unwrap_err();
        assert_eq!(refused.code(), ErrorCode::InvalidValue, "{bad}");
        assert!(!dir.join("third").exists(), "nothing is created");
        assert!(!dir.join("third.keys").exists(), "no key file is created");
        for word in bad.split_whitespace() {
            assert!(!refused.to_string().contains(word), "{refused}");
        }
    }
}

#[test]
fn recovery_does_not_open_a_database_that_exists() {
    let dir = TempDir::new("recover-existing");
    let laptop = open(at(&dir, "laptop"));
    let words = words(&laptop);
    drop(open(at(&dir, "phone")));
    let refused = now(Db::open(at(&dir, "phone").recover(&words))).unwrap_err();
    assert_eq!(refused.code(), ErrorCode::InvalidValue);
}

#[test]
fn an_agent_with_an_account_of_its_own_is_not_linked_yet() {
    let dir = TempDir::new("link-agent");
    let laptop = open(at(&dir, "laptop"));
    let agent = open(at(&dir, "agent").agent());
    let refused = now(laptop
        .account()
        .pair(PairKind::Agent, &now(agent.invitation())))
    .unwrap_err();
    assert_eq!(refused.code(), ErrorCode::NotAllowed);
}
