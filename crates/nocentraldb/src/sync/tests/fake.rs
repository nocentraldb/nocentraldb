//! A relay that misbehaves on purpose: it authenticates every member, then
//! answers as a relay in trouble would, so the tests can count how often
//! the database connects and asks.

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use ncdb_core::cbor::{self, Key, Value};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message;

use super::relay_support::space::Space;
use super::relay_support::{field, get, kind, message, uint};

/// The space whose entries [`Trouble::Stranger`] sends: one of another
/// simulated member's, which the database's member does not belong to.
pub fn stranger() -> Space {
    Space::with_nonce(0, [0x5a; 16])
}

/// How the fake relay answers once a member authenticated.
#[derive(Clone, Copy, Debug)]
pub enum Trouble {
    /// Every frame of a push: `unavailable`.
    Unavailable,
    /// Every push: the error `rate_limited`, retry after this many
    /// seconds.
    RateLimited(u64),
    /// Every pull: one entry past a gap, which never moves the cursor.
    NoProgress,
    /// No request is ever answered.
    Silent,
    /// Every request is answered, except `ping`.
    NoPong,
    /// Right after `welcome`, a message of 200 KiB.
    Huge,
    /// Right after `welcome`, live entries of a space the member never
    /// subscribed to: the genesis of [`stranger`].
    Stranger,
    /// The TCP connection is accepted, but the WebSocket handshake never
    /// answered.
    Hang,
    /// `hello` is answered with the error of this code, and the connection
    /// ends.
    Refuse(&'static str),
}

/// A fake relay on a free local port.
pub struct FakeRelay {
    pub addr: SocketAddr,
    connections: Arc<AtomicUsize>,
    requests: Arc<AtomicUsize>,
}

impl FakeRelay {
    pub async fn start(trouble: Trouble) -> FakeRelay {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("a port");
        let addr = listener.local_addr().expect("an address");
        let connections = Arc::new(AtomicUsize::new(0));
        let requests = Arc::new(AtomicUsize::new(0));
        let (counted, asked) = (connections.clone(), requests.clone());
        tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                counted.fetch_add(1, Ordering::SeqCst);
                tokio::spawn(serve(stream, trouble, asked.clone()));
            }
        });
        FakeRelay {
            addr,
            connections,
            requests,
        }
    }

    pub fn url(&self) -> String {
        format!("ws://{}", self.addr)
    }

    /// How many TCP connections the database opened.
    pub fn connections(&self) -> usize {
        self.connections.load(Ordering::SeqCst)
    }

    /// How many requests after `welcome` the database sent, of every
    /// connection.
    pub fn requests(&self) -> usize {
        self.requests.load(Ordering::SeqCst)
    }
}

type Ws = WebSocketStream<TcpStream>;

async fn send(ws: &mut Ws, value: &Value) -> Option<()> {
    let bytes = cbor::encode(value);
    ws.send(Message::Binary(bytes.into())).await.ok()
}

async fn recv(ws: &mut Ws) -> Option<Value> {
    loop {
        match ws.next().await? {
            Ok(Message::Binary(bytes)) => return cbor::decode(&bytes).ok(),
            Ok(Message::Ping(_) | Message::Pong(_)) => {}
            _ => return None,
        }
    }
}

fn map(entries: Vec<(u64, Value)>) -> Value {
    let map = entries
        .into_iter()
        .map(|(k, v)| (Key::Unsigned(k), v))
        .collect::<BTreeMap<_, _>>();
    Value::Map(map)
}

