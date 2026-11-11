//! The upload and download URLs of large changes (RFC 0007, "Large
//! changes"), served over plain HTTP/1.1 by the same listener as the
//! WebSocket protocol (NOC-208): the head of each request on a new
//! connection is read first, and a request that asks for a WebSocket
//! upgrade goes on to the WebSocket handshake, head and all.
//!
//! - `PUT /uploads/<token>` takes the frame of an upload once, while its
//!   URL is valid, with exactly the declared size as `Content-Length`. A
//!   `Content-Length` above the largest frame is refused before any of the
//!   body is read.
//! - `GET /frames/<token>` serves an entry's frame while its token is
//!   valid.
//!
//! A token is 32 random bytes in hex. Each connection serves one request
//! and closes. Every check goes through the relay core.

use std::sync::Arc;
use std::time::Duration;

use ncdb_relay::store::{RelayStore, Token};
use ncdb_relay::{Filled, Relay};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::Instant;

use crate::MAX_FRAME;
use crate::connection::{Shared, blocking};
use crate::prefixed::Prefixed;

/// The longest request head the server reads.
const MAX_HEAD: usize = 8 * 1024;

/// The most headers a request head may carry.
const MAX_HEADERS: usize = 32;

/// The path of an upload URL, before its token.
pub(crate) const UPLOADS: &str = "/uploads/";

/// The path of a download URL, before its token.
pub(crate) const FRAMES: &str = "/frames/";

/// A token in hex, as a URL carries it.
pub(crate) fn hex(token: &Token) -> String {
    token.0.iter().map(|b| format!("{b:02x}")).collect()
}

/// The token of a URL's last path segment: 64 lowercase hex digits.
fn token(hex: &str) -> Option<Token> {
    let hex = hex.as_bytes();
    if hex.len() != 64 {
        return None;
    }
    let digit = |c: u8| match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        _ => None,
    };
    let mut token = [0u8; 32];
    for (i, byte) in token.iter_mut().enumerate() {
        *byte = digit(hex[2 * i])? << 4 | digit(hex[2 * i + 1])?;
    }
    Some(Token(token))
}

/// What a request head asks for.
enum Head {
    /// A WebSocket upgrade.
    Upgrade,
    /// `PUT /uploads/<token>`, with its `Content-Length`, if it has one.
    Put(Option<Token>, Option<u64>),
    /// `GET /frames/<token>`.
    Get(Option<Token>),
    /// Anything else.
    Other,
}

/// Reads the head of the first request on `stream`. A WebSocket upgrade
/// comes back with every byte read so far in front of the stream; any
/// other request is served here, and the connection closes.
pub(crate) async fn route<S: RelayStore + 'static>(
    mut stream: TcpStream,
    shared: &Arc<Shared<S>>,
) -> Option<Prefixed> {
    let deadline = Instant::now() + shared.handshake_timeout;
    let mut buf = Vec::with_capacity(1024);
    let (head, len) = loop {
        if let Some(parsed) = parse(&buf) {
            break parsed?;
        }
        if buf.len() >= MAX_HEAD {
            return None;
        }
        let mut chunk = [0u8; 1024];
        let read = tokio::time::timeout_at(deadline, stream.read(&mut chunk)).await;
        match read {
            Ok(Ok(n)) if n > 0 => buf.extend_from_slice(&chunk[..n]),
            _ => return None,
        }
    };
    let (status, body) = match head {
        Head::Upgrade => return Some(Prefixed::new(buf, stream)),
        Head::Put(Some(token), length) => {
            let rest = buf.split_off(len);
            put(&mut stream, shared, token, length, rest).await
        }
        Head::Get(Some(token)) => get(shared.relay.clone(), token).await,
        Head::Put(None, _) | Head::Get(None) | Head::Other => (404, Vec::new()),
    };
    respond(stream, shared, status, body).await;
    None
}

