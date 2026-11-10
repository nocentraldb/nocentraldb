//! A member that talks to a relay server over a real socket, and a server
//! on a fresh SQLite database. The client writes and reads RFC 0007's
//! messages as CBOR values of its own, so the tests check the wire layout,
//! not the server's codec against itself.
#![allow(dead_code)]

#[path = "../../../ncdb-core/tests/support/space.rs"]
pub mod space;

use std::collections::{BTreeMap, BTreeSet};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use ed25519_dalek::Signer;
use futures_util::{SinkExt, StreamExt};
use ncdb_core::cbor::{self, Key, Value};
use ncdb_core::change::Change;
use ncdb_core::ids::SpaceId;
use ncdb_relay::Relay;
use ncdb_relay::sqlite::SqliteStore;
use ncdb_relay_server::Server;
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use space::{Who, account, key};

/// How long a test waits for a message before it fails.
const WAIT: Duration = Duration::from_secs(20);

/// The relay id of the test server.
pub const RELAY_ID: [u8; 16] = [0x5e; 16];

/// A temporary directory, removed on drop.
pub struct TempDir(pub PathBuf);

impl TempDir {
    pub fn new() -> TempDir {
        let mut name = [0u8; 16];
        getrandom::fill(&mut name).expect("a secure generator");
        let name: String = name.iter().map(|b| format!("{b:02x}")).collect();
        let dir = std::env::temp_dir().join(format!("ncdb-relay-server-{name}"));
        std::fs::create_dir_all(&dir).expect("a temporary directory");
        TempDir(dir)
    }

