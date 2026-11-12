//! NOC-290: the function's two kinds of invocation. API Gateway's WebSocket
//! events go to the handler; the daily schedule's event, whose input is
//! exactly [`SWEEP_INPUT`], runs the orphan sweep with a cutoff a day back
//! (RFC 0007, "How large changes go to S3"). No WebSocket event ever
//! reaches the sweep, not even a message whose text is the schedule's
//! input, and nothing else does either.

use std::collections::BTreeSet;
use std::sync::Mutex;

use ncdb_relay::sqlite::SqliteStore;
use ncdb_relay::store::{Entry, RelayStore, StoreError, Token, UploadId};
use ncdb_relay::{Config, ErrorCode, Relay};
use ncdb_relay_lambda::invocation::{SWEEP_AGE, SWEEP_INPUT, Sweep, invoke};
use ncdb_relay_lambda::{ConnectionApi, Handler, MemoryConnections, Posted, Urls};
use serde_json::{Value, json};

const CONNECT: &str = include_str!("../src/harness/events/connect.json");
const MESSAGE: &str = include_str!("../src/harness/events/message.json");
const DISCONNECT: &str = include_str!("../src/harness/events/disconnect.json");

/// A sweep that records each cutoff it is asked for, and answers with
/// `deleted`, or fails when that is `None`.
struct Recorded {
    cutoffs: Mutex<Vec<u64>>,
    deleted: Option<u32>,
}

impl Recorded {
    fn answering(deleted: Option<u32>) -> Recorded {
        Recorded {
            cutoffs: Mutex::new(Vec::new()),
            deleted,
        }
    }

    fn cutoffs(&self) -> Vec<u64> {
        self.cutoffs.lock().unwrap().clone()
    }
}

impl Sweep for Recorded {
    async fn sweep(&self, cutoff: u64) -> Result<u32, StoreError> {
        self.cutoffs.lock().unwrap().push(cutoff);
        self.deleted
            .ok_or_else(|| StoreError("the bucket refused".into()))
    }
}

/// A connection API that posts nowhere.
struct Nowhere;

impl ConnectionApi for Nowhere {
    async fn post(&self, _: &str, _: Vec<u8>) -> Result<Posted, StoreError> {
        Ok(Posted::Sent)
    }
    async fn close(&self, _: &str) -> Result<(), StoreError> {
        Ok(())
    }
}

struct NoUrls;

impl Urls for NoUrls {
    async fn upload(&self, _: UploadId, _: Token, _: u64) -> Result<String, ErrorCode> {
        Ok(String::from("http://uploads.invalid/"))
    }
    async fn download<S: RelayStore>(
        &self,
        _: &Relay<S>,
        _: ncdb_core::ids::SpaceId,
        _: &Entry,
    ) -> Result<String, ErrorCode> {
        Ok(String::from("http://frames.invalid/"))
    }
}

type Tested = Handler<SqliteStore, MemoryConnections, Nowhere, NoUrls>;

