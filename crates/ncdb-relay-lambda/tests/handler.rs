//! The handler driven by recorded API Gateway WebSocket events, read
//! through `aws_lambda_events` as Lambda reads them, against the SQLite
//! backend and a connection API that records what it is asked to do.

use std::collections::BTreeSet;
use std::sync::Mutex;

use aws_lambda_events::apigw::ApiGatewayWebsocketProxyRequest;
use ncdb_core::cbor::{self, Key, Value};
use ncdb_core::ids::{MemberId, SpaceId};
use ncdb_relay::sqlite::SqliteStore;
use ncdb_relay::store::Epoch;
use ncdb_relay::store::{Entry, RelayStore, StoreError, Token, UploadId};
use ncdb_relay::{Config, ErrorCode, Relay};
use ncdb_relay_lambda::connections::{
    Connection, Connections, MAX_CONNECTION_AGE, Stage, Subscription,
};
use ncdb_relay_lambda::{
    Body, ConnectionApi, Event, Handler, MemoryConnections, Posted, Route, Urls,
};

const CONNECT: &str = include_str!("../src/harness/events/connect.json");
const MESSAGE: &str = include_str!("../src/harness/events/message.json");
const DISCONNECT: &str = include_str!("../src/harness/events/disconnect.json");

/// The connection id the recorded events carry, a fixed stand-in for the
/// one API Gateway gave (NOC-292).
const RECORDED: &str = "Q7mXJdJpFiACFbQ=";

/// The body of the recorded message: the base64 of the `hello` the Rust
/// SDK sent, which API Gateway handed to Lambda as it arrived.
const HELLO: &str = "owFlaGVsbG8DgQEEbW5jZGItcnMvMC4xLjA=";

/// A connection API that records posts and closes; a post to a connection
/// in `gone` is `Gone`.
#[derive(Default)]
struct Recording {
    posts: Mutex<Vec<(String, Vec<u8>)>>,
    closed: Mutex<Vec<String>>,
    gone: Mutex<Vec<String>>,
}

impl ConnectionApi for Recording {
    async fn post(&self, id: &str, message: Vec<u8>) -> Result<Posted, StoreError> {
        if self.gone.lock().unwrap().iter().any(|g| g == id) {
            return Ok(Posted::Gone);
        }
        self.posts.lock().unwrap().push((id.to_owned(), message));
        Ok(Posted::Sent)
    }

    async fn close(&self, id: &str) -> Result<(), StoreError> {
        self.closed.lock().unwrap().push(id.to_owned());
        Ok(())
    }
}

struct NoUrls;

impl<S: RelayStore> Urls<S> for NoUrls {
    async fn upload(&self, _: UploadId, _: Token, _: u64) -> Result<String, ErrorCode> {
        Ok(String::from("http://uploads.invalid/"))
    }
    async fn download(
        &self,
        _: &Relay<S>,
        _: ncdb_core::ids::SpaceId,
        _: &Entry,
    ) -> Result<String, ErrorCode> {
        Ok(String::from("http://frames.invalid/"))
    }
}

type Tested = Handler<SqliteStore, MemoryConnections, Recording, NoUrls>;

/// A handler on a fresh SQLite database, and the database's directory.
fn handler() -> (Tested, std::path::PathBuf) {
    handler_with(MemoryConnections::default())
}

