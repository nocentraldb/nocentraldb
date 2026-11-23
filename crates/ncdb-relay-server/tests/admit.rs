//! RFC 0015, "How the relay takes it", over a real socket: `admit` {2 req,
//! 3 admission} carries an admission's canonical bytes, and the relay
//! answers `ok`, `malformed` for bytes that are no admission, or
//! `not_allowed` for an admission it does not take (pm, 2026-11-23,
//! NOC-469).

mod support;

use std::collections::BTreeSet;
use std::time::{SystemTime, UNIX_EPOCH};

use ed25519_dalek::SigningKey;
use ncdb_core::admission::Admission;
use ncdb_core::cbor::Value;
use support::*;

fn admission_key() -> SigningKey {
    SigningKey::from_bytes(&[0x5a; 32])
}

fn admit(req: u64, bytes: Vec<u8>) -> Value {
    message("admit", Some(req), &[(3, Value::Bytes(bytes))])
}

/// An admission for `relay_id`, by `key`, that holds for an hour.
fn admission(key: &SigningKey, relay_id: &[u8]) -> Vec<u8> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    Admission::sign(key, relay_id.to_vec(), [0x49; 32], now + 3600, 3).encode()
}

#[tokio::test(flavor = "multi_thread")]
async fn admit_answers_ok_malformed_or_not_allowed() {
    let keys = BTreeSet::from([admission_key().verifying_key().to_bytes()]);
    let (addr, _dir) = server_with_admission_keys(keys).await;
    let (mut member, _) = Client::member(addr, 0).await;

    let taken = admission(&admission_key(), &RELAY_ID);
    assert_eq!(
        member.ask(&admit(1, taken)).await,
        message("ok", Some(1), &[])
    );

    assert_eq!(
        member.ask(&admit(2, vec![0xa0])).await,
        error(Some(2), "malformed")
    );
    let elsewhere = admission(&admission_key(), &[0x0f; 16]);
    assert_eq!(
        member.ask(&admit(3, elsewhere)).await,
        error(Some(3), "not_allowed")
    );
    let stranger = admission(&SigningKey::from_bytes(&[0x5b; 32]), &RELAY_ID);
    assert_eq!(
        member.ask(&admit(4, stranger)).await,
        error(Some(4), "not_allowed")
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn admit_before_auth_is_unauthenticated() {
    let (addr, _dir) = server().await;
    let mut client = Client::connect(addr).await;
    let answer = client
        .ask(&admit(1, admission(&admission_key(), &RELAY_ID)))
        .await;
    assert_eq!(kind(&answer), "error");
}
