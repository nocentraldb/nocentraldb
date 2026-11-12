//! The pairing answer (RFC 0010, "Adding a device or an agent to an
//! account" and "The pairing answer"; RFC 0003, "Pairing answer"; the
//! owner's NOC-273 A, Carsten, 2026-11-12): a device of the account builds
//! it for a new device or an agent, and the new member reads and completes
//! it. It carries the certificate the account key signed, the account key
//! statement, for a device the account key sealed with HPKE under
//! `"nocentraldb/account-key"`, and when the invitation carried another
//! account's certificate the account key's half of the link statement
//! (`"nocentraldb/account-link"`).

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ed25519_dalek::{Signer, SigningKey};
use ncdb_core::keys::{MemberKey, RecoveryKey};
use ncdb_core::membership::{AccountKeyStatement, MemberKind};
use ncdb_core::pairing::{self, CheckError, PairingAnswer, ReadError};

/// The account's recovery key, account key and statement number 2.
fn recovery() -> RecoveryKey {
    RecoveryKey::from_code(&[0x42; 16])
}

fn account() -> SigningKey {
    SigningKey::from_bytes(&[0x11; 32])
}

fn statement() -> AccountKeyStatement {
    AccountKeyStatement::sign(&recovery(), account().verifying_key().to_bytes(), 2)
}

/// The new member.
fn member() -> MemberKey {
    MemberKey::from_secrets([0x21; 32], [0x22; 32])
}

/// The per-database account the new member drew before it pairs.
fn old_account() -> SigningKey {
    SigningKey::from_bytes(&[0x33; 32])
}

fn answer(kind: MemberKind) -> PairingAnswer {
    PairingAnswer::build(&account(), &statement(), &member().public(), kind, None).unwrap()
}

fn text(kind: MemberKind) -> String {
    answer(kind).to_string()
}

fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, b)| n | u32::from(*b) << (16 - 8 * i));
        for i in 0..=chunk.len() {
            out.push(ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char);
        }
    }
    out
}

/// The answer's bytes.
fn bytes(kind: MemberKind) -> Vec<u8> {
    answer(kind).to_bytes()
}

/// Reads mutated bytes through the text form, as a new member reads them.
fn from_bytes(bytes: &[u8]) -> Result<PairingAnswer, ReadError> {
    format!("ncdb-pair-1:{}", base64(bytes)).parse()
}

#[test]
fn the_text_is_the_prefix_and_unpadded_base64url_of_the_bytes() {
    for kind in [MemberKind::Device, MemberKind::Agent] {
        let answer = answer(kind);
        assert_eq!(
            answer.to_string(),
            format!("ncdb-pair-1:{}", base64(&answer.to_bytes()))
        );
        assert_eq!(from_bytes(&answer.to_bytes()).unwrap(), answer);
        assert_eq!(
            PairingAnswer::from_bytes(&answer.to_bytes()).unwrap(),
            answer
        );
    }
}

#[test]
fn a_device_completes_its_answer_with_the_certificate_the_statement_and_the_account_key() {
    let answer: PairingAnswer = text(MemberKind::Device).parse().unwrap();
    assert_eq!(answer.kind(), MemberKind::Device);
    let paired = answer.complete(&member(), None).unwrap();
    assert_eq!(paired.certificate.member, member().id());
    assert_eq!(paired.certificate.kind, MemberKind::Device);
    assert_eq!(
        paired.certificate.account_key,
        account().verifying_key().to_bytes()
    );
    assert_eq!(paired.certificate.statement.as_deref(), Some(&statement()));
    assert!(
        paired
            .certificate
            .verify(&account().verifying_key().to_bytes())
    );
    assert_eq!(
        paired.account.map(|key| key.to_bytes()),
        Some(account().to_bytes())
    );
}

#[test]
fn an_answer_for_an_agent_carries_no_account_key() {
    let answer: PairingAnswer = text(MemberKind::Agent).parse().unwrap();
    assert_eq!(answer.kind(), MemberKind::Agent);
    let paired = answer.complete(&member(), None).unwrap();
    assert_eq!(paired.certificate.kind, MemberKind::Agent);
    assert!(paired.account.is_none());
    assert_eq!(bytes(MemberKind::Agent)[202], 0);
}

