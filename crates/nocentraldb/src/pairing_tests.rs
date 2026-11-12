//! What accounts add to the SDK that stores nothing (RFC 0009, "Accounts
//! and devices"; the owner's NOC-273 A, Carsten, 2026-11-12): the recovery
//! code's words and its redacted forms, the pairing answer's text, the
//! errors of a pairing and the notices of accounts.

use ed25519_dalek::SigningKey;
use ncdb_core::keys::{MemberKey, RecoveryKey};
use ncdb_core::membership::{AccountKeyStatement, MemberKind};
use ncdb_core::recovery::RecoveryCode as CoreCode;

use super::{PairingAnswer, RecoveryCode, check_error};
use crate::error::ErrorCode;
use crate::members::{Notice, NoticeCode};

fn answer() -> PairingAnswer {
    let recovery = RecoveryKey::from_code(&[0x42; 16]);
    let account = SigningKey::from_bytes(&[0x11; 32]);
    let statement =
        AccountKeyStatement::sign(&recovery, account.verifying_key().to_bytes(), 1);
    let member = MemberKey::from_secrets([0x21; 32], [0x22; 32]);
    PairingAnswer(
        ncdb_core::pairing::PairingAnswer::build(
            &account,
            &statement,
            &member.public(),
            MemberKind::Agent,
            None,
        )
        .unwrap(),
    )
}

#[test]
fn the_recovery_codes_words_are_the_twelve_words_of_the_list() {
    let core = CoreCode::from_bits([0x7f; 16]);
    let code = RecoveryCode::new(core.clone());
    let words = code.words();
    assert_eq!(words.join(" "), *core.words());
    let list: Vec<&str> = ncdb_core::recovery::WORD_LIST.lines().collect();
    assert!(words.iter().all(|word| list.contains(word)));
}

#[test]
fn the_recovery_code_shows_as_redacted() {
    let code = RecoveryCode::new(CoreCode::from_bits([0x7f; 16]));
    assert_eq!(code.to_string(), "RecoveryCode(redacted)");
    assert_eq!(format!("{code:?}"), "RecoveryCode(redacted)");
    assert_eq!(format!("{code:#?}"), "RecoveryCode(redacted)");
}

#[test]
fn a_pairing_answer_reads_back_from_its_text() {
    let answer = answer();
    let text = answer.to_string();
    assert!(text.starts_with("ncdb-pair-1:"));
    assert_eq!(text.parse::<PairingAnswer>().unwrap(), answer);
}

#[test]
fn a_text_that_is_not_a_pairing_answer_is_invalid_and_names_no_byte() {
    let text = answer().to_string();
    for bad in [
        String::new(),
        "ncdb-pair-1:".to_owned(),
        text.replacen("ncdb-pair-1:", "ncdb-invite-1:", 1),
        format!("{text}="),
        text[..text.len() - 4].to_owned(),
    ] {
        let refused = bad.parse::<PairingAnswer>().unwrap_err();
        assert_eq!(refused.code(), ErrorCode::InvalidValue, "{bad}");
        assert!(refused.to_string().contains("not a pairing answer"), "{refused}");
        assert!(!refused.to_string().contains(&text[12..30]), "{refused}");
    }
}

#[test]
fn an_answer_that_does_not_hold_is_an_invalid_certificate() {
    use ncdb_core::pairing::CheckError;
    for check in [
        CheckError::Statement,
        CheckError::Certificate,
        CheckError::AccountKey,
        CheckError::Link,
    ] {
        assert_eq!(check_error(check).code(), ErrorCode::InvalidCertificate);
    }
}

#[test]
fn the_notices_of_accounts_carry_the_texts_the_owner_decided() {
    let notices: [(Notice, NoticeCode, &str); 6] = [
        (
            Notice::DEVICE_ADDED,
            NoticeCode::DeviceAdded,
            "Your new device was added to this space and reads its whole history.",
        ),
        (
            Notice::DEVICE_PAIRED,
            NoticeCode::DevicePaired,
            "This device now holds your account key. Keep your recovery code: it is the way back if you lose every device.",
        ),
        (
            Notice::AGENT_PAIRED,
            NoticeCode::AgentPaired,
            "This agent acts for your account in the spaces an admin adds it to. It holds no account key.",
        ),
        (
            Notice::ACCOUNT_LINKED,
            NoticeCode::AccountLinked,
            "Spaces this database joined before keep its old account. Other members see it as linked to yours.",
        ),
        (
            Notice::ACCOUNT_RECOVERED,
            NoticeCode::AccountRecovered,
            "Your account is back with a new account key. Your spaces come back when one of their members adds this device.",
        ),
        (
            Notice::ACCOUNT_KEY_RENEWED,
            NoticeCode::AccountKeyRenewed,
            "Your account has a new account key. Members that have not learnt it yet still accept the old one.",
        ),
    ];
    for (notice, code, text) in notices {
        assert_eq!(notice.code, code);
        assert_eq!(notice.text, text);
    }
}
