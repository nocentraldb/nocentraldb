//! A small HTTP/1.1 client over a real socket, written by hand, for the
//! upload and download URLs of RFC 0007, "Large changes": one request per
//! connection, `Connection: close`, the whole response read to the end.

use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// A response: its status code, its headers and its body.
#[derive(Debug, PartialEq, Eq)]
pub struct Response {
    pub status: u16,
    /// Each header's name in lowercase, and its value.
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Response {
    /// The value of the header `name`, if the response has it.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

/// The host and the path of an `http://host:port/path` URL.
fn split(url: &str) -> (&str, &str) {
    let rest = url.strip_prefix("http://").expect("an http:// URL");
    let slash = rest.find('/').expect("a path");
    (&rest[..slash], &rest[slash..])
}

/// Sends `method` to `url` with `body` and the given `Content-Length`.
pub async fn request_with_length(method: &str, url: &str, length: usize, body: &[u8]) -> Response {
    request_with_headers(method, url, &[], length, body).await
}

/// Sends `method` to `url` with the extra `headers`, `body` and the given
/// `Content-Length`.
pub async fn request_with_headers(
    method: &str,
    url: &str,
    headers: &[(&str, &str)],
    length: usize,
    body: &[u8],
) -> Response {
    let (host, path) = split(url);
    let mut stream = TcpStream::connect(host).await.expect("the server listens");
    let extra: String = headers
        .iter()
        .map(|(name, value)| format!("{name}: {value}\r\n"))
        .collect();
    let head = format!(
        "{method} {path} HTTP/1.1\r\nHost: {host}\r\n{extra}Content-Length: {length}\r\nConnection: close\r\n\r\n"
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
    let mut lines = head.split("\r\n");
    let status = lines
        .next()
        .and_then(|line| line.split(' ').nth(1))
        .and_then(|code| code.parse().ok())
        .expect("a status line");
    let headers = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_ascii_lowercase(), value.trim().to_owned()))
        .collect();
    Response {
        status,
        headers,
        body: raw[end + 4..].to_vec(),
    }
}