#[test]
fn a_device_answer_is_284_bytes_and_an_agents_204_and_64_more_with_a_link() {
    assert_eq!(bytes(MemberKind::Device).len(), 284);
    assert_eq!(bytes(MemberKind::Agent).len(), 204);
    let link = PairingAnswer::build(
        &account(),
        &statement(),
        &member().public(),
        MemberKind::Device,
        Some(old_account().verifying_key().to_bytes()),
    )
    .unwrap();
    let text = link.to_string();
    assert!(text.starts_with("ncdb-pair-1:"));
    assert_eq!(
        text.len(),
        "ncdb-pair-1:".len() + (348usize * 4).div_ceil(3)
    );
}

#[test]
fn the_layout_holds_the_fields_at_their_offsets() {
    let bytes = bytes(MemberKind::Device);
    let statement = statement();
    assert_eq!(bytes[0], 1, "version");
    assert_eq!(bytes[1], 1, "kind: a device");
    assert_eq!(bytes[2..34], account().verifying_key().to_bytes());
    assert_eq!(bytes[98..130], statement.recovery_key);
    assert_eq!(bytes[130..138], 2u64.to_be_bytes());
    assert_eq!(bytes[138..202], statement.signature);
    assert_eq!(bytes[202], 1, "a sealed key follows");
    assert_eq!(bytes[283], 0, "no link");
}

#[test]
fn an_answer_for_another_member_id_is_refused() {
    let other = MemberKey::from_secrets([0x51; 32], [0x52; 32]);
    let answer: PairingAnswer = text(MemberKind::Agent).parse().unwrap();
    assert_eq!(
        answer.complete(&other, None).err(),
        Some(CheckError::Certificate)
    );
    // A device's answer sealed to another member is refused as well.
    let answer: PairingAnswer = text(MemberKind::Device).parse().unwrap();
    assert!(answer.complete(&other, None).is_err());
}

#[test]
fn an_answer_whose_statement_does_not_verify_is_refused() {
    let mut bytes = bytes(MemberKind::Agent);
    bytes[140] ^= 1;
    let answer = from_bytes(&bytes).unwrap();
    assert_eq!(
        answer.complete(&member(), None).err(),
        Some(CheckError::Statement)
    );
    // Another recovery key with the same signature does not verify either.
    let mut bytes = self::bytes(MemberKind::Agent);
    bytes[98] ^= 1;
    let answer = from_bytes(&bytes).unwrap();
    assert_eq!(
        answer.complete(&member(), None).err(),
        Some(CheckError::Statement)
    );
    // Nor does another number.
    let mut bytes = self::bytes(MemberKind::Agent);
    bytes[137] = 3;
    let answer = from_bytes(&bytes).unwrap();
    assert_eq!(
        answer.complete(&member(), None).err(),
        Some(CheckError::Statement)
    );
}

#[test]
fn a_tampered_certificate_is_refused() {
    // A flipped bit in the certificate's signature.
    let mut bytes = bytes(MemberKind::Device);
    bytes[40] ^= 1;
    let answer = from_bytes(&bytes).unwrap();
    assert_eq!(
        answer.complete(&member(), None).err(),
        Some(CheckError::Certificate)
    );
}

#[test]
fn a_device_answer_relabelled_as_an_agent_or_back_is_refused() {
    // An agent's answer whose kind byte says device fails the layout: a
    // device's answer carries a sealed key.
    let mut agent = bytes(MemberKind::Agent);
    agent[1] = 1;
    assert_eq!(from_bytes(&agent).err(), Some(ReadError::SealedKey));
    // A device's answer cut down to an agent's: the certificate was signed
    // for a device, so it does not verify as an agent's.
    let device = bytes(MemberKind::Device);
    let mut relabelled = device[..202].to_vec();
    relabelled[1] = 2;
    relabelled.extend([0, 0]);
    let answer = from_bytes(&relabelled).unwrap();
    assert_eq!(
        answer.complete(&member(), None).err(),
        Some(CheckError::Certificate)
    );
}

