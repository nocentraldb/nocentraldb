//! How many connections the server holds open at once. RFC 0007 lets a
//! relay close any connection at any time ("Liveness"), so a connection
//! above a limit is closed as soon as it is accepted, before the WebSocket
//! upgrade, and the member reconnects with back-off. One limit counts every
//! open connection; a smaller one counts those that have not finished the
//! opening handshake, so peers that stop in it cannot take every place.
//!
//! The server's handshake deadline stays at its default of 10 seconds, so
//! a close the tests see within [`AT_ONCE`] comes from a limit, not from
//! the deadline.

mod support;

use std::net::SocketAddr;
use std::time::Duration;

use support::*;
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;

/// How soon a connection above a limit is closed.
const AT_ONCE: Duration = Duration::from_secs(2);

/// How long a test retries a connection that a freed place should admit.
const WAIT: Duration = Duration::from_secs(20);

/// The server ends the TCP connection within [`AT_ONCE`]: a read returns
/// end of file or an error.
async fn closed_at_once(stream: &mut TcpStream) {
    let mut buffer = [0u8; 1024];
    loop {
        let read = tokio::time::timeout(AT_ONCE, stream.read(&mut buffer))
            .await
            .expect("the server closes the connection at once");
        match read {
            Ok(0) | Err(_) => return,
            Ok(_) => continue,
        }
    }
}

/// A TCP connection that the server accepted and still holds: it is open
/// once the server has had time to close it.
async fn held(addr: SocketAddr) -> TcpStream {
    let mut stream = TcpStream::connect(addr).await.expect("a TCP connection");
    let mut buffer = [0u8; 1];
    let read = tokio::time::timeout(Duration::from_millis(300), stream.read(&mut buffer)).await;
    assert!(read.is_err(), "the server holds the connection: {read:?}");
    stream
}

/// A member authenticates, retrying while the server closes it, as a
/// member that reconnects with back-off does.
async fn member_eventually(addr: SocketAddr, who: space::Who) -> Client {
    let started = std::time::Instant::now();
    loop {
        assert!(started.elapsed() < WAIT, "a place frees in time");
        if let Ok((ws, _)) = tokio_tungstenite::connect_async(format!("ws://{addr}")).await {
            let mut client = Client::from(ws);
            if client.try_authenticate(who).await {
                return client;
            }
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_connection_above_the_limit_is_closed_at_once() {
    let (addr, _dir) = server_with_max_connections(2).await;
    let (_first, _) = Client::member(addr, 0).await;
    let (_second, _) = Client::member(addr, 1).await;
    let mut third = TcpStream::connect(addr).await.expect("a TCP connection");
    closed_at_once(&mut third).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_connection_in_the_handshake_counts_towards_the_limit() {
    let (addr, _dir) = server_with_max_connections(2).await;
    let (_member, _) = Client::member(addr, 0).await;
    let _stalled = held(addr).await;
    let mut third = TcpStream::connect(addr).await.expect("a TCP connection");
    closed_at_once(&mut third).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_closed_connection_frees_its_place() {
    let (addr, _dir) = server_with_max_connections(1).await;
    let (first, _) = Client::member(addr, 0).await;
    drop(first);
    let mut second = member_eventually(addr, 1).await;
    let pong = second.ask(&message("ping", Some(7), &[])).await;
    assert_eq!(kind(&pong), "pong", "{pong:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_peer_above_the_handshake_limit_is_closed_at_once() {
    let (addr, _dir) = server_with_max_handshakes(2).await;
    let _first = held(addr).await;
    let _second = held(addr).await;
    let mut third = TcpStream::connect(addr).await.expect("a TCP connection");
    closed_at_once(&mut third).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn an_authenticated_member_leaves_the_handshake_limit() {
    let (addr, _dir) = server_with_max_handshakes(1).await;
    let (_first, _) = Client::member(addr, 0).await;
    let (mut second, _) = Client::member(addr, 1).await;
    let pong = second.ask(&message("ping", Some(7), &[])).await;
    assert_eq!(kind(&pong), "pong", "{pong:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_peer_that_leaves_the_handshake_frees_its_place() {
    let (addr, _dir) = server_with_max_handshakes(1).await;
    let stalled = held(addr).await;
    drop(stalled);
    let mut member = member_eventually(addr, 0).await;
    let pong = member.ask(&message("ping", Some(7), &[])).await;
    assert_eq!(kind(&pong), "pong", "{pong:?}");
}
