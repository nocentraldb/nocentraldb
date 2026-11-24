//! An authorizer in front of a relay, for the tests of a relay URL that
//! carries a short-lived token (NOC-496): the Rust SDK's and the bridge's.

use std::sync::{Arc, Mutex};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// An authorizer in front of a relay, as API Gateway's on `$connect`: it
/// lets a WebSocket handshake through when its URL carries
/// `token=<valid>`, and refuses any other with 403.
pub(crate) struct Gate {
    addr: std::net::SocketAddr,
    targets: Arc<Mutex<Vec<String>>>,
}

impl Gate {
    pub(crate) async fn start(relay: std::net::SocketAddr, valid: &'static str) -> Gate {
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

    pub(crate) fn url(&self) -> String {
        format!("ws://{}", self.addr)
    }

    /// The request target of every handshake the gate saw, in order.
    pub(crate) fn targets(&self) -> Vec<String> {
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