#[test]
fn an_answer_signed_by_another_account_key_than_its_statement_names_is_refused() {
    // The statement names the account key, but another key signed the
    // certificate: swap in a certificate signature by another key.
    let other = SigningKey::from_bytes(&[0x77; 32]);
    let swapped = PairingAnswer::build(
        &other,
        &AccountKeyStatement::sign(&recovery(), other.verifying_key().to_bytes(), 2),
        &member().public(),
        MemberKind::Agent,
        None,
    )
    .unwrap();
    let mut bytes = bytes(MemberKind::Agent);
    let other_answer = swapped.to_bytes();
    bytes[34..98].copy_from_slice(&other_answer[34..98]);
    let answer = from_bytes(&bytes).unwrap();
    assert_eq!(
        answer.complete(&member(), None).err(),
        Some(CheckError::Certificate)
    );
}

#[test]
fn a_sealed_key_that_does_not_open_or_does_not_match_the_statement_is_refused() {
    // A flipped bit in the ciphertext: it does not open.
    let mut bytes = bytes(MemberKind::Device);
    bytes[250] ^= 1;
    let answer = from_bytes(&bytes).unwrap();
    assert_eq!(
        answer.complete(&member(), None).err(),
        Some(CheckError::AccountKey)
    );
    // Another account key sealed to the member: it opens, but is not the
    // one the statement names.
    let other = SigningKey::from_bytes(&[0x77; 32]);
    let sealed = pairing::seal_account_key(&other, &member().public()).unwrap();
    let mut bytes = self::bytes(MemberKind::Device);
    bytes[203..235].copy_from_slice(&sealed.encapsulated_key);
    bytes[235..283].copy_from_slice(&sealed.ciphertext);
    let answer = from_bytes(&bytes).unwrap();
    assert_eq!(
        answer.complete(&member(), None).err(),
        Some(CheckError::AccountKey)
    );
}

#[test]
fn a_link_signature_completes_only_for_the_old_account_it_names() {
    let old = old_account().verifying_key().to_bytes();
    let answer = PairingAnswer::build(
        &account(),
        &statement(),
        &member().public(),
        MemberKind::Device,
        Some(old),
    )
    .unwrap();
    let answer: PairingAnswer = answer.to_string().parse().unwrap();
    let paired = answer.complete(&member(), Some(&old)).unwrap();
    // The account key signed H("nocentraldb/account-link" || {1: old, 2: recovery}).
    let digest = pairing::link_digest(&old, &recovery().public());
    assert_eq!(
        paired.link.map(|signature| signature.to_vec()),
        Some(account().sign(&digest).to_bytes().to_vec())
    );
    // For another old account, or none, it is refused.
    let another = SigningKey::from_bytes(&[0x44; 32])
        .verifying_key()
        .to_bytes();
    assert_eq!(
        answer.complete(&member(), Some(&another)).err(),
        Some(CheckError::Link)
    );
    assert_eq!(
        answer.complete(&member(), None).err(),
        Some(CheckError::Link)
    );
    // An answer without a link, to a member that has an old account.
    let plain: PairingAnswer = text(MemberKind::Device).parse().unwrap();
    assert_eq!(
        plain.complete(&member(), Some(&old)).err(),
        Some(CheckError::Link)
    );
}

#[test]
fn the_link_context_and_input_are_the_rfcs() {
    assert_eq!(pairing::LINK_CONTEXT, b"nocentraldb/account-link");
    assert_eq!(pairing::ANSWER_PREFIX, "ncdb-pair-1:");
}

