//! A small, strict HTTP/1.1 client for the upload and download URLs of
//! large changes (RFC 0007, "Large changes"): one request per connection,
//! `Connection: close`. It refuses what it does not need: a response head
//! longer than [`MAX_HEAD`], a response without `Content-Length` or with
//! `Transfer-Encoding`, a redirect or any other status than success, and a
//! body other than the declared size. A download reads exactly the size the
//! relay announced and nothing after it.
//!
//! **What this does not do yet.** `https://` URLs fail as unsupported: TLS
//! comes with `wss://` (NOC-121, second part).

use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// The largest frame the relay accepts: 16 MiB (RFC 0007, "Large
/// changes").
pub(crate) const MAX_FRAME: u64 = 16 * 1024 * 1024;

/// The longest response head the client reads.
const MAX_HEAD: usize = 8 * 1024;

/// The longest one request may take, a 16 MiB body included. The tests
/// shorten it.
const TRANSFER: Duration = if cfg!(test) {
    Duration::from_secs(10)
} else {
    Duration::from_secs(5 * 60)
};

/// Why a request failed. A failure never carries the URL, which holds a
/// token.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Failed {
    /// Not an `http://` URL.
    Unsupported,
    /// The connection failed, or the request took longer than [`TRANSFER`].
    Network,
    /// The server answered with this status, not success.
    Status(u16),
    /// The response is not one this client reads.
    Malformed,
    /// The body is not of the size it must be, or larger than 16 MiB.
    Size,
}

/// `GET url`: a body of exactly `size` bytes.
pub(crate) async fn get(url: &str, size: u64) -> Result<Vec<u8>, Failed> {
    if size > MAX_FRAME {
        return Err(Failed::Size);
    }
    timed(async {
        let (status, length, mut stream, mut body) = exchange("GET", url, &[]).await?;
        if status != 200 {
            return Err(Failed::Status(status));
        }
        if length != Some(size) || body.len() as u64 > size {
            return Err(Failed::Size);
        }
        let rest = (size - body.len() as u64) as usize;
        let start = body.len();
        body.resize(start + rest, 0);
        stream
            .read_exact(&mut body[start..])
            .await
            .map_err(|_| Failed::Network)?;
        Ok(body)
    })
    .await
}

/// `PUT url` with `body`.
pub(crate) async fn put(url: &str, body: &[u8]) -> Result<(), Failed> {
    timed(async {
        let (status, ..) = exchange("PUT", url, body).await?;
        match status {
            200..=299 => Ok(()),
            status => Err(Failed::Status(status)),
        }
    })
    .await
}

async fn timed<T>(
    request: impl std::future::Future<Output = Result<T, Failed>>,
) -> Result<T, Failed> {
    tokio::time::timeout(TRANSFER, request)
        .await
        .unwrap_or(Err(Failed::Network))
}

/// The authority (`host:port`) and the path of an `http://` URL.
fn split(url: &str) -> Result<(String, &str), Failed> {
    let rest = url.strip_prefix("http://").ok_or(Failed::Unsupported)?;
    if rest.bytes().any(|b| b.is_ascii_control() || b == b' ') {
        return Err(Failed::Unsupported);
    }
    let slash = rest.find('/').ok_or(Failed::Unsupported)?;
    let (authority, path) = rest.split_at(slash);
    if authority.is_empty() || authority.contains('@') {
        return Err(Failed::Unsupported);
    }
    let authority = match authority.rsplit_once(':') {
        Some((_, port)) if !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()) => {
            authority.to_owned()
        }
        _ if authority.starts_with('[') && !authority.ends_with(']') => {
            return Err(Failed::Unsupported);
        }
        _ => format!("{authority}:80"),
    };
    Ok((authority, path))
}

