//! The test relay of `ncdb_relay_server::local` (NOC-507): a relay server
//! an app or engine starts in its own tests, on SQLite in memory and a free
//! loopback port, on a runtime of its own, that stops when dropped. These
//! tests talk to it as a member would, with the server's own test client.

mod support;

use std::future::Future;
use std::net::SocketAddr;
use std::time::{SystemTime, UNIX_EPOCH};

use ed25519_dalek::SigningKey;
use ncdb_core::admission::Admission;
use ncdb_core::cbor::Value;
use ncdb_relay_server::local::{Options, TestRelay};
use support::space::Space;
use support::*;

/// The address in a `ws://` URL.
fn addr(url: &str) -> SocketAddr {
    url.strip_prefix("ws://")
        .expect("a ws:// URL")
        .parse()
        .expect("an address")
}

/// What the relay answers when member 0 pushes the genesis of a new space.
async fn creates_a_space(addr: SocketAddr) -> Value {
    let (mut client, _) = Client::member(addr, 0).await;
    let space = Space::new(0);
    client
        .push(1, space.space, &[&space.genesis])
        .await
        .remove(0)
}

#[tokio::test(flavor = "multi_thread")]
async fn it_serves_ws_on_a_free_loopback_port() {
    let one = TestRelay::start(Options::new()).expect("the relay starts");
    let two = TestRelay::start(Options::new()).expect("a second relay starts");
    let (a, b) = (addr(&one.url()), addr(&two.url()));
    assert!(a.ip().is_loopback());
    assert_ne!(a.port(), 0);
    assert_ne!(a, b);
}

#[tokio::test(flavor = "multi_thread")]
async fn an_allowed_identity_may_create_a_space() {
    let relay =
        TestRelay::start(Options::new().allow_identity(&account_hex())).expect("the relay starts");
    assert_eq!(creates_a_space(addr(&relay.url())).await, accepted(1));
}

#[tokio::test(flavor = "multi_thread")]
async fn without_an_allowlist_a_genesis_is_refused_with_not_allowed() {
    let relay = TestRelay::start(Options::new()).expect("the relay starts");
    assert_eq!(
        creates_a_space(addr(&relay.url())).await,
        rejected("not_allowed")
    );
}

#[test]
fn an_identity_that_is_not_64_hex_digits_is_invalid_input() {
    for identity in ["", "00", &"g".repeat(64), &"0".repeat(66)] {
        let error = TestRelay::start(Options::new().allow_identity(identity))
            .err()
            .expect("the relay does not start");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn an_admission_by_an_admission_key_for_its_relay_id_is_taken() {
    let key = SigningKey::from_bytes(&[0x5a; 32]);
    let relay = TestRelay::start(Options::new().admission_key(key.verifying_key().to_bytes()))
        .expect("the relay starts");
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let admission = Admission::sign(&key, relay.relay_id().to_vec(), [0x49; 32], now + 3600, 3);

    let (mut member, _) = Client::member(addr(&relay.url()), 0).await;
    let admit = message("admit", Some(1), &[(3, Value::Bytes(admission.encode()))]);
    assert_eq!(member.ask(&admit).await, message("ok", Some(1), &[]));
}

#[tokio::test(flavor = "multi_thread")]
async fn dropped_it_stops_listening() {
    let relay = TestRelay::start(Options::new()).expect("the relay starts");
    let addr = addr(&relay.url());
    drop(relay);
    assert!(tokio::net::TcpStream::connect(addr).await.is_err());
}

#[test]
fn it_starts_and_stops_outside_any_runtime() {
    let relay =
        TestRelay::start(Options::new().allow_identity(&account_hex())).expect("the relay starts");
    let url = relay.url();
    drop(relay);
    assert!(std::net::TcpStream::connect(addr(&url)).is_err());
}

/// What `admissions` answers for `identity`, at once.
fn admission_for(relay: &TestRelay, identity: &str) -> std::io::Result<String> {
    let fetch = relay.admissions();
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    match std::pin::pin!(fetch(identity.to_owned())).poll(&mut cx) {
        std::task::Poll::Ready(answer) => answer,
        std::task::Poll::Pending => panic!("the admission waits"),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn admitting_any_account_it_signs_an_admission_that_lets_an_unlisted_account_create_a_space()
{
    let relay = TestRelay::start(Options::new().admit_any()).expect("the relay starts");
    let text = admission_for(&relay, &account_hex()).expect("an admission");
    let admission: Admission = text.parse().expect("an admission's text");
    assert_eq!(admission.relay_id, relay.relay_id().to_vec());

    let (mut member, _) = Client::member(addr(&relay.url()), 0).await;
    let admit = message("admit", Some(1), &[(3, Value::Bytes(admission.encode()))]);
    assert_eq!(member.ask(&admit).await, message("ok", Some(1), &[]));
    let space = Space::new(0);
    let pushed = member.push(2, space.space, &[&space.genesis]).await;
    assert_eq!(pushed, [accepted(1)]);
}

#[test]
fn without_admit_any_or_for_a_malformed_identity_there_is_no_admission() {
    let closed = TestRelay::start(Options::new()).expect("the relay starts");
    assert!(admission_for(&closed, &account_hex()).is_err());
    let open = TestRelay::start(Options::new().admit_any()).expect("the relay starts");
    assert!(admission_for(&open, "00").is_err());
}
