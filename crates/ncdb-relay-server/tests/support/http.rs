//! A small HTTP/1.1 client over a real socket, written by hand, for the
//! upload and download URLs of RFC 0007, "Large changes": one request per
//! connection, `Connection: close`, the whole response read to the end.

use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// A response: its status code and its body.
#[derive(Debug, PartialEq, Eq)]
pub struct Response {
    pub status: u16,
    pub body: Vec<u8>,
}

/// The host and the path of an `http://host:port/path` URL.
fn split(url: &str) -> (&str, &str) {
    let rest = url.strip_prefix("http://").expect("an http:// URL");
    let slash = rest.find('/').expect("a path");
    (&rest[..slash], &rest[slash..])
}

/// Sends `method` to `url` with `body` and the given `Content-Length`.
pub async fn request_with_length(method: &str, url: &str, length: usize, body: &[u8]) -> Response {
    let (host, path) = split(url);
    let mut stream = TcpStream::connect(host).await.expect("the server listens");
    let head = format!(
        "{method} {path} HTTP/1.1\r\nHost: {host}\r\nContent-Length: {length}\r\nConnection: close\r\n\r\n"
    );
    stream
        .write_all(head.as_bytes())
        .await
        .expect("the head is sent");
    // A server may answer and close before it reads all of a body it
    // refuses; the response is read all the same.
    let _ = stream.write_all(body).await;
    let mut raw = Vec::new();
    let read = tokio::time::timeout(Duration::from_secs(20), stream.read_to_end(&mut raw));
    read.await
        .expect("an answer in time")
        .expect("the answer is read");
    parse(&raw)
}

/// Sends `method` to `url` with `body`, its length as `Content-Length`.
pub async fn request(method: &str, url: &str, body: &[u8]) -> Response {
    request_with_length(method, url, body.len(), body).await
}

/// `PUT url` with `body`.
pub async fn put(url: &str, body: &[u8]) -> Response {
    request("PUT", url, body).await
}

/// `GET url`.
pub async fn get(url: &str) -> Response {
    request("GET", url, &[]).await
}

fn parse(raw: &[u8]) -> Response {
    let end = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .expect("a response head");
    let head = std::str::from_utf8(&raw[..end]).expect("a text head");
    let status = head
        .split(' ')
        .nth(1)
        .and_then(|code| code.parse().ok())
        .expect("a status line");
    Response {
        status,
        body: raw[end + 4..].to_vec(),
    }
}
