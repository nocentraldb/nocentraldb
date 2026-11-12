//! The pairing answer's vectors (RFC 0010, "The pairing answer"):
//! docs/rfc/vectors/0010/pairing_answer.py writes them with its own HPKE,
//! Ed25519 and canonical CBOR, from the inputs of
//! tests/vectors/key_envelope.py. The core builds the same text from the
//! same inputs, and reads and completes it.

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ed25519_dalek::SigningKey;
use ncdb_core::keys::{MemberKey, RecoveryKey};
use ncdb_core::membership::{AccountKeyStatement, MemberKind};
use ncdb_core::pairing::PairingAnswer;

const DEVICE: &str = include_str!("../../../docs/rfc/vectors/0010/pairing-answer.device.txt");
const AGENT: &str = include_str!("../../../docs/rfc/vectors/0010/pairing-answer.agent.txt");
const DEVICE_LINK: &str =
    include_str!("../../../docs/rfc/vectors/0010/pairing-answer.device-link.txt");

fn recovery() -> RecoveryKey {
    RecoveryKey::from_code(&[0x42; 16])
}

fn account() -> SigningKey {
    SigningKey::from_bytes(&[0x11; 32])
}

fn old_account() -> [u8; 32] {
    SigningKey::from_bytes(&[0x33; 32])
        .verifying_key()
        .to_bytes()
}

/// The key envelope vector's recipient.
fn member() -> MemberKey {
    MemberKey::from_secrets(
        std::array::from_fn(|i| 0x10 + i as u8),
        std::array::from_fn(|i| 0x60 + i as u8),
    )
}

fn ikm() -> [u8; 32] {
    std::array::from_fn(|i| 0xb0 + i as u8)
}

fn statement() -> AccountKeyStatement {
    AccountKeyStatement::sign(&recovery(), account().verifying_key().to_bytes(), 2)
}

fn build(kind: MemberKind, old: Option<[u8; 32]>) -> String {
    PairingAnswer::build_with_ephemeral(
        &account(),
        &statement(),
        &member().public(),
        kind,
        old,
        ikm(),
    )
    .unwrap()
    .to_string()
}

#[test]
fn the_core_builds_the_vectors_text() {
    assert_eq!(build(MemberKind::Device, None), DEVICE.trim_end());
    assert_eq!(build(MemberKind::Agent, None), AGENT.trim_end());
    assert_eq!(
        build(MemberKind::Device, Some(old_account())),
        DEVICE_LINK.trim_end()
    );
}

#[test]
fn the_vectors_complete_on_their_member() {
    for (text, kind, old) in [
        (DEVICE, MemberKind::Device, None),
        (AGENT, MemberKind::Agent, None),
        (DEVICE_LINK, MemberKind::Device, Some(old_account())),
    ] {
        let answer: PairingAnswer = text.trim_end().parse().unwrap();
        let paired = answer.complete(&member(), old.as_ref()).unwrap();
        assert_eq!(paired.certificate.member, member().id());
        assert_eq!(paired.certificate.kind, kind);
        assert_eq!(paired.certificate.statement.as_deref(), Some(&statement()));
        assert_eq!(
            paired.account.map(|key| key.to_bytes()),
            (kind == MemberKind::Device).then(|| account().to_bytes())
        );
        assert_eq!(paired.link.is_some(), old.is_some());
    }
}
