//! The account's identity through the bridge (RFC 0015, "In the SDK";
//! names as pm built them on NOC-469, waiting on the owner's NOC-493):
//! `open` answers `"identity"`, the recovery key in hex or `null` while
//! the database waits to be paired, and `account.completePairing` answers
//! the identity the pairing gave.

use ncdb_core::cbor::Value;

use super::{Bridge, LOCK, as_text, field, map, ok, text};

fn open(extra: &[(&str, Value)]) -> (Bridge, Value) {
    let mut bridge = Bridge::new();
    let mut args = vec![("lock", text(LOCK)), ("inMemory", Value::Bool(true))];
    args.extend(extra.iter().cloned());
    let answer = ok(&mut bridge, "open", map(&args));
    (bridge, answer)
}

#[test]
fn open_answers_the_identity_or_null_while_waiting_to_be_paired() {
    let (mut laptop, opened) = open(&[]);
    let identity = field(&opened, "identity").expect("an identity").clone();
    let hex = as_text(&identity);
    assert_eq!(hex.len(), 64);
    assert!(
        hex.bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    );

    let (mut phone, waiting) = open(&[("pairing", Value::Bool(true))]);
    assert_eq!(field(&waiting, "identity"), Some(&Value::Null));

    let invitation = field(&ok(&mut phone, "invitation", map(&[])), "text")
        .expect("a text")
        .clone();
    let paired = ok(
        &mut laptop,
        "account.pair",
        map(&[("kind", text("device")), ("invitation", invitation)]),
    );
    let answer = field(&paired, "answer").expect("an answer").clone();
    let completed = ok(
        &mut phone,
        "account.completePairing",
        map(&[("answer", answer)]),
    );
    assert_eq!(field(&completed, "identity"), Some(&identity));
}
