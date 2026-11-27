//! What the tests share: a small HTTP client for the backend, standing in
//! for the app's own calls to its backend, and waits on the SDK's streams.

#![allow(dead_code)]

use std::future::poll_fn;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::pin::Pin;
use std::time::Duration;

use futures_core::Stream;
use nocentraldb::{Connection, Db};

/// How long a test waits for the relay or the backend.
pub const WAIT: Duration = Duration::from_secs(20);

/// An answer of the backend: its status and its body.
#[derive(Debug)]
pub struct Answer {
    pub status: u16,
    pub body: String,
}

impl Answer {
    /// The body as JSON.
    pub fn json(&self) -> serde_json::Value {
        serde_json::from_str(&self.body).expect("a JSON body")
    }
}

/// Sends `method path` with `body` to the backend at `address`, with the
/// user's `session` when given.
pub fn request(
    address: &str,
    method: &str,
    path: &str,
    session: Option<&str>,
    body: &str,
) -> Answer {
    let mut stream = TcpStream::connect(address).expect("the backend listens");
    stream.set_read_timeout(Some(WAIT)).unwrap();
    let auth = session.map_or(String::new(), |s| format!("Authorization: Bearer {s}\r\n"));
    write!(
        stream,
        "{method} {path} HTTP/1.1\r\nHost: {address}\r\n{auth}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .unwrap();
    let mut answer = String::new();
    stream.read_to_string(&mut answer).unwrap();
    let (head, body) = answer.split_once("\r\n\r\n").expect("a head and a body");
    let status = head
        .split(' ')
        .nth(1)
        .and_then(|s| s.parse().ok())
        .expect("a status");
    Answer {
        status,
        body: body.to_owned(),
    }
}

/// Signs up `email` with the backend and gives the session, as the app's
/// own signup would.
pub fn sign_up(address: &str, email: &str) -> String {
    let answer = request(
        address,
        "POST",
        "/signup",
        None,
        &serde_json::json!({ "email": email }).to_string(),
    );
    assert_eq!(answer.status, 200, "{}", answer.body);
    answer.json()["session"].as_str().unwrap().to_owned()
}

/// The next item of a stream, within [`WAIT`].
pub async fn next<S: Stream + Unpin>(stream: &mut S) -> S::Item {
    let item = poll_fn(|cx| Pin::new(&mut *stream).poll_next(cx));
    tokio::time::timeout(WAIT, item)
        .await
        .expect("in time")
        .expect("the stream does not end")
}

/// Waits until `db` is connected and the relay holds every change it wrote.
pub async fn synced(db: &Db) {
    let mut sync = db.sync().subscribe();
    loop {
        let status = next(&mut sync).await;
        assert_eq!(status.error, None, "the relay reports an error");
        if status.connection == Connection::Connected && status.unconfirmed == 0 {
            return;
        }
    }
}