/// The head in `buf`, and its length, once it is complete: none while it
/// is not, `Some(None)` when it is no HTTP request.
fn parse(buf: &[u8]) -> Option<Option<(Head, usize)>> {
    let mut headers = [httparse::EMPTY_HEADER; MAX_HEADERS];
    let mut request = httparse::Request::new(&mut headers);
    let len = match request.parse(buf) {
        Ok(httparse::Status::Complete(len)) => len,
        Ok(httparse::Status::Partial) => return None,
        Err(_) => return Some(None),
    };
    let header = |name: &str| {
        request
            .headers
            .iter()
            .find(|h| h.name.eq_ignore_ascii_case(name))
            .and_then(|h| std::str::from_utf8(h.value).ok())
    };
    let upgrade = header("upgrade").is_some_and(|v| v.eq_ignore_ascii_case("websocket"));
    let length = header("content-length").and_then(|v| v.trim().parse::<u64>().ok());
    let chunked = header("transfer-encoding").is_some();
    let path = request.path.unwrap_or("");
    let head = match request.method {
        _ if upgrade => Head::Upgrade,
        Some("PUT") if path.starts_with(UPLOADS) => {
            let length = if chunked { None } else { length };
            Head::Put(token(&path[UPLOADS.len()..]), length)
        }
        Some("GET") if path.starts_with(FRAMES) => Head::Get(token(&path[FRAMES.len()..])),
        _ => Head::Other,
    };
    Some(Some((head, len)))
}

/// `PUT /uploads/<token>`: the declared size, the body of exactly that
/// length, then the core fills the upload.
async fn put<S: RelayStore + 'static>(
    stream: &mut TcpStream,
    shared: &Arc<Shared<S>>,
    token: Token,
    length: Option<u64>,
    mut body: Vec<u8>,
) -> (u16, Vec<u8>) {
    let Some(length) = length else {
        return (411, Vec::new());
    };
    if length > MAX_FRAME as u64 {
        return (413, Vec::new());
    }
    let relay = shared.relay.clone();
    match blocking(async move { relay.upload_size(token).await }).await {
        Ok(Some(size)) if size == length => {}
        Ok(Some(_)) => return (400, Vec::new()),
        Ok(None) => return (404, Vec::new()),
        Err(_) => return (503, Vec::new()),
    }
    let length = length as usize;
    if body.len() > length {
        return (400, Vec::new());
    }
    body.reserve_exact(length - body.len());
    // Each read must make progress within the handshake timeout, so a
    // peer that stops sending holds its place only so long.
    let mut chunk = vec![0u8; 64 * 1024];
    while body.len() < length {
        let want = (length - body.len()).min(chunk.len());
        let read = tokio::time::timeout(shared.handshake_timeout, stream.read(&mut chunk[..want]));
        match read.await {
            Ok(Ok(n)) if n > 0 => body.extend_from_slice(&chunk[..n]),
            _ => return (400, Vec::new()),
        }
    }
    let relay = shared.relay.clone();
    match blocking(async move { relay.fill_upload(token, body).await }).await {
        Ok(Filled::Stored) => (200, Vec::new()),
        Ok(Filled::NotFound) => (404, Vec::new()),
        Ok(Filled::WrongSize) => (400, Vec::new()),
        Err(_) => (503, Vec::new()),
    }
}

/// `GET /frames/<token>`: the entry's frame.
async fn get<S: RelayStore + 'static>(relay: Arc<Relay<S>>, token: Token) -> (u16, Vec<u8>) {
    match blocking(async move { relay.download(token).await }).await {
        Ok(Some(frame)) => (200, frame),
        Ok(None) => (404, Vec::new()),
        Err(_) => (503, Vec::new()),
    }
}

/// Writes the response and closes the connection. Before it closes, it
/// reads and drops what the peer still sends, for a short while, so a
/// refused body does not reset the connection before the peer reads the
/// response.
async fn respond<S>(mut stream: TcpStream, shared: &Arc<Shared<S>>, status: u16, body: Vec<u8>) {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        411 => "Length Required",
        413 => "Content Too Large",
        _ => "Service Unavailable",
    };
    let content_type = if body.is_empty() {
        ""
    } else {
        "Content-Type: application/octet-stream\r\n"
    };
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\n{content_type}Connection: close\r\n\r\n",
        body.len()
    );
    let write = async {
        stream.write_all(head.as_bytes()).await?;
        stream.write_all(&body).await?;
        stream.shutdown().await
    };
    if !matches!(
        tokio::time::timeout(shared.write_timeout, write).await,
        Ok(Ok(()))
    ) {
        return;
    }
    let linger = Duration::from_secs(1).min(shared.handshake_timeout);
    let deadline = Instant::now() + linger;
    let mut sink = [0u8; 16 * 1024];
    while let Ok(Ok(n)) = tokio::time::timeout_at(deadline, stream.read(&mut sink)).await {
        if n == 0 {
            break;
        }
    }
}
