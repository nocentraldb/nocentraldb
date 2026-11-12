//! The handler driven by recorded API Gateway WebSocket events, read
//! through `aws_lambda_events` as Lambda reads them, against the SQLite
//! backend and a connection API that records what it is asked to do.

use std::collections::BTreeSet;
use std::sync::Mutex;

use aws_lambda_events::apigw::ApiGatewayWebsocketProxyRequest;
use ncdb_core::cbor::{self, Key, Value};
use ncdb_relay::sqlite::SqliteStore;
use ncdb_relay::store::{StoreError, Token};
use ncdb_relay::{Config, Relay};
use ncdb_relay_lambda::connections::{Connections, MAX_CONNECTION_AGE, Stage};
use ncdb_relay_lambda::{
    Body, ConnectionApi, Event, Handler, MemoryConnections, Posted, Route, Urls,
};

const CONNECT: &str = include_str!("../src/harness/events/connect.json");
const MESSAGE: &str = include_str!("../src/harness/events/message.json");
const DISCONNECT: &str = include_str!("../src/harness/events/disconnect.json");

/// The connection id the recorded events carry.
const RECORDED: &str = "Q7mXJdJpFiACFbQ=";

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

impl Urls for NoUrls {
    fn upload(&self, _: &Token) -> String {
        String::from("http://uploads.invalid/")
    }
    fn download(&self, _: &Token) -> String {
        String::from("http://frames.invalid/")
    }
}

type Tested = Handler<SqliteStore, MemoryConnections, Recording, NoUrls>;