    pub fn database(&self) -> PathBuf {
        self.0.join("relay.sqlite")
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The hex of the simulated account's key, which may create spaces.
pub fn account_hex() -> String {
    hex(&account().verifying_key().to_bytes())
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// A server on a fresh SQLite database on a free local port, on which the
/// simulated account may create spaces.
pub async fn server() -> (SocketAddr, TempDir) {
    server_with(|server| server).await
}

/// As [`server`], with a connection that gives up on a member after
/// `timeout` without progress in writing to it.
pub async fn server_with_write_timeout(timeout: Duration) -> (SocketAddr, TempDir) {
    server_with(|server| server.with_write_timeout(timeout)).await
}

/// As [`server`], with a deadline of `timeout` on each step of the
/// opening handshake.
pub async fn server_with_handshake_timeout(timeout: Duration) -> (SocketAddr, TempDir) {
    server_with(|server| server.with_handshake_timeout(timeout)).await
}

/// As [`server`], holding at most `max` connections open at once.
pub async fn server_with_max_connections(max: usize) -> (SocketAddr, TempDir) {
    server_with(|server| server.with_max_connections(max)).await
}

/// As [`server`], holding at most `max` connections open at once that
/// have not finished the opening handshake.
pub async fn server_with_max_handshakes(max: usize) -> (SocketAddr, TempDir) {
    server_with(|server| server.with_max_handshakes(max)).await
}

async fn server_with(
    configure: impl FnOnce(Server<SqliteStore>) -> Server<SqliteStore>,
) -> (SocketAddr, TempDir) {
    let dir = TempDir::new();
    let store = SqliteStore::open(&dir.database()).expect("the database opens");
    let allowed = BTreeSet::from([account().verifying_key().to_bytes()]);
    let relay = Relay::new(store, ncdb_relay_server::config(allowed));
    let server = Server::bind("127.0.0.1:0", relay, RELAY_ID)
        .await
        .expect("the server binds");
    let server = configure(server);
    let addr = server.local_addr().expect("an address");
    tokio::spawn(server.run());
    (addr, dir)
}

/// A message: its type at key 1, its request id at key 2, then its fields.
pub fn message(kind: &str, req: Option<u64>, fields: &[(u64, Value)]) -> Value {
    let mut map = BTreeMap::new();
    map.insert(Key::Unsigned(1), Value::Text(kind.to_owned()));
    if let Some(req) = req {
        map.insert(Key::Unsigned(2), Value::Unsigned(req));
    }
    for (k, v) in fields {
        map.insert(Key::Unsigned(*k), v.clone());
    }
    Value::Map(map)
}

/// The field `k` of a map, if it has one.
pub fn get(value: &Value, k: u64) -> Option<&Value> {
    match value {
        Value::Map(map) => map.get(&Key::Unsigned(k)),
        _ => panic!("not a map"),
    }
}

/// The field `k` of a map.
pub fn field(value: &Value, k: u64) -> &Value {
    get(value, k).unwrap_or_else(|| panic!("no field {k}"))
}

pub fn text(value: &Value) -> &str {
    match value {
        Value::Text(text) => text,
        _ => panic!("not text"),
    }
}

pub fn uint(value: &Value) -> u64 {
    match value {
        Value::Unsigned(n) => *n,
        _ => panic!("not an unsigned integer"),
    }
}

pub fn bytes(value: &Value) -> &[u8] {
    match value {
        Value::Bytes(bytes) => bytes,
        _ => panic!("not bytes"),
    }
}

pub fn array(value: &Value) -> &[Value] {
    match value {
        Value::Array(items) => items,
        _ => panic!("not an array"),
    }
}

/// The type of a message.
pub fn kind(value: &Value) -> &str {
    text(field(value, 1))
}

/// The keys of a map, which must all be unsigned integers.
pub fn keys(value: &Value) -> Vec<u64> {
    match value {
        Value::Map(map) => map
            .keys()
            .map(|k| match k {
                Key::Unsigned(n) => *n,
                Key::Text(_) => panic!("a text key"),
            })
            .collect(),
        _ => panic!("not a map"),
    }
}

/// What `welcome` told the member.
pub struct Welcome {
    pub member: Vec<u8>,
    pub epoch: Vec<u8>,
    pub spaces: Vec<Vec<u8>>,
}

/// A member's connection to the relay.
pub struct Client {
    ws: WebSocketStream<MaybeTlsStream<TcpStream>>,
}

impl From<WebSocketStream<MaybeTlsStream<TcpStream>>> for Client {
    fn from(ws: WebSocketStream<MaybeTlsStream<TcpStream>>) -> Client {
        Client { ws }
    }
}

impl Client {
    /// A WebSocket connection that has not said hello yet.
    pub async fn connect(addr: SocketAddr) -> Client {
        let (ws, _) = tokio_tungstenite::connect_async(format!("ws://{addr}"))
            .await
            .expect("the server accepts the connection");
        Client { ws }
    }

    /// hello, challenge, auth and welcome, false when the server ends the
    /// connection on the way instead.
    pub async fn try_authenticate(&mut self, who: Who) -> bool {
        let versions = Value::Array(vec![Value::Unsigned(1)]);
        let hello = cbor::encode(&message("hello", None, &[(3, versions)]));
        let Some(challenge) = self.try_ask(hello).await else {
            return false;
        };
        assert_eq!(kind(&challenge), "challenge");
        let relay_id = bytes(field(&challenge, 4)).to_vec();
        let nonce: [u8; 32] = bytes(field(&challenge, 5)).try_into().expect("32 bytes");
        let auth = cbor::encode(&auth(who, &relay_id, &nonce));
        match self.try_ask(auth).await {
            Some(welcome) => kind(&welcome) == "welcome",
            None => false,
        }
    }

    /// Sends `bytes` and reads the next message, none when the connection
    /// ends first.
    async fn try_ask(&mut self, bytes: Vec<u8>) -> Option<Value> {
        self.ws.send(Message::Binary(bytes.into())).await.ok()?;
        loop {
            let next = tokio::time::timeout(WAIT, self.ws.next())
                .await
                .expect("a message or a close in time");
            match next?.ok()? {
                Message::Binary(bytes) => {
                    return Some(cbor::decode(&bytes).expect("canonical CBOR"));
                }
                Message::Ping(_) | Message::Pong(_) => continue,
                _ => return None,
            }
        }
    }

    /// A connection authenticated as `who`, and its welcome.
    pub async fn member(addr: SocketAddr, who: Who) -> (Client, Welcome) {
        let mut client = Client::connect(addr).await;
        let welcome = client.authenticate(who).await;
        (client, welcome)
    }

    /// hello, challenge, auth, welcome.
    pub async fn authenticate(&mut self, who: Who) -> Welcome {
        let versions = Value::Array(vec![Value::Unsigned(1)]);
        self.send(&message("hello", None, &[(3, versions)])).await;
        let challenge = self.recv().await;
        assert_eq!(kind(&challenge), "challenge");
        let relay_id = bytes(field(&challenge, 4)).to_vec();
        let nonce: [u8; 32] = bytes(field(&challenge, 5)).try_into().expect("32 bytes");
        self.send(&auth(who, &relay_id, &nonce)).await;
        let welcome = self.recv().await;
        assert_eq!(kind(&welcome), "welcome", "{welcome:?}");
        Welcome {
            member: bytes(field(&welcome, 3)).to_vec(),
            epoch: bytes(field(&welcome, 4)).to_vec(),
            spaces: array(field(&welcome, 5))
                .iter()
                .map(|s| bytes(s).to_vec())
                .collect(),
        }
    }

    pub async fn send(&mut self, value: &Value) {
        self.send_bytes(cbor::encode(value)).await;
    }

    pub async fn send_bytes(&mut self, bytes: Vec<u8>) {
        self.ws
            .send(Message::Binary(bytes.into()))
            .await
            .expect("the message is sent");
    }

    /// The next message, decoded; it must be canonical CBOR in one binary
    /// WebSocket message of at most 128 KiB.
    pub async fn recv(&mut self) -> Value {
        loop {
            let next = tokio::time::timeout(WAIT, self.ws.next())
                .await
                .expect("a message in time")
                .expect("the connection is open")
                .expect("a message");
            match next {
                Message::Binary(bytes) => {
                    assert!(bytes.len() <= ncdb_relay_server::MAX_MESSAGE);
                    return cbor::decode(&bytes).expect("canonical CBOR");
                }
                Message::Ping(_) | Message::Pong(_) => continue,
                other => panic!("not a binary message: {other:?}"),
            }
        }
    }

    /// Nothing arrives within `ms` milliseconds.
    pub async fn quiet(&mut self, ms: u64) {
        let next = tokio::time::timeout(Duration::from_millis(ms), self.ws.next()).await;
        assert!(next.is_err(), "unexpected: {next:?}");
    }

    /// The server closes the connection.
    pub async fn closed(&mut self) {
        loop {
            let next = tokio::time::timeout(WAIT, self.ws.next())
                .await
                .expect("the server closes in time");
            match next {
                None | Some(Err(_)) | Some(Ok(Message::Close(_))) => return,
                Some(Ok(Message::Ping(_) | Message::Pong(_))) => continue,
                Some(Ok(other)) => panic!("a message after the error: {other:?}"),
            }
        }
    }

    /// Sends a WebSocket ping every `every` until the server closes the
    /// connection, which it must do in time.
    pub async fn ping_until_closed(&mut self, every: Duration) {
        let started = std::time::Instant::now();
        loop {
            assert!(started.elapsed() < WAIT, "the server closes in time");
            if self
                .ws
                .send(Message::Ping(Vec::new().into()))
                .await
                .is_err()
            {
                return;
            }
            match tokio::time::timeout(every, self.ws.next()).await {
                Err(_) | Ok(Some(Ok(Message::Ping(_) | Message::Pong(_)))) => continue,
                Ok(None | Some(Err(_)) | Some(Ok(Message::Close(_)))) => return,
                Ok(Some(Ok(other))) => panic!("a message before the close: {other:?}"),
            }
        }
    }

    /// Every message until the server ends the connection, which it must
    /// do in time. A message cut off by the end is not among them.
    pub async fn until_closed(&mut self) -> Vec<Value> {
        let mut messages = Vec::new();
        loop {
            let next = tokio::time::timeout(WAIT, self.ws.next())
                .await
                .expect("the server ends the connection in time");
            match next {
                None | Some(Err(_)) | Some(Ok(Message::Close(_))) => return messages,
                Some(Ok(Message::Binary(bytes))) => {
                    messages.push(cbor::decode(&bytes).expect("canonical CBOR"));
                }
                Some(Ok(_)) => continue,
            }
        }
    }

    /// A request and its answer.
    pub async fn ask(&mut self, value: &Value) -> Value {
        self.send(value).await;
        self.recv().await
    }

    /// Pushes changes to `space`; the results. An accepted genesis makes
    /// this connection's member current in the space, so its `added`
    /// (RFC 0007, decision R13) must come right after `pushed`, and is
    /// read here.
    pub async fn push(&mut self, req: u64, space: SpaceId, changes: &[&Change]) -> Vec<Value> {
        let results = self.push_only(req, space, changes).await;
        let created = changes
            .iter()
            .zip(&results)
            .any(|(change, result)| is_genesis(change) && get(result, 1).is_some());
        if created {
            let added = self.recv().await;
            assert_eq!(kind(&added), "added", "{added:?}");
            assert_eq!(bytes(field(&added, 3)), space.0);
        }
        results
    }

    /// Pushes changes to `space`; the results, and nothing read after
    /// `pushed`.
    pub async fn push_only(&mut self, req: u64, space: SpaceId, changes: &[&Change]) -> Vec<Value> {
        let frames = changes.iter().map(|c| Value::Bytes(c.bytes().to_vec()));
        let push = message(
            "push",
            Some(req),
            &[
                (3, Value::Bytes(space.0.to_vec())),
                (4, Value::Array(frames.collect())),
            ],
        );
        let pushed = self.ask(&push).await;
        assert_eq!(kind(&pushed), "pushed", "{pushed:?}");
        assert_eq!(uint(field(&pushed, 2)), req);
        array(field(&pushed, 3)).to_vec()
    }
}

/// Whether `change` is a space's genesis.
pub fn is_genesis(change: &Change) -> bool {
    use ncdb_core::change::Kind;
    use ncdb_core::membership::{ControlBody, Entry};
    change.header().kind == Kind::CONTROL
        && ControlBody::decode(&change.header().payload)
            .is_ok_and(|body| matches!(body.entry, Entry::Genesis(_)))
}

/// The `auth` message of `who` for this relay id and nonce.
pub fn auth(who: Who, relay_id: &[u8], nonce: &[u8; 32]) -> Value {
    let signing = ed25519_dalek::SigningKey::from_bytes(&[who; 32]);
    let digest = ncdb_relay::auth::digest(relay_id, nonce);
    let signature = signing.sign(&digest);
    let public = key(who).public();
    message(
        "auth",
        None,
        &[
            (3, Value::Bytes(public.signing.to_vec())),
            (4, Value::Bytes(public.agreement.to_vec())),
            (5, Value::Bytes(signature.to_bytes().to_vec())),
        ],
    )
}

/// `{1: position}`: accepted at this position.
pub fn accepted(position: u64) -> Value {
    result(1, Value::Unsigned(position))
}

/// `{2: position}`: the relay already held it, at this position.
pub fn duplicate(position: u64) -> Value {
    result(2, Value::Unsigned(position))
}

/// `{3: code}`: refused with this code.
pub fn rejected(code: &str) -> Value {
    result(3, Value::Text(code.to_owned()))
}

fn result(k: u64, value: Value) -> Value {
    Value::Map(BTreeMap::from([(Key::Unsigned(k), value)]))
}

/// The error with this request id and code.
pub fn error(req: Option<u64>, code: &str) -> Value {
    message("error", req, &[(3, Value::Text(code.to_owned()))])
}