fn handler() -> Tested {
    let mut name = [0u8; 8];
    getrandom::fill(&mut name).unwrap();
    let dir = std::env::temp_dir().join(format!(
        "ncdb-relay-lambda-{}",
        name.iter().map(|b| format!("{b:02x}")).collect::<String>()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let store = SqliteStore::open(&dir.join("relay.sqlite")).unwrap();
    Handler::new(
        Relay::new(store, Config::new(BTreeSet::new())),
        [0x5e; 16],
        MemoryConnections::default(),
        Nowhere,
        NoUrls,
    )
}

/// A recorded WebSocket event, as if API Gateway sent it at `now`.
fn websocket(json: &str, now: u64) -> Value {
    let mut event: Value = serde_json::from_str(json).unwrap();
    event["requestContext"]["requestTimeEpoch"] = json!(now);
    event["requestContext"]["connectedAt"] = json!(now);
    event
}

const NOW: u64 = 1_800_000_000_000;

#[tokio::test]
async fn the_schedule_s_input_runs_the_sweep_with_a_cutoff_a_day_back() {
    let handler = handler();
    let sweep = Recorded::answering(Some(3));
    let input: Value = serde_json::from_str(SWEEP_INPUT).unwrap();

    let outcome = invoke(&handler, &sweep, input, NOW).await;

    assert_eq!(sweep.cutoffs(), vec![NOW - SWEEP_AGE]);
    assert_eq!(SWEEP_AGE, 24 * 60 * 60 * 1000);
    assert!(!outcome.failed);
    let line: Value = serde_json::from_str(&outcome.line).unwrap();
    assert_eq!(line["sweep"], "orphans");
    assert_eq!(line["deleted"], 3);
    assert_eq!(line["cutoff"], NOW - SWEEP_AGE);
    assert!(handler.connections().is_empty());
}

#[tokio::test]
async fn websocket_events_go_to_the_handler_and_never_reach_the_sweep() {
    let handler = handler();
    let sweep = Recorded::answering(Some(0));
    let now = ncdb_relay_lambda::contract::now();

    for json in [CONNECT, MESSAGE, DISCONNECT] {
        let outcome = invoke(&handler, &sweep, websocket(json, now), now).await;
        assert!(!outcome.failed);
        let line: Value = serde_json::from_str(&outcome.line).unwrap();
        assert!(line.get("route").is_some(), "{}", outcome.line);
        assert!(line.get("sweep").is_none(), "{}", outcome.line);
    }
    assert_eq!(sweep.cutoffs(), Vec::<u64>::new());
}

#[tokio::test]
async fn a_message_whose_text_is_the_schedule_s_input_is_only_a_message() {
    let handler = handler();
    let sweep = Recorded::answering(Some(0));
    let now = ncdb_relay_lambda::contract::now();
    invoke(&handler, &sweep, websocket(CONNECT, now), now).await;

    let mut message = websocket(MESSAGE, now);
    message["body"] = json!(SWEEP_INPUT);
    message["isBase64Encoded"] = json!(false);
    let outcome = invoke(&handler, &sweep, message, now).await;

    let line: Value = serde_json::from_str(&outcome.line).unwrap();
    assert_eq!(line["route"], "message");
    assert_eq!(sweep.cutoffs(), Vec::<u64>::new());
}

#[tokio::test]
async fn any_other_event_is_refused_and_sweeps_nothing() {
    let handler = handler();
    let sweep = Recorded::answering(Some(0));
    let others = [
        json!({"ncdb-relay": "sweep-orphans", "cutoff": 0}),
        json!({"ncdb-relay": "sweep"}),
        json!({"ncdb-relay": ["sweep-orphans"]}),
        // EventBridge's own scheduled event, which the schedule never sends.
        json!({
            "version": "0",
            "detail-type": "Scheduled Event",
            "source": "aws.events",
            "detail": {}
        }),
        json!({}),
        json!(null),
        json!("sweep-orphans"),
        json!({"requestContext": {"eventType": "MESSAGE"}}),
    ];

    for other in others {
        let outcome = invoke(&handler, &sweep, other.clone(), NOW).await;
        let line: Value = serde_json::from_str(&outcome.line).unwrap();
        assert!(line.get("refused").is_some(), "{other}: {}", outcome.line);
    }
    assert_eq!(sweep.cutoffs(), Vec::<u64>::new());
}

#[tokio::test]
async fn a_failed_sweep_fails_the_invocation_and_logs_no_detail() {
    let handler = handler();
    let sweep = Recorded::answering(None);
    let input: Value = serde_json::from_str(SWEEP_INPUT).unwrap();

    let outcome = invoke(&handler, &sweep, input, NOW).await;

    assert!(outcome.failed);
    let line: Value = serde_json::from_str(&outcome.line).unwrap();
    assert_eq!(line["sweep"], "orphans");
    assert_eq!(line["failed"], true);
    assert!(!outcome.line.contains("bucket refused"), "{}", outcome.line);
}