/// A handler on a fresh SQLite database whose connections are `connections`.
fn handler_with(connections: MemoryConnections) -> (Tested, std::path::PathBuf) {
    let mut name = [0u8; 8];
    getrandom::fill(&mut name).unwrap();
    let dir = std::env::temp_dir().join(format!(
        "ncdb-relay-lambda-{}",
        name.iter().map(|b| format!("{b:02x}")).collect::<String>()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let store = SqliteStore::open(&dir.join("relay.sqlite")).unwrap();
    let relay = Relay::new(store, Config::new(BTreeSet::new()));
    let handler = Handler::new(relay, [0x5e; 16], connections, Recording::default(), NoUrls);
    (handler, dir)
}

fn recorded(json: &str) -> Event {
    let request: ApiGatewayWebsocketProxyRequest = serde_json::from_str(json).unwrap();
    Event::from_request(&request).unwrap()
}

/// A recorded event, as if API Gateway sent it now on a connection opened
/// now: the recorded times are weeks old, and a connection's record
/// expires 2 hours after it opened.
fn live(json: &str) -> Event {
    let now = ncdb_relay_lambda::contract::now();
    Event {
        at: now,
        connected_at: now,
        ..recorded(json)
    }
}

/// The posts, decoded: each is the base64 text of a message (NOC-338).
fn posts(api: &Recording) -> Vec<(String, Value)> {
    let posts = api.posts.lock().unwrap();
    posts
        .iter()
        .map(|(id, text)| {
            let text = std::str::from_utf8(text).expect("a post is text");
            let bytes = ncdb_relay::wire::from_text(text).expect("a post is base64");
            (id.clone(), cbor::decode(&bytes).unwrap())
        })
        .collect()
}

fn field(value: &Value, k: u64) -> Option<&Value> {
    match value {
        Value::Map(map) => map.get(&Key::Unsigned(k)),
        _ => None,
    }
}

fn kind(value: &Value) -> &str {
    match field(value, 1) {
        Some(Value::Text(kind)) => kind,
        _ => panic!("no type"),
    }
}

fn code(value: &Value) -> &str {
    match field(value, 3) {
        Some(Value::Text(code)) => code,
        _ => panic!("no code"),
    }
}

#[test]
fn recorded_events_read_as_their_routes() {
    // The events were captured from a deployed API on 2026-11-17 (NOC-292).
    let connect = recorded(CONNECT);
    assert_eq!(connect.route, Route::Connect);
    assert_eq!(connect.connection, RECORDED);
    assert_eq!(connect.body, Body::None);
    assert_eq!(connect.connected_at, 1_794_875_589_580);
    assert_eq!(connect.at, 1_794_875_589_580);

    let message = recorded(MESSAGE);
    assert_eq!(message.route, Route::Message);
    assert_eq!(message.connected_at, 1_794_875_589_580);
    assert_eq!(message.at, 1_794_875_589_838);
    let Body::Message(bytes) = &message.body else {
        panic!("a message");
    };
    let hello = cbor::decode(bytes).unwrap();
    assert_eq!(kind(&hello), "hello");
    assert_eq!(field(&hello, 4), Some(&Value::Text("ncdb-rs/0.1.0".into())));

    let disconnect = recorded(DISCONNECT);
    assert_eq!(disconnect.route, Route::Disconnect);
    assert_eq!(disconnect.body, Body::None);
    assert_eq!(disconnect.at, 1_794_875_619_311);
}

#[test]
fn api_gateway_hands_a_text_frame_to_lambda_as_it_arrived() {
    // The member sent its `hello` as base64 in a text frame (NOC-338). API
    // Gateway put that text in `body` unchanged and marked it as not
    // base64-encoded: it does not encode a text frame again.
    let request: ApiGatewayWebsocketProxyRequest = serde_json::from_str(MESSAGE).unwrap();
    assert!(!request.is_base64_encoded);
    assert_eq!(request.body.as_deref(), Some(HELLO));
    let Body::Message(bytes) = recorded(MESSAGE).body else {
        panic!("a message");
    };
    assert_eq!(Some(bytes), ncdb_relay::wire::from_text(HELLO));
}

#[test]
fn a_body_that_is_no_base64_or_marked_binary_keeps_only_its_size() {
    let text = MESSAGE.replace(HELLO, "some text");
    assert_eq!(recorded(&text).body, Body::Unreadable(9));
    let bad = MESSAGE.replace(HELLO, "%%%");
    assert_eq!(recorded(&bad).body, Body::Unreadable(3));
    // A binary frame never reaches Lambda: API Gateway closes the
    // connection with 1003. A body marked as base64-encoded binary is no
    // message of RFC 0007.
    let binary = MESSAGE.replace("\"isBase64Encoded\": false", "\"isBase64Encoded\": true");
    assert_eq!(recorded(&binary).body, Body::Unreadable(HELLO.len()));
}

#[test]
fn an_event_without_a_connection_or_a_type_is_not_one() {
    let without: ApiGatewayWebsocketProxyRequest =
        serde_json::from_str(&MESSAGE.replace(RECORDED, "")).unwrap();
    assert!(Event::from_request(&without).is_err());
    let unknown: ApiGatewayWebsocketProxyRequest =
        serde_json::from_str(&MESSAGE.replace("\"MESSAGE\"", "\"OTHER\"")).unwrap();
    assert!(Event::from_request(&unknown).is_err());
}

#[tokio::test]
async fn connect_then_hello_answers_a_challenge_and_disconnect_forgets() {
    let (handler, _dir) = handler();
    let line = handler.handle(live(CONNECT)).await;
    assert_eq!(line.route, Route::Connect);
    let stored = handler.connections().get(RECORDED).await.unwrap().unwrap();
    assert_eq!(stored.stage, Stage::Hello);
    assert_eq!(stored.expires_at, stored.connected_at + MAX_CONNECTION_AGE);

    let line = handler.handle(live(MESSAGE)).await;
    assert_eq!(line.kind, Some("hello"));
    let posts = posts(handler_api(&handler));
    assert_eq!(posts.len(), 1);
    assert_eq!(posts[0].0, RECORDED);
    assert_eq!(kind(&posts[0].1), "challenge");
    let stored = handler.connections().get(RECORDED).await.unwrap().unwrap();
    assert!(matches!(stored.stage, Stage::Auth { .. }));

    handler.handle(live(DISCONNECT)).await;
    assert!(handler.connections().is_empty());
}

#[tokio::test]
async fn a_message_on_a_connection_without_a_record_is_refused_and_closed() {
    let (handler, _dir) = handler();
    let line = handler.handle(live(MESSAGE)).await;
    let posts = posts(handler_api(&handler));
    assert_eq!(posts.len(), 1);
    assert_eq!(kind(&posts[0].1), "error");
    assert_eq!(code(&posts[0].1), "unauthenticated");
    assert_eq!(*handler_api(&handler).closed.lock().unwrap(), [RECORDED]);
    assert_eq!(line.codes.len(), 1);
}

#[tokio::test]
async fn a_message_that_is_no_base64_before_auth_closes_the_connection() {
    let (handler, _dir) = handler();
    handler.handle(live(CONNECT)).await;
    let text = MESSAGE.replace(HELLO, "hello");
    handler.handle(live(&text)).await;
    let posts = posts(handler_api(&handler));
    assert_eq!(code(&posts[0].1), "unauthenticated");
    assert_eq!(*handler_api(&handler).closed.lock().unwrap(), [RECORDED]);
    assert!(handler.connections().is_empty());
}

/// Stands in for the emulator until NOC-278 closes: see
/// `delivery_to_a_gone_subscriber_waits_for_noc_278` in `tests/emulator.rs`.
#[tokio::test]
async fn a_post_to_a_gone_connection_forgets_it() {
    let (handler, _dir) = handler();
    handler.handle(live(CONNECT)).await;
    handler_api(&handler)
        .gone
        .lock()
        .unwrap()
        .push(RECORDED.to_owned());
    let line = handler.handle(live(MESSAGE)).await;
    assert_eq!(line.posts, 0);
    assert!(handler.connections().is_empty());
}

#[tokio::test]
async fn the_log_line_names_sizes_and_never_the_message() {
    let (handler, _dir) = handler();
    handler.handle(live(CONNECT)).await;
    let line = handler.handle(live(MESSAGE)).await;
    assert_eq!(line.bytes_in, 26);
    assert_eq!(line.posts, 1);
    let text = line.to_string();
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    let keys: BTreeSet<&str> = json
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    let allowed = BTreeSet::from([
        "route",
        "connection",
        "type",
        "in",
        "out",
        "posts",
        "codes",
        "at",
        "ms",
    ]);
    assert_eq!(keys, allowed);
    assert_eq!(json["route"], "message");
    assert_eq!(json["type"], "hello");
    assert_eq!(json["connection"], RECORDED);
    // The client string the `hello` carries is content the log never holds.
    assert!(!text.contains("ncdb-rs"));
}

fn handler_api(handler: &Tested) -> &Recording {
    handler.api()
}

#[tokio::test]
async fn the_memory_store_keeps_the_connections_contract() {
    ncdb_relay_lambda::contract::run(&MemoryConnections::default(), "memory").await;
}

#[tokio::test]
async fn a_store_that_keeps_records_keeps_the_connections_contract() {
    ncdb_relay_lambda::contract::run(&MemoryConnections::keeping(30), "kept").await;
}

const DAY: u64 = 24 * 60 * 60 * 1000;

#[tokio::test]
async fn a_closed_connection_is_kept_for_the_days_set_and_reads_as_gone() {
    let (handler, _dir) = handler_with(MemoryConnections::keeping(30));
    handler.handle(live(CONNECT)).await;
    let open = handler.connections().get(RECORDED).await.unwrap().unwrap();
    handler.handle(live(DISCONNECT)).await;
    assert_eq!(handler.connections().get(RECORDED).await.unwrap(), None);
    let kept = handler.connections().kept(RECORDED).expect("a kept record");
    assert_eq!(kept.connected_at, open.connected_at);
    assert!(kept.ended_at >= open.connected_at);
    assert_eq!(kept.until, kept.ended_at + 30 * DAY);
}

#[tokio::test]
async fn a_gone_connection_is_kept_too() {
    let (handler, _dir) = handler_with(MemoryConnections::keeping(7));
    handler.handle(live(CONNECT)).await;
    handler_api(&handler)
        .gone
        .lock()
        .unwrap()
        .push(RECORDED.to_owned());
    handler.handle(live(MESSAGE)).await;
    assert_eq!(handler.connections().get(RECORDED).await.unwrap(), None);
    let kept = handler.connections().kept(RECORDED).expect("a kept record");
    assert_eq!(kept.until, kept.ended_at + 7 * DAY);
}

#[tokio::test]
async fn by_default_a_closed_connection_keeps_no_record() {
    let (handler, _dir) = handler();
    handler.handle(live(CONNECT)).await;
    handler.handle(live(DISCONNECT)).await;
    assert_eq!(handler.connections().kept(RECORDED), None);
}

#[tokio::test]
async fn a_kept_record_names_the_member_and_every_space_it_subscribed_to() {
    let store = MemoryConnections::keeping(30);
    let member = MemberId([0x11; 32]);
    let (stays, left) = (SpaceId([0x33; 16]), SpaceId([0x44; 16]));
    let now = ncdb_relay_lambda::contract::now();
    let connection = Connection {
        stage: Stage::Member(member),
        connected_at: now,
        expires_at: now + MAX_CONNECTION_AGE,
    };
    store.put("c", connection).await.unwrap();
    let sub = Subscription {
        member,
        req: 1,
        epoch: Epoch([0x55; 16]),
    };
    store.subscribe("c", stays, sub).await.unwrap();
    store.subscribe("c", left, sub).await.unwrap();
    store.unsubscribe("c", left).await.unwrap();
    assert!(store.subscribers(left).await.unwrap().is_empty());
    store.remove("c").await.unwrap();
    assert!(store.subscribers(stays).await.unwrap().is_empty());
    assert!(store.of_member(member).await.unwrap().is_empty());
    let kept = store.kept("c").expect("a kept record");
    assert_eq!(kept.member, Some(member));
    assert_eq!(kept.spaces, [stays, left]);
}
