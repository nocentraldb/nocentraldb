//! The upload and download URLs of large changes, which presigned S3 URLs
//! serve on AWS (RFC 0007, "How large changes go to S3"): `PUT
//! /uploads/<token>` takes an upload's frame once, of its declared size;
//! `GET /frames/<token>` serves an entry's frame while its token is valid.
//! Any origin may read the answer, as the bucket's CORS rule allows. One
//! request per connection.

use std::fmt::Write as _;
use std::sync::Arc;
use std::time::Duration;

use ncdb_relay::Filled;
use ncdb_relay::store::{Token, TokenUrls};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use super::Local;

/// The path of an upload URL, before its token.
pub(super) const UPLOADS: &str = "/uploads/";

/// The path of a download URL, before its token.
pub(super) const FRAMES: &str = "/frames/";

/// The largest request head.
const MAX_HEAD: usize = 16 * 1024;

/// How long a request may take to arrive.
const TIMEOUT: Duration = Duration::from_secs(10);

/// A token in hex, as a URL carries it.
pub(super) fn hex(token: &Token) -> String {
    token.0.iter().fold(String::new(), |mut out, b| {
        let _ = write!(out, "{b:02x}");
        out
    })
}

/// The token of a URL's last path segment: 64 lowercase hex digits.
fn token(hex: &str) -> Option<Token> {
    if hex.len() != 64 {
        return None;
    }
    let mut token = [0u8; 32];
    for (i, byte) in token.iter_mut().enumerate() {
        let pair = hex.get(2 * i..2 * i + 2)?;
        if pair.bytes().any(|c| c.is_ascii_uppercase()) {
            return None;
        }
        *byte = u8::from_str_radix(pair, 16).ok()?;
    }
    Some(Token(token))
}

/// Serves requests until the listener fails.
pub(super) async fn serve<S: TokenUrls + 'static>(listener: TcpListener, handler: Arc<Local<S>>) {
    while let Ok((stream, _)) = listener.accept().await {
        let handler = handler.clone();
        tokio::spawn(async move {
            let _ = tokio::time::timeout(TIMEOUT, one(stream, handler)).await;
        });
    }
}

/// Runs a future of the relay core on a thread that may block.
async fn blocking<T: Send + 'static>(future: impl Future<Output = T> + Send + 'static) -> T {
    let runtime = tokio::runtime::Handle::current();
    tokio::task::spawn_blocking(move || runtime.block_on(future))
        .await
        .expect("a relay call does not panic")
}

async fn one<S: TokenUrls + 'static>(mut stream: TcpStream, handler: Arc<Local<S>>) {
    let mut buf = Vec::new();
    let mut chunk = vec![0u8; 64 * 1024];
    let (method, path, length, start) = loop {
        let Ok(n) = stream.read(&mut chunk).await else {
            return;
        };
        if n == 0 {
            return;
        }
        buf.extend_from_slice(&chunk[..n]);
        let mut headers = [httparse::EMPTY_HEADER; 32];
        let mut request = httparse::Request::new(&mut headers);
        match request.parse(&buf) {
            Ok(httparse::Status::Complete(start)) => {
                let length = request
                    .headers
                    .iter()
                    .find(|h| h.name.eq_ignore_ascii_case("content-length"))
                    .and_then(|h| {
                        std::str::from_utf8(h.value)
                            .ok()?
                            .trim()
                            .parse::<u64>()
                            .ok()
                    });
                let method = request.method.unwrap_or_default().to_owned();
                let path = request.path.unwrap_or_default().to_owned();
                break (method, path, length, start);
            }
            Ok(httparse::Status::Partial) if buf.len() < MAX_HEAD => continue,
            _ => return respond(stream, 400, Vec::new()).await,
        }
    };
    let (status, body) = match (method.as_str(), path.as_str()) {
        ("PUT", path) if path.starts_with(UPLOADS) => match token(&path[UPLOADS.len()..]) {
            Some(token) => {
                let body = buf.split_off(start);
                put(&mut stream, &handler, token, length, body, &mut chunk).await
            }
            None => (404, Vec::new()),
        },
        ("GET", path) if path.starts_with(FRAMES) => match token(&path[FRAMES.len()..]) {
            Some(token) => {
                let handler = handler.clone();
                match blocking(async move { handler.relay().download(token).await }).await {
                    Ok(Some(frame)) => (200, frame),
                    Ok(None) => (404, Vec::new()),
                    Err(_) => (503, Vec::new()),
                }
            }
            None => (404, Vec::new()),
        },
        _ => (404, Vec::new()),
    };
    respond(stream, status, body).await;
}

/// `PUT /uploads/<token>`: the declared size, the body of exactly that
/// length, then the core fills the upload.
async fn put<S: TokenUrls + 'static>(
    stream: &mut TcpStream,
    handler: &Arc<Local<S>>,
    token: Token,
    length: Option<u64>,
    mut body: Vec<u8>,
    chunk: &mut [u8],
) -> (u16, Vec<u8>) {
    let Some(length) = length else {
        return (411, Vec::new());
    };
    let size = {
        let handler = handler.clone();
        blocking(async move { handler.relay().upload_size(token).await }).await
    };
    match size {
        Ok(Some(size)) if size == length => {}
        Ok(Some(_)) => return (400, Vec::new()),
        Ok(None) => return (404, Vec::new()),
        Err(_) => return (503, Vec::new()),
    }
    let length = usize::try_from(length).unwrap_or(usize::MAX);
    if body.len() > length {
        return (400, Vec::new());
    }
    while body.len() < length {
        let want = (length - body.len()).min(chunk.len());
        match stream.read(&mut chunk[..want]).await {
            Ok(n) if n > 0 => body.extend_from_slice(&chunk[..n]),
            _ => return (400, Vec::new()),
        }
    }
    let handler = handler.clone();
    match blocking(async move { handler.relay().fill_upload(token, body).await }).await {
        Ok(Filled::Stored) => (200, Vec::new()),
        Ok(Filled::NotFound) => (404, Vec::new()),
        Ok(Filled::WrongSize) => (400, Vec::new()),
        Err(_) => (503, Vec::new()),
    }
}

async fn respond(mut stream: TcpStream, status: u16, body: Vec<u8>) {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        411 => "Length Required",
        _ => "Service Unavailable",
    };
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\n\
         Access-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes()).await;
    let _ = stream.write_all(&body).await;
    let _ = stream.shutdown().await;
}
