//! An invitation's text form (RFC 0003, "Invitation"; the owner's NOC-141
//! A): `Display` writes `ncdb-invite-1:` and unpadded base64url of the
//! layout, `FromStr` reads it, and both hold to the vectors in
//! docs/rfc/vectors/0003, which Python wrote independently of this crate.

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so these tests run on both targets (NOC-175).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::membership::MemberKind;

use super::Invitation;
use super::tests::{database, now};
use crate::error::ErrorCode;

const PLAIN: &str = include_str!("../../../../docs/rfc/vectors/0003/invitation.txt");
const STATEMENT: &str = include_str!("../../../../docs/rfc/vectors/0003/invitation-statement.txt");
const FIELDS: &str = include_str!("../../../../docs/rfc/vectors/0003/fields.txt");
const PLAIN_HEX: &str = include_str!("../../../../docs/rfc/vectors/0003/invitation.hex");
const STATEMENT_HEX: &str =
    include_str!("../../../../docs/rfc/vectors/0003/invitation-statement.hex");

/// The field `name` of fields.txt, as bytes.
fn field(name: &str) -> Vec<u8> {
    let line = FIELDS
        .lines()
        .find(|line| line.starts_with(&format!("{name}: ")))
        .unwrap_or_else(|| panic!("fields.txt has no {name}"));
    from_hex(&line[name.len() + 2..])
}

fn from_hex(text: &str) -> Vec<u8> {
    let digits: String = text.split_whitespace().collect();
    (0..digits.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&digits[i..i + 2], 16).unwrap())
        .collect()
}

/// The text form of `bytes`, written the way RFC 0003 gives it.
fn text_of(bytes: &[u8]) -> String {
    format!(
        "ncdb-invite-1:{}",
        data_encoding::BASE64URL_NOPAD.encode(bytes)
    )
}

#[test]
fn the_vector_without_a_statement_reads_field_by_field() {
    let invitation: Invitation = PLAIN.trim().parse().unwrap();

    assert_eq!(invitation.key.signing.to_vec(), field("signing key"));
    assert_eq!(invitation.key.agreement.to_vec(), field("agreement key"));
    assert_eq!(invitation.key.id().0.to_vec(), field("member id"));
    let certificate = &invitation.certificate;
    assert_eq!(certificate.account_key.to_vec(), field("account key"));
    assert_eq!(certificate.member, invitation.key.id());
    assert_eq!(certificate.kind, MemberKind::Device);
    assert_eq!(certificate.parent, None);
    assert_eq!(
        certificate.signature.to_vec(),
        field("certificate signature")
    );
    assert!(certificate.statement.is_none());
    assert!(certificate.verify(&certificate.account_key));
}

#[test]
fn the_vector_with_a_statement_reads_its_recovery_key_number_and_signature() {
    let invitation: Invitation = STATEMENT.trim().parse().unwrap();

    let statement = invitation.certificate.statement.as_deref().unwrap();
    assert_eq!(statement.recovery_key.to_vec(), field("recovery key"));
    assert_eq!(statement.account_key, invitation.certificate.account_key);
    assert_eq!(statement.number.to_be_bytes().to_vec(), field("number"));
    assert_eq!(statement.signature.to_vec(), field("statement signature"));
    assert!(
        invitation
            .certificate
            .verify(&invitation.certificate.account_key)
    );
}

#[test]
fn both_vectors_are_written_back_as_the_same_text() {
    for text in [PLAIN.trim(), STATEMENT.trim()] {
        let invitation: Invitation = text.parse().unwrap();

        assert_eq!(invitation.to_string(), text);
    }
}

#[test]
fn a_databases_own_invitation_round_trips() {
    let invitation = now(database().invitation());

    let text = invitation.to_string();
    let read: Invitation = text.parse().unwrap();

    assert!(text.starts_with("ncdb-invite-1:"));
    assert_eq!(read, invitation);
    assert_eq!(read.fingerprint(), invitation.fingerprint());
}

/// Every text RFC 0003 refuses is refused with `InvalidValue`.
#[test]
fn a_text_that_does_not_follow_the_layout_is_an_invalid_value() {
    let bytes = from_hex(PLAIN_HEX);
    let body = &PLAIN.trim()["ncdb-invite-1:".len()..];
    let changed = |offset: usize, value: u8| {
        let mut copy = bytes.clone();
        copy[offset] = value;
        text_of(&copy)
    };
    let mut statement = from_hex(STATEMENT_HEX);
    statement[195..203].copy_from_slice(&0u64.to_be_bytes());
    let cases = [
        ("another prefix", format!("ncdb-invite-2:{body}")),
        ("no prefix", body.to_string()),
        ("padding", format!("ncdb-invite-1:{body}=")),
        (
            "a character outside base64url",
            format!("ncdb-invite-1:+{}", &body[1..]),
        ),
        ("an unknown version", changed(0, 2)),
        ("a sub-agent's kind", changed(97, 3)),
        ("kind 0", changed(97, 0)),
        ("an unknown statement byte", changed(162, 2)),
        ("a statement byte without the statement", changed(162, 1)),
        ("one byte short", text_of(&bytes[..162])),
        (
            "one byte too many",
            text_of(&[bytes.as_slice(), &[0]].concat()),
        ),
        ("number 0", text_of(&statement)),
        ("nothing", String::new()),
    ];

    for (case, text) in cases {
        let refused = text.parse::<Invitation>().unwrap_err();

        assert_eq!(refused.code(), ErrorCode::InvalidValue, "{case}");
    }
}
