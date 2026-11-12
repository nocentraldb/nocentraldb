//! The handler behind the local stand-in for API Gateway, over real
//! sockets, with the relay server's own test client: a member speaks RFC
//! 0007 to it exactly as to the WebSocket server. API Gateway's limits
//! hold: a message above 128 KiB closes the connection, and so does the
//! connection's maximum age; either way `$disconnect` forgets it.

#[path = "../../ncdb-relay-server/tests/support/mod.rs"]
mod support;

use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ncdb_core::cbor::Value;
use ncdb_core::ids::SpaceId;
use ncdb_core::membership::Role;
use ncdb_relay::sqlite::SqliteStore;
use ncdb_relay::{Config, Relay};
use ncdb_relay_lambda::harness::Gateway;
use support::space::{Space, account};
use support::*;

/// A gateway on a fresh SQLite database, closing connections after
/// `max_age`; its address, its database and its log.
struct Local {
    addr: SocketAddr,
    _dir: TempDir,
    log: Arc<Mutex<Vec<String>>>,
    handler: Arc<
        ncdb_relay_lambda::Handler<
            SqliteStore,
            ncdb_relay_lambda::MemoryConnections,
            ncdb_relay_lambda::harness::FakeApi,
            ncdb_relay_lambda::harness::LocalUrls,
        >,
    >,
}

async fn gateway(max_age: Option<Duration>) -> Local {
    let dir = TempDir::new();
    let store = SqliteStore::open(&dir.database()).expect("the database opens");
    let allowed = BTreeSet::from([account().verifying_key().to_bytes()]);
    let mut config = Config::new(allowed);
    config.max_frame = ncdb_relay::MAX_FRAME;
    let mut gateway = Gateway::bind("127.0.0.1:0", Relay::new(store, config), RELAY_ID)
        .await
        .expect("the gateway binds");
    if let Some(max_age) = max_age {
        gateway = gateway.with_max_age(max_age);
    }
    let local = Local {
        addr: gateway.local_addr().expect("an address"),
        _dir: dir,
        log: gateway.log(),
        handler: gateway.handler(),
    };
    tokio::spawn(gateway.run());
    local
}

