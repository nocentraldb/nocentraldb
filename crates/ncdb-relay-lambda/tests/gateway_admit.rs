//! RFC 0015, "How the relay takes it", behind the local stand-in for API
//! Gateway: the Lambda handler answers `admit` as the WebSocket server does,
//! with `ok`, `malformed` or `not_allowed` (pm, 2026-11-23, NOC-469).

#[path = "../../ncdb-relay-server/tests/support/mod.rs"]
mod support;

use std::collections::BTreeSet;
use std::time::{SystemTime, UNIX_EPOCH};

use ed25519_dalek::SigningKey;
use ncdb_core::admission::Admission;
use ncdb_core::cbor::Value;
use ncdb_relay::sqlite::SqliteStore;
use ncdb_relay::{Config, Relay};
use ncdb_relay_lambda::harness::Gateway;
use support::space::account;
use support::*;

fn admission_key() -> SigningKey {
    SigningKey::from_bytes(&[0x5a; 32])
}

fn admit(req: u64, bytes: Vec<u8>) -> Value {
    message("admit", Some(req), &[(3, Value::Bytes(bytes))])
}

fn admission(key: &SigningKey, relay_id: &[u8]) -> Vec<u8> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    Admission::sign(key, relay_id.to_vec(), [0x49; 32], now + 3600, 3).encode()
}

#[tokio::test(flavor = "multi_thread")]
async fn the_handler_answers_admit() {
    let dir = TempDir::new();
    let store = SqliteStore::open(&dir.database()).expect("the database opens");
    let mut config = Config::new(BTreeSet::from([account().verifying_key().to_bytes()]));
    config.admission_keys = BTreeSet::from([admission_key().verifying_key().to_bytes()]);
    let gateway = Gateway::bind("127.0.0.1:0", Relay::new(store, config), RELAY_ID)
        .await
        .expect("the gateway binds");
    let addr = gateway.local_addr().expect("an address");
    tokio::spawn(gateway.run());
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
}