/// A handler on a fresh SQLite database, and the database's directory.
fn handler() -> (Tested, std::path::PathBuf) {
    let mut name = [0u8; 8];
    getrandom::fill(&mut name).unwrap();
    let dir = std::env::temp_dir().join(format!(
        "ncdb-relay-lambda-{}",
        name.iter().map(|b| format!("{b:02x}")).collect::<String>()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let store = SqliteStore::open(&dir.join("relay.sqlite")).unwrap();
    let relay = Relay::new(store, Config::new(BTreeSet::new()));
    let handler = Handler::new(
        relay,
        [0x5e; 16],
        MemoryConnections::default(),
        Recording::default(),
        NoUrls,
    );
    (handler, dir)
}

fn recorded(json: &str) -> Event {
    let request: ApiGatewayWebsocketProxyRequest = serde_json::from_str(json).unwrap();
    Event::from_request(&request).unwrap()
}

/// The posts, decoded.
fn posts(api: &Recording) -> Vec<(String, Value)> {
    let posts = api.posts.lock().unwrap();
    posts
        .iter()
        .map(|(id, bytes)| (id.clone(), cbor::decode(bytes).unwrap()))
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
    let connect = recorded(CONNECT);
    assert_eq!(connect.route, Route::Connect);
    assert_eq!(connect.connection, RECORDED);
    assert_eq!(connect.body, Body::None);
    assert_eq!(connect.connected_at, 1_793_369_121_000);
    assert_eq!(connect.at, 1_793_369_121_012);

    let message = recorded(MESSAGE);
    assert_eq!(message.route, Route::Message);
    // The recorded message is a binary `hello`, base64 as API Gateway
    // hands it to Lambda.
    let Body::Binary(bytes) = &message.body else {
        panic!("a binary message");
    };
    assert_eq!(kind(&cbor::decode(bytes).unwrap()), "hello");

    let disconnect = recorded(DISCONNECT);
    assert_eq!(disconnect.route, Route::Disconnect);
    assert_eq!(disconnect.body, Body::None);
}

#[test]
fn a_text_message_keeps_only_its_size() {
    let json = MESSAGE
        .replace("\"isBase64Encoded\": true", "\"isBase64Encoded\": false")
        .replace("owFlaGVsbG8DgQEEa25jZGItcnMvMC4x", "some text");
    assert_eq!(recorded(&json).body, Body::Text(9));
}

#[test]
fn an_event_without_a_connection_or_a_type_is_not_one() {
    let without: ApiGatewayWebsocketProxyRequest =
        serde_json::from_str(&MESSAGE.replace(RECORDED, "")).unwrap();
    assert!(Event::from_request(&without).is_err());
    let unknown: ApiGatewayWebsocketProxyRequest =
        serde_json::from_str(&MESSAGE.replace("\"MESSAGE\"", "\"OTHER\"")).unwrap();
    assert!(Event::from_request(&unknown).is_err());
    let bad: ApiGatewayWebsocketProxyRequest =
        serde_json::from_str(&MESSAGE.replace("owFlaGVsbG8DgQEEa25jZGItcnMvMC4x", "%%%")).unwrap();
    assert!(Event::from_request(&bad).is_err());
}

#[tokio::test]
async fn connect_then_hello_answers_a_challenge_and_disconnect_forgets() {
    let (handler, _dir) = handler();
    let line = handler.handle(recorded(CONNECT)).await;
    assert_eq!(line.route, Route::Connect);
    let stored = handler.connections().get(RECORDED).await.unwrap().unwrap();
    assert_eq!(stored.stage, Stage::Hello);
    assert_eq!(stored.expires_at, 1_793_369_121_000 + MAX_CONNECTION_AGE);

    let line = handler.handle(recorded(MESSAGE)).await;
    assert_eq!(line.kind, Some("hello"));
    let posts = posts(handler_api(&handler));
    assert_eq!(posts.len(), 1);
    assert_eq!(posts[0].0, RECORDED);
    assert_eq!(kind(&posts[0].1), "challenge");
    let stored = handler.connections().get(RECORDED).await.unwrap().unwrap();
    assert!(matches!(stored.stage, Stage::Auth { .. }));

    handler.handle(recorded(DISCONNECT)).await;
    assert!(handler.connections().is_empty());
}

#[tokio::test]
async fn a_message_on_a_connection_without_a_record_is_refused_and_closed() {
    let (handler, _dir) = handler();
    let line = handler.handle(recorded(MESSAGE)).await;
    let posts = posts(handler_api(&handler));
    assert_eq!(posts.len(), 1);
    assert_eq!(kind(&posts[0].1), "error");
    assert_eq!(code(&posts[0].1), "unauthenticated");
    assert_eq!(*handler_api(&handler).closed.lock().unwrap(), [RECORDED]);
    assert_eq!(line.codes.len(), 1);
}

#[tokio::test]
async fn a_text_message_before_auth_closes_the_connection() {
    let (handler, _dir) = handler();
    handler.handle(recorded(CONNECT)).await;
    let text = MESSAGE
        .replace("\"isBase64Encoded\": true", "\"isBase64Encoded\": false")
        .replace("owFlaGVsbG8DgQEEa25jZGItcnMvMC4x", "hello");
    handler.handle(recorded(&text)).await;
    let posts = posts(handler_api(&handler));
    assert_eq!(code(&posts[0].1), "unauthenticated");
    assert_eq!(*handler_api(&handler).closed.lock().unwrap(), [RECORDED]);
    assert!(handler.connections().is_empty());
}

#[tokio::test]
async fn a_post_to_a_gone_connection_forgets_it() {
    let (handler, _dir) = handler();
    handler.handle(recorded(CONNECT)).await;
    handler_api(&handler)
        .gone
        .lock()
        .unwrap()
        .push(RECORDED.to_owned());
    let line = handler.handle(recorded(MESSAGE)).await;
    assert_eq!(line.posts, 0);
    assert!(handler.connections().is_empty());
}

#[tokio::test]
async fn the_log_line_names_sizes_and_never_the_message() {
    let (handler, _dir) = handler();
    handler.handle(recorded(CONNECT)).await;
    let line = handler.handle(recorded(MESSAGE)).await;
    assert_eq!(line.bytes_in, 24);
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
