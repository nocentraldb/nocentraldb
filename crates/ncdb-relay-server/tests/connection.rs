//! RFC 0007, "Transport and encoding" and "Opening a connection", over a
//! real socket: hello, challenge, auth, welcome, and the errors of a
//! connection that cannot open.

mod support;

use ncdb_core::cbor::Value;
use ncdb_relay::sqlite::SqliteStore;
use ncdb_relay::store::RelayStore;
use support::space::{Space, id};
use support::*;

#[tokio::test(flavor = "multi_thread")]
async fn hello_gets_a_challenge_with_the_relay_id_a_nonce_and_the_limits() {
    let (addr, _dir) = server().await;
    let mut client = Client::connect(addr).await;
    let hello = message(
        "hello",
        None,
        &[
            (3, Value::Array(vec![Value::Unsigned(1)])),
            (4, Value::Text("ncdb-test/0.1".into())),
        ],
    );
    let challenge = client.ask(&hello).await;
    assert_eq!(kind(&challenge), "challenge");
    assert_eq!(keys(&challenge), [1, 3, 4, 5, 6]);
    assert_eq!(uint(field(&challenge, 3)), 1);
    assert_eq!(bytes(field(&challenge, 4)), RELAY_ID);
    assert_eq!(bytes(field(&challenge, 5)).len(), 32);
    let limits = field(&challenge, 6);
    assert_eq!(keys(limits), [1, 2, 3]);
    assert_eq!(uint(field(limits, 1)), 128 * 1024);
    assert_eq!(uint(field(limits, 2)), 32);
    assert_eq!(uint(field(limits, 3)), ncdb_relay_server::MAX_FRAME as u64);
}

#[tokio::test(flavor = "multi_thread")]
async fn every_challenge_has_a_fresh_nonce() {
    let (addr, _dir) = server().await;
    let hello = message(
        "hello",
        None,
        &[(3, Value::Array(vec![Value::Unsigned(1)]))],
    );
    let mut nonces = Vec::new();
    for _ in 0..2 {
        let mut client = Client::connect(addr).await;
        let challenge = client.ask(&hello).await;
        nonces.push(bytes(field(&challenge, 5)).to_vec());
    }
    assert_ne!(nonces[0], nonces[1]);
}

#[tokio::test(flavor = "multi_thread")]
async fn auth_gets_a_welcome_with_the_member_id_the_epoch_and_its_spaces() {
    let (addr, dir) = server().await;
    let (mut client, welcome) = Client::member(addr, 0).await;
    assert_eq!(welcome.member, id(0).0);
    let epoch =
        ncdb_relay::conformance::block_on(SqliteStore::open(&dir.database()).unwrap().epoch())
            .unwrap();
    assert_eq!(welcome.epoch, epoch.0);
    assert!(welcome.spaces.is_empty());

    let space = Space::new(0);
    let results = client.push(1, space.space, &[&space.genesis]).await;
    assert_eq!(results, [accepted(1)]);
    let (_, again) = Client::member(addr, 0).await;
    assert_eq!(again.spaces, [space.space.0.to_vec()]);
    let (_, stranger) = Client::member(addr, 5).await;
    assert!(stranger.spaces.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_hello_without_a_common_version_is_refused_and_closed() {
    let (addr, _dir) = server().await;
    let mut client = Client::connect(addr).await;
    let hello = message(
        "hello",
        None,
        &[(3, Value::Array(vec![Value::Unsigned(2)]))],
    );
    assert_eq!(client.ask(&hello).await, error(None, "unsupported_version"));
    client.closed().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_signature_over_another_nonce_is_unauthenticated() {
    let (addr, _dir) = server().await;
    let mut client = Client::connect(addr).await;
    let hello = message(
        "hello",
        None,
        &[(3, Value::Array(vec![Value::Unsigned(1)]))],
    );
    let challenge = client.ask(&hello).await;
    let auth = auth(0, bytes(field(&challenge, 4)), &[0; 32]);
    assert_eq!(client.ask(&auth).await, error(None, "unauthenticated"));
    client.closed().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_request_before_auth_is_unauthenticated() {
    let (addr, _dir) = server().await;
    let mut client = Client::connect(addr).await;
    let ping = message("ping", Some(9), &[]);
    assert_eq!(client.ask(&ping).await, error(Some(9), "unauthenticated"));
    client.closed().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn ping_gets_pong() {
    let (addr, _dir) = server().await;
    let (mut client, _) = Client::member(addr, 0).await;
    let pong = client.ask(&message("ping", Some(7), &[])).await;
    assert_eq!(pong, message("pong", Some(7), &[]));
}