#[test]
fn a_text_that_is_not_an_answer_is_refused_and_nothing_is_read() {
    let good = bytes(MemberKind::Device);
    let cases: Vec<(String, ReadError)> = vec![
        (
            text(MemberKind::Device).replacen("ncdb-pair-1:", "ncdb-pair-2:", 1),
            ReadError::Prefix,
        ),
        (
            text(MemberKind::Device).replacen("ncdb-pair-1:", "", 1),
            ReadError::Prefix,
        ),
        (
            format!("{}=", text(MemberKind::Device)),
            ReadError::Encoding,
        ),
        (format!("{}!", text(MemberKind::Agent)), ReadError::Encoding),
        (
            format!("ncdb-pair-1:{}", base64(&[&[2u8][..], &good[1..]].concat())),
            ReadError::Version,
        ),
        (
            format!(
                "ncdb-pair-1:{}",
                base64(&[&good[..1], &[3u8], &good[2..]].concat())
            ),
            ReadError::Kind,
        ),
        (
            format!(
                "ncdb-pair-1:{}",
                base64(&[&good[..1], &[0u8], &good[2..]].concat())
            ),
            ReadError::Kind,
        ),
        (
            format!("ncdb-pair-1:{}", base64(&good[..283])),
            ReadError::Length,
        ),
        (
            format!("ncdb-pair-1:{}", base64(&[&good[..], &[0u8][..]].concat())),
            ReadError::Length,
        ),
        (
            format!(
                "ncdb-pair-1:{}",
                base64(&[&good[..283], &[2u8][..]].concat())
            ),
            ReadError::Link,
        ),
        (
            format!(
                "ncdb-pair-1:{}",
                base64(&[&good[..283], &[1u8][..], &[0u8; 63][..]].concat())
            ),
            ReadError::Length,
        ),
        (
            format!(
                "ncdb-pair-1:{}",
                base64(&[&good[..202], &[0u8][..], &good[283..]].concat())
            ),
            ReadError::SealedKey,
        ),
        (
            format!(
                "ncdb-pair-1:{}",
                base64(&[&good[..130], &[0u8; 8][..], &good[138..]].concat())
            ),
            ReadError::Number,
        ),
        ("ncdb-pair-1:".to_owned(), ReadError::Length),
    ];
    for (text, expected) in cases {
        assert_eq!(
            text.parse::<PairingAnswer>().err(),
            Some(expected),
            "{text}"
        );
    }
}

#[test]
fn building_an_answer_for_a_sub_agent_or_with_a_statement_for_another_key_fails() {
    assert!(
        PairingAnswer::build(
            &account(),
            &statement(),
            &member().public(),
            MemberKind::SubAgent,
            None
        )
        .is_err()
    );
    let other = SigningKey::from_bytes(&[0x77; 32]);
    let wrong = AccountKeyStatement::sign(&recovery(), other.verifying_key().to_bytes(), 2);
    assert!(
        PairingAnswer::build(
            &account(),
            &wrong,
            &member().public(),
            MemberKind::Agent,
            None
        )
        .is_err()
    );
}

#[test]
fn no_key_bytes_reach_debug_or_errors() {
    let answer = PairingAnswer::build(
        &account(),
        &statement(),
        &member().public(),
        MemberKind::Device,
        Some(old_account().verifying_key().to_bytes()),
    )
    .unwrap();
    let paired = answer
        .complete(&member(), Some(&old_account().verifying_key().to_bytes()))
        .unwrap();
    let shown = [
        format!("{answer:?}"),
        format!("{paired:?}"),
        format!("{}", CheckError::AccountKey),
        format!("{}", ReadError::Length),
    ];
    let secrets: Vec<Vec<u8>> = vec![
        account().to_bytes().to_vec(),
        account().verifying_key().to_bytes().to_vec(),
        member().public().agreement.to_vec(),
        recovery().public().to_vec(),
    ];
    for text in &shown {
        assert!(!text.contains(&answer.to_string()[12..24]), "{text}");
        for secret in &secrets {
            let hex: String = secret.iter().map(|b| format!("{b:02x}")).collect();
            assert!(!text.contains(&hex), "{text}");
            assert!(!text.contains(&base64(secret)[..16]), "{text}");
            assert!(!text.contains(&format!("{:?}", &secret[..8])), "{text}");
        }
    }
}
