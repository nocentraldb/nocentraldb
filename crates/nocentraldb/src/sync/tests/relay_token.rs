//! A relay protected by a short-lived token in its URL (NOC-496; NOC-417,
//! "Protect the endpoint"): [`OpenOptions::relay_with`] takes the app's
//! function that gives the relay URL, called before each connect, so a
//! token that expired is replaced by a fresh one. A `$connect` the relay's
//! authorizer refuses shows in the sync status as `connect_refused`; a
//! function that fails, or gives no `ws://` or `wss://` URL, as
//! `relay_url_failed`; in both cases the SDK retries with backoff.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use super::support::*;
use crate::sync::Connection;
use crate::{Db, OpenOptions};

const ME: Who = 1;

/// An authorizer in front of a relay, as API Gateway's on `$connect`: it
/// lets a WebSocket handshake through when its URL carries
/// `token=<valid>`, and refuses any other with 403.
struct Gate {
    addr: std::net::SocketAddr,
    targets: Arc<Mutex<Vec<String>>>,
}

impl Gate {
    async fn start(relay: std::net::SocketAddr, valid: &'static str) -> Gate {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("a port");
        let addr = listener.local_addr().expect("an address");
        let targets = Arc::new(Mutex::new(Vec::new()));
        let seen = targets.clone();
        tokio::spawn(async move {
            while let Ok((inbound, _)) = listener.accept().await {
                tokio::spawn(admit(inbound, relay, valid, seen.clone()));
            }
        });
        Gate { addr, targets }
    }

    fn url(&self) -> String {
        format!("ws://{}", self.addr)
    }

    /// The request target of every handshake the gate saw, in order.
    fn targets(&self) -> Vec<String> {
        self.targets.lock().unwrap().clone()
    }
}

/// Reads the handshake's head, then refuses it or forwards it, and the
/// rest of the connection, to the relay.
async fn admit(
    mut inbound: TcpStream,
    relay: std::net::SocketAddr,
    valid: &str,
    targets: Arc<Mutex<Vec<String>>>,
) {
    let mut head = Vec::new();
    let mut buffer = [0u8; 4096];
    while !head.windows(4).any(|w| w == b"\r\n\r\n") {
        match inbound.read(&mut buffer).await {
            Ok(0) | Err(_) => return,
            Ok(n) => head.extend_from_slice(&buffer[..n]),
        }
    }
    let line = String::from_utf8_lossy(&head)
        .lines()
        .next()
        .unwrap_or("")
        .to_owned();
    let target = line.split(' ').nth(1).unwrap_or("").to_owned();
    targets.lock().unwrap().push(target.clone());
    if !target.contains(&format!("token={valid}")) {
        let _ = inbound
            .write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n")
            .await;
        let _ = inbound.shutdown().await;
        return;
    }
    let Ok(mut outbound) = TcpStream::connect(relay).await else {
        return;
    };
    if outbound.write_all(&head).await.is_err() {
        return;
    }
    let _ = tokio::io::copy_bidirectional(&mut inbound, &mut outbound).await;
}

async fn open(options: OpenOptions) -> Db {
    Db::open_as(options, [ME; 32], [ME ^ 0x80; 32], account())
        .await
        .expect("the test lock opens")
}

#[tokio::test(flavor = "multi_thread")]
async fn after_a_refused_connect_the_next_connect_asks_the_function_for_a_fresh_url() {
    let relay = LocalRelay::start().await;
    let gate = Gate::start(relay.addr, "fresh").await;
    let calls = Arc::new(AtomicUsize::new(0));
    let (base, counted) = (gate.url(), calls.clone());
    let options = OpenOptions::new(LOCK).in_memory().relay_with(move || {
        // The first token has expired by the time it is used.
        let token = match counted.fetch_add(1, Ordering::SeqCst) {
            0 => "expired",
            _ => "fresh",
        };
        let url = format!("{base}/prod?token={token}");
        async move { Ok::<_, std::io::Error>(url) }
    });
    let db = open(options).await;

    let status = synced(&db).await;

    assert_eq!(status.error, None);
    let targets = gate.targets();
    assert_eq!(
        targets[..2],
        ["/prod?token=expired", "/prod?token=fresh"],
        "{targets:?}"
    );
    assert!(calls.load(Ordering::SeqCst) >= 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_refused_connect_shows_as_connect_refused_and_is_retried() {
    let relay = LocalRelay::start().await;
    let gate = Gate::start(relay.addr, "fresh").await;
    let url = format!("{}/prod?token=expired", gate.url());
    let db = open(OpenOptions::new(LOCK).in_memory().relay(&url)).await;

    eventually("the relay refused the connect twice", || {
        gate.targets().len() >= 2
    })
    .await;
    let mut status = db.sync().current();
    eventually("the status names the refused connect", || {
        status = db.sync().current();
        status.error.is_some()
    })
    .await;

    assert_eq!(status.error.as_deref(), Some("connect_refused"));
    assert_ne!(status.connection, Connection::Connected);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_relay_function_that_fails_shows_as_relay_url_failed_and_is_called_again() {
    let calls = Arc::new(AtomicUsize::new(0));
    let counted = calls.clone();
    let options = OpenOptions::new(LOCK).in_memory().relay_with(move || {
        counted.fetch_add(1, Ordering::SeqCst);
        async { Err::<String, _>(std::io::Error::other("the app's backend is down")) }
    });
    let db = open(options).await;

    eventually("the function was called again", || {
        calls.load(Ordering::SeqCst) >= 2
    })
    .await;
    let status = db.sync().current();

    assert_eq!(status.error.as_deref(), Some("relay_url_failed"));
    assert_ne!(status.connection, Connection::Connected);
    assert!(!format!("{status:?}").contains("backend"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_relay_function_that_gives_no_relay_url_shows_as_relay_url_failed() {
    let options = OpenOptions::new(LOCK).in_memory().relay_with(|| async {
        Ok::<_, std::io::Error>("https://relay.example.com/?token=t".into())
    });
    let db = open(options).await;

    let mut status = db.sync().current();
    eventually("the status names the failed URL", || {
        status = db.sync().current();
        status.error.is_some()
    })
    .await;

    assert_eq!(status.error.as_deref(), Some("relay_url_failed"));
    assert!(!format!("{status:?}").contains("token"));
}