impl Local {
    /// Waits until the handler holds no open connection, and every
    /// `$disconnect` is logged: the gateway writes an invocation's line
    /// after the handler returns.
    async fn forgotten(&self) {
        let started = Instant::now();
        let count = |route: &str| {
            self.lines()
                .iter()
                .filter(|line| line["route"] == route)
                .count()
        };
        while !self.handler.connections().is_empty() || count("connect") != count("disconnect") {
            assert!(
                started.elapsed() < Duration::from_secs(20),
                "forgotten in time"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert_eq!(self.handler.connections().subscriptions(), 0);
    }

    fn lines(&self) -> Vec<serde_json::Value> {
        let log = self.log.lock().unwrap();
        log.iter()
            .map(|line| serde_json::from_str(line).expect("a JSON line"))
            .collect()
    }
}

fn subscribe(req: u64, space: SpaceId, epoch: &[u8], after: u64) -> Value {
    message(
        "subscribe",
        Some(req),
        &[
            (3, Value::Bytes(space.0.to_vec())),
            (4, Value::Bytes(epoch.to_vec())),
            (5, Value::Unsigned(after)),
        ],
    )
}

/// The request id, positions and head of an `entries` message.
fn page(message: &Value) -> (Option<u64>, Vec<u64>, u64) {
    assert_eq!(kind(message), "entries", "{message:?}");
    let positions = array(field(message, 4))
        .iter()
        .map(|e| uint(field(e, 1)))
        .collect();
    (
        get(message, 2).map(uint),
        positions,
        uint(field(message, 5)),
    )
}

#[tokio::test(flavor = "multi_thread")]
async fn a_push_reaches_the_subscribers_of_other_connections_live() {
    let local = gateway(None).await;
    let (mut zero, welcome) = Client::member(local.addr, 0).await;
    let mut space = Space::new(0);
    let genesis = space.genesis.clone();
    let add = space.add(0, 1, Role::Writer, &[&genesis]);
    let pushed = zero.push(1, space.space, &[&genesis, &add]).await;
    assert_eq!(pushed, [accepted(1), accepted(2)]);

    let (mut one, welcome_one) = Client::member(local.addr, 1).await;
    assert_eq!(welcome_one.spaces, [space.space.0.to_vec()]);
    let catch_up = one.ask(&subscribe(5, space.space, &welcome.epoch, 0)).await;
    assert_eq!(page(&catch_up), (Some(5), vec![1, 2], 2));

    let change = space.sealed(0, Vec::new());
    assert_eq!(zero.push(2, space.space, &[&change]).await, [accepted(3)]);
    let live = one.recv().await;
    assert_eq!(
        keys(&live),
        [1, 3, 4, 5],
        "a live message has no request id"
    );
    assert_eq!(page(&live), (None, vec![3], 3));
    zero.quiet(200).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_member_added_while_connected_hears_added() {
    let local = gateway(None).await;
    let (mut zero, _) = Client::member(local.addr, 0).await;
    let (mut one, welcome_one) = Client::member(local.addr, 1).await;
    assert!(welcome_one.spaces.is_empty());
    let mut space = Space::new(0);
    let genesis = space.genesis.clone();
    zero.push(1, space.space, &[&genesis]).await;
    let add = space.add(0, 1, Role::Reader, &[&genesis]);
    assert_eq!(zero.push_only(2, space.space, &[&add]).await, [accepted(2)]);
    let added = one.recv().await;
    assert_eq!(kind(&added), "added");
    assert_eq!(bytes(field(&added, 3)), space.space.0);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_message_above_128_kib_closes_the_connection() {
    let local = gateway(None).await;
    let (mut zero, _) = Client::member(local.addr, 0).await;
    zero.send_bytes(vec![0xa0; 128 * 1024 + 1]).await;
    zero.closed().await;
    local.forgotten().await;
    // The message never reached the handler: only the handshake and the
    // close did.
    let routes: Vec<String> = local
        .lines()
        .iter()
        .map(|line| line["route"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(routes, ["connect", "message", "message", "disconnect"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_message_of_128_kib_still_reaches_the_handler() {
    let local = gateway(None).await;
    let (mut zero, _) = Client::member(local.addr, 0).await;
    // Not a request, so the relay answers malformed and keeps the
    // connection: what counts is that the gateway let it through.
    zero.send_bytes(vec![0xa0; 128 * 1024]).await;
    let answer = zero.recv().await;
    assert_eq!(text(field(&answer, 3)), "malformed");
    // The handler posts its answer before the gateway writes the
    // invocation's log line, so the line may come a moment after.
    let started = Instant::now();
    while !local.lines().iter().any(|line| line["in"] == 128 * 1024) {
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "logged in time"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_connection_closes_at_its_maximum_age_and_is_forgotten() {
    let local = gateway(Some(Duration::from_millis(400))).await;
    let (mut zero, welcome) = Client::member(local.addr, 0).await;
    let space = Space::new(0);
    let genesis = space.genesis.clone();
    zero.push(1, space.space, &[&genesis]).await;
    zero.ask(&subscribe(2, space.space, &welcome.epoch, 0))
        .await;
    assert_eq!(local.handler.connections().subscriptions(), 1);
    zero.closed().await;
    local.forgotten().await;
    // A member reconnects, as RFC 0007's "Liveness" says it does.
    let (_again, welcome) = Client::member(local.addr, 0).await;
    assert_eq!(welcome.spaces, [space.space.0.to_vec()]);
}

#[tokio::test(flavor = "multi_thread")]
async fn every_log_line_holds_only_ids_sizes_codes_and_times() {
    let local = gateway(None).await;
    let (mut zero, _) = Client::member(local.addr, 0).await;
    let mut space = Space::new(0);
    let genesis = space.genesis.clone();
    let change = space.sealed(0, Vec::new());
    zero.push(1, space.space, &[&genesis, &change]).await;
    // The same change again is a duplicate; a change of a stranger is
    // refused with a code.
    let mut other = Space::new(1);
    let stranger = other.sealed(1, Vec::new());
    let results = zero.push_only(2, space.space, &[&change, &stranger]).await;
    assert_eq!(results[0], duplicate(2));
    drop(zero);
    local.forgotten().await;

    let allowed = BTreeSet::from([
        "route",
        "connection",
        "member",
        "type",
        "in",
        "out",
        "posts",
        "codes",
        "at",
        "ms",
    ]);
    let lines = local.lines();
    for line in &lines {
        let object = line.as_object().unwrap();
        for (key, value) in object {
            assert!(allowed.contains(key.as_str()), "{key} in {line}");
            match key.as_str() {
                "route" | "type" => assert!(value.as_str().unwrap().len() < 16),
                "connection" => assert_eq!(value.as_str().unwrap().len(), 16),
                "member" => assert_eq!(value.as_str().unwrap().len(), 64),
                "codes" => assert!(value.as_array().unwrap().iter().all(|c| c.is_string())),
                _ => assert!(value.is_u64(), "{key} is a number"),
            }
        }
    }
    let pushes: Vec<&serde_json::Value> = lines.iter().filter(|l| l["type"] == "push").collect();
    assert_eq!(pushes.len(), 2);
    assert!(pushes[1]["codes"].as_array().unwrap().len() == 1);
    // No frame's bytes reach the log, in hex or otherwise.
    let all: String = local.log.lock().unwrap().join("\n");
    for frame in [genesis.bytes(), change.bytes(), stranger.bytes()] {
        assert!(!all.contains(&hex(&frame[frame.len() - 16..])));
    }
}