async fn serve(stream: TcpStream, trouble: Trouble, requests: Arc<AtomicUsize>) {
    if let Trouble::Hang = trouble {
        let _held = stream;
        tokio::time::sleep(Duration::from_secs(3600)).await;
        return;
    }
    let Ok(mut ws) = tokio_tungstenite::accept_async(stream).await else {
        return;
    };
    let _hello = recv(&mut ws).await;
    if let Trouble::Refuse(code) = trouble {
        let error = message("error", None, &[(3, Value::Text(code.to_owned()))]);
        send(&mut ws, &error).await;
        return;
    }
    let limits = map(vec![
        (1, Value::Unsigned(128 * 1024)),
        (2, Value::Unsigned(32)),
        (3, Value::Unsigned(127 * 1024)),
    ]);
    let challenge = message(
        "challenge",
        None,
        &[
            (3, Value::Unsigned(1)),
            (4, Value::Bytes(vec![0x5e; 16])),
            (5, Value::Bytes(vec![7; 32])),
            (6, limits),
        ],
    );
    send(&mut ws, &challenge).await;
    let _auth = recv(&mut ws).await;
    let welcome = message(
        "welcome",
        None,
        &[
            (3, Value::Bytes(vec![1; 32])),
            (4, Value::Bytes(vec![9; 16])),
            (5, Value::Array(Vec::new())),
        ],
    );
    send(&mut ws, &welcome).await;
    if let Trouble::Huge = trouble {
        let huge = message("pong", Some(0), &[(3, Value::Bytes(vec![0; 200 * 1024]))]);
        send(&mut ws, &huge).await;
    }
    if let Trouble::Stranger = trouble {
        let space = stranger();
        let entry = map(vec![
            (1, Value::Unsigned(1)),
            (2, Value::Unsigned(0)),
            (3, Value::Bytes(space.genesis.bytes().to_vec())),
        ]);
        let entries = message(
            "entries",
            None,
            &[
                (3, Value::Bytes(space.space.0.to_vec())),
                (4, Value::Array(vec![entry])),
                (5, Value::Unsigned(1)),
            ],
        );
        send(&mut ws, &entries).await;
    }
    while let Some(request) = recv(&mut ws).await {
        requests.fetch_add(1, Ordering::SeqCst);
        let Some(answer) = answer(&request, trouble) else {
            continue;
        };
        if send(&mut ws, &answer).await.is_none() {
            return;
        }
    }
}

fn answer(request: &Value, trouble: Trouble) -> Option<Value> {
    let req = get(request, 2).map(uint);
    let error = |code: &str, extra: Vec<(u64, Value)>| {
        let mut fields = vec![(3, Value::Text(code.to_owned()))];
        fields.extend(extra);
        message("error", req, &fields)
    };
    match (trouble, kind(request)) {
        (Trouble::Silent, _) | (Trouble::NoPong, "ping") => None,
        (_, "ping") => Some(message("pong", req, &[])),
        (Trouble::NoProgress, "pull") => {
            let entry = map(vec![
                (1, Value::Unsigned(2)),
                (2, Value::Unsigned(0)),
                (3, Value::Bytes(vec![1, 2, 3])),
            ]);
            Some(message(
                "entries",
                req,
                &[
                    (3, field(request, 3).clone()),
                    (4, Value::Array(vec![entry])),
                    (5, Value::Unsigned(5)),
                ],
            ))
        }
        (_, "pull" | "subscribe") => Some(error("unknown_space", Vec::new())),
        (Trouble::Unavailable, "push") => {
            let frames = match field(request, 4) {
                Value::Array(frames) => frames.len(),
                _ => 0,
            };
            let rejected = map(vec![(3, Value::Text("unavailable".into()))]);
            Some(message(
                "pushed",
                req,
                &[(3, Value::Array(vec![rejected; frames]))],
            ))
        }
        (Trouble::RateLimited(secs), "push") => {
            Some(error("rate_limited", vec![(4, Value::Unsigned(secs))]))
        }
        (_, "push") => {
            let frames = match field(request, 4) {
                Value::Array(frames) => frames.len() as u64,
                _ => 0,
            };
            let accepted = (1..=frames)
                .map(|p| map(vec![(1, Value::Unsigned(p))]))
                .collect();
            Some(message("pushed", req, &[(3, Value::Array(accepted))]))
        }
        _ => None,
    }
}
