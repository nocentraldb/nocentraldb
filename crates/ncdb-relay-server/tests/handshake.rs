//! A peer that never finishes the opening handshake (RFC 0007, "Opening a
//! connection"): the WebSocket upgrade, `hello` and `auth`. Each step has a
//! deadline; when it passes, the server closes the connection, so a peer
//! that stops halfway cannot hold a task forever. The deadline runs per
//! step from the moment the step starts: WebSocket pings do not extend it.
//! An authenticated member is no longer under it. Each test starts its
//! clock before whatever starts the server's step, so a close it sees too
//! early is one the server made too early.

mod support;

use std::time::{Duration, Instant};

use ncdb_core::cbor::Value;
use support::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// How long the test server gives each step of the handshake.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_millis(300);

/// How long a test waits for the server to close before it fails.
const WAIT: Duration = Duration::from_secs(20);

fn hello() -> Value {
    message(
        "hello",
        None,
        &[(3, Value::Array(vec![Value::Unsigned(1)]))],
    )
}

/// The server ends the TCP connection: a read returns end of file or an
/// error, and whatever it sends before that is discarded.
async fn tcp_closed(stream: &mut TcpStream) {
    let mut buffer = [0u8; 1024];
    loop {
        let read = tokio::time::timeout(WAIT, stream.read(&mut buffer))
            .await
            .expect("the server closes the connection in time");
        match read {
            Ok(0) | Err(_) => return,
            Ok(_) => continue,
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_peer_that_never_sends_the_upgrade_is_closed() {
    let (addr, _dir) = server_with_handshake_timeout(HANDSHAKE_TIMEOUT).await;
    let started = Instant::now();
    let mut stream = TcpStream::connect(addr).await.expect("a TCP connection");
    tcp_closed(&mut stream).await;
    assert!(started.elapsed() >= HANDSHAKE_TIMEOUT);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_peer_that_stops_halfway_through_the_upgrade_is_closed() {
    let (addr, _dir) = server_with_handshake_timeout(HANDSHAKE_TIMEOUT).await;
    let started = Instant::now();
    let mut stream = TcpStream::connect(addr).await.expect("a TCP connection");
    stream
        .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\n")
        .await
        .expect("half a request is sent");
    tcp_closed(&mut stream).await;
    assert!(started.elapsed() >= HANDSHAKE_TIMEOUT);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_peer_that_never_says_hello_is_closed() {
    let (addr, _dir) = server_with_handshake_timeout(HANDSHAKE_TIMEOUT).await;
    let started = Instant::now();
    let mut client = Client::connect(addr).await;
    client.closed().await;
    assert!(started.elapsed() >= HANDSHAKE_TIMEOUT);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_peer_that_never_answers_the_challenge_is_closed() {
    let (addr, _dir) = server_with_handshake_timeout(HANDSHAKE_TIMEOUT).await;
    let mut client = Client::connect(addr).await;
    let started = Instant::now();
    let challenge = client.ask(&hello()).await;
    assert_eq!(kind(&challenge), "challenge");
    client.closed().await;
    assert!(started.elapsed() >= HANDSHAKE_TIMEOUT);
}

#[tokio::test(flavor = "multi_thread")]
async fn pings_do_not_extend_a_step() {
    let (addr, _dir) = server_with_handshake_timeout(HANDSHAKE_TIMEOUT).await;
    let started = Instant::now();
    let mut client = Client::connect(addr).await;
    client.ping_until_closed(Duration::from_millis(50)).await;
    assert!(started.elapsed() >= HANDSHAKE_TIMEOUT);
}

#[tokio::test(flavor = "multi_thread")]
async fn each_step_has_its_own_deadline() {
    let (addr, _dir) = server_with_handshake_timeout(HANDSHAKE_TIMEOUT).await;
    let mut client = Client::connect(addr).await;
    // Most of the hello step passes; the auth step still gets its whole
    // deadline.
    tokio::time::sleep(HANDSHAKE_TIMEOUT * 2 / 3).await;
    let challenge = client.ask(&hello()).await;
    assert_eq!(kind(&challenge), "challenge");
    tokio::time::sleep(HANDSHAKE_TIMEOUT * 2 / 3).await;
    let relay_id = bytes(field(&challenge, 4)).to_vec();
    let nonce: [u8; 32] = bytes(field(&challenge, 5)).try_into().expect("32 bytes");
    let welcome = client.ask(&auth(0, &relay_id, &nonce)).await;
    assert_eq!(kind(&welcome), "welcome", "{welcome:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn an_authenticated_member_is_not_under_the_deadline() {
    let (addr, _dir) = server_with_handshake_timeout(HANDSHAKE_TIMEOUT).await;
    let (mut client, _) = Client::member(addr, 0).await;
    client
        .quiet(u64::try_from((HANDSHAKE_TIMEOUT * 3).as_millis()).unwrap())
        .await;
    let pong = client.ask(&message("ping", Some(7), &[])).await;
    assert_eq!(kind(&pong), "pong", "{pong:?}");
}