/// Sends one request and reads its response head: the status, the
/// `Content-Length`, the stream, and the bytes of the body already read.
async fn exchange(
    method: &str,
    url: &str,
    body: &[u8],
) -> Result<(u16, Option<u64>, TcpStream, Vec<u8>), Failed> {
    let (authority, path) = split(url)?;
    let mut stream = TcpStream::connect(&authority)
        .await
        .map_err(|_| Failed::Network)?;
    let head = format!(
        "{method} {path} HTTP/1.1\r\nHost: {authority}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream
        .write_all(head.as_bytes())
        .await
        .map_err(|_| Failed::Network)?;
    stream.write_all(body).await.map_err(|_| Failed::Network)?;
    let mut buf = Vec::with_capacity(1024);
    let end = loop {
        if let Some(end) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break end;
        }
        if buf.len() >= MAX_HEAD {
            return Err(Failed::Malformed);
        }
        let mut chunk = [0u8; 1024];
        let want = chunk.len().min(MAX_HEAD + 4 - buf.len());
        match stream.read(&mut chunk[..want]).await {
            Ok(0) | Err(_) => return Err(Failed::Network),
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
    };
    let rest = buf.split_off(end + 4);
    let head = std::str::from_utf8(&buf[..end]).map_err(|_| Failed::Malformed)?;
    let (status, length) = parse(head)?;
    Ok((status, length, stream, rest))
}

/// The status and the `Content-Length` of a response head.
fn parse(head: &str) -> Result<(u16, Option<u64>), Failed> {
    let mut lines = head.split("\r\n");
    let status_line = lines.next().ok_or(Failed::Malformed)?;
    let code = status_line
        .strip_prefix("HTTP/1.1 ")
        .or_else(|| status_line.strip_prefix("HTTP/1.0 "))
        .ok_or(Failed::Malformed)?;
    let code = code.get(..3).ok_or(Failed::Malformed)?;
    if !code.bytes().all(|b| b.is_ascii_digit()) {
        return Err(Failed::Malformed);
    }
    let status = code.parse().map_err(|_| Failed::Malformed)?;
    let mut length = None;
    for line in lines {
        let (name, value) = line.split_once(':').ok_or(Failed::Malformed)?;
        let value = value.trim();
        if name.eq_ignore_ascii_case("transfer-encoding") {
            return Err(Failed::Malformed);
        }
        if name.eq_ignore_ascii_case("content-length") {
            let parsed: u64 = value.parse().map_err(|_| Failed::Malformed)?;
            if length.is_some_and(|known| known != parsed) {
                return Err(Failed::Malformed);
            }
            length = Some(parsed);
        }
    }
    Ok((status, length))
}

#[cfg(test)]
mod tests {
    use tokio::net::TcpListener;

    use super::*;

    /// A server that answers one request with `response`, and the URL of
    /// its path `/x`.
    async fn serving(response: Vec<u8>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 4096];
            let _ = stream.read(&mut buf).await;
            let _ = stream.write_all(&response).await;
            let _ = stream.shutdown().await;
        });
        format!("http://{addr}/x")
    }

    fn response(head: &str, body: &[u8]) -> Vec<u8> {
        let mut bytes = head.as_bytes().to_vec();
        bytes.extend_from_slice(body);
        bytes
    }

    #[tokio::test]
    async fn a_download_reads_exactly_its_declared_size() {
        let url = serving(response(
            "HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\n",
            b"abc",
        ))
        .await;
        assert_eq!(get(&url, 3).await, Ok(b"abc".to_vec()));
    }

    #[tokio::test]
    async fn a_download_of_another_size_fails() {
        let url = serving(response(
            "HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\n",
            b"abcd",
        ))
        .await;
        assert_eq!(get(&url, 3).await, Err(Failed::Size));
        let url = serving(response(
            "HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\n",
            b"abcdef",
        ))
        .await;
        assert_eq!(get(&url, 3).await, Err(Failed::Size));
    }

    #[tokio::test]
    async fn a_response_without_content_length_or_chunked_fails() {
        let url = serving(response("HTTP/1.1 200 OK\r\n\r\n", b"abc")).await;
        assert_eq!(get(&url, 3).await, Err(Failed::Size));
        let chunked = "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nContent-Length: 3\r\n\r\n";
        let url = serving(response(chunked, b"abc")).await;
        assert_eq!(get(&url, 3).await, Err(Failed::Malformed));
    }

    #[tokio::test]
    async fn a_redirect_is_not_followed() {
        let head =
            "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/y\r\nContent-Length: 0\r\n\r\n";
        let url = serving(response(head, b"")).await;
        assert_eq!(get(&url, 3).await, Err(Failed::Status(302)));
    }

    #[tokio::test]
    async fn a_head_too_long_fails() {
        let head = format!("HTTP/1.1 200 OK\r\nX: {}\r\n\r\n", "a".repeat(MAX_HEAD));
        let url = serving(response(&head, b"")).await;
        assert_eq!(get(&url, 0).await, Err(Failed::Malformed));
    }

    #[tokio::test]
    async fn a_download_above_the_largest_frame_is_not_asked_for() {
        assert_eq!(
            get("http://127.0.0.1:1/x", MAX_FRAME + 1).await,
            Err(Failed::Size)
        );
    }

    #[tokio::test]
    async fn an_upload_succeeds_on_success_only() {
        let url = serving(response(
            "HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n",
            b"",
        ))
        .await;
        assert_eq!(put(&url, b"abc").await, Ok(()));
        let url = serving(response(
            "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n",
            b"",
        ))
        .await;
        assert_eq!(put(&url, b"abc").await, Err(Failed::Status(404)));
    }

    #[test]
    fn only_plain_http_urls_with_a_path_are_read() {
        assert_eq!(split("https://relay.example/x"), Err(Failed::Unsupported));
        assert_eq!(split("http://relay.example"), Err(Failed::Unsupported));
        assert_eq!(
            split("http://user@relay.example/x"),
            Err(Failed::Unsupported)
        );
        assert_eq!(split("http://relay.example/x y"), Err(Failed::Unsupported));
        assert_eq!(
            split("http://relay.example/frames/ab"),
            Ok(("relay.example:80".to_owned(), "/frames/ab"))
        );
        assert_eq!(
            split("http://127.0.0.1:7700/x"),
            Ok(("127.0.0.1:7700".to_owned(), "/x"))
        );
    }
}
