//! NOC-290: the function's two kinds of invocation. API Gateway's WebSocket
//! events go to the handler; the daily schedule's event, whose input is
//! exactly [`SWEEP_INPUT`], runs the orphan sweep with a cutoff a day back
//! (RFC 0007, "How large changes go to S3"). No WebSocket event ever
//! reaches the sweep, not even a message whose text is the schedule's
//! input, and nothing else does either. A sweep logs that it started
//! before it runs, and that it ended after, so a run the timeout cut off
//! shows as a start without an end.

use std::collections::BTreeSet;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use ncdb_relay::sqlite::SqliteStore;
use ncdb_relay::store::{Entry, RelayStore, StoreError, Token, UploadId};
use ncdb_relay::{Config, ErrorCode, Relay};
use ncdb_relay_lambda::invocation::{
    Pass, SWEEP_AGE, SWEEP_INPUT, SWEEP_MARGIN, Sweep, Swept, invoke,
};
use ncdb_relay_lambda::{ConnectionApi, Handler, MemoryConnections, Posted, Urls};
use serde_json::{Value, json};

const CONNECT: &str = include_str!("../src/harness/events/connect.json");
const MESSAGE: &str = include_str!("../src/harness/events/message.json");
const DISCONNECT: &str = include_str!("../src/harness/events/disconnect.json");

/// The log lines an invocation wrote, in order.
#[derive(Default)]
struct Log(Mutex<Vec<String>>);

impl Log {
    fn write(&self, line: &str) {
        self.0.lock().unwrap().push(line.to_owned());
    }

    fn lines(&self) -> Vec<Value> {
        let lines = self.0.lock().unwrap();
        lines
            .iter()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    fn last(&self) -> Value {
        self.lines().pop().expect("no line")
    }
}

/// A sweep that records each cutoff, now and deadline it is given, and the
/// log lines written before it ran, and answers with `deleted` in a pass
/// that ends as `pass`, or fails when `deleted` is `None`.
struct Recorded<'a> {
    cutoffs: Mutex<Vec<u64>>,
    deadlines: Mutex<Vec<(u64, Instant)>>,
    logged_before: Mutex<Vec<Value>>,
    log: &'a Log,
    deleted: Option<u32>,
    pass: Pass,
}

impl<'a> Recorded<'a> {
    fn answering(log: &'a Log, deleted: Option<u32>) -> Recorded<'a> {
        Recorded::ending(log, deleted, Pass::Completed)
    }

    fn ending(log: &'a Log, deleted: Option<u32>, pass: Pass) -> Recorded<'a> {
        Recorded {
            cutoffs: Mutex::new(Vec::new()),
            deadlines: Mutex::new(Vec::new()),
            logged_before: Mutex::new(Vec::new()),
            log,
            deleted,
            pass,
        }
    }

    fn cutoffs(&self) -> Vec<u64> {
        self.cutoffs.lock().unwrap().clone()
    }
}

impl Sweep for Recorded<'_> {
    async fn sweep(&self, cutoff: u64, now: u64, deadline: Instant) -> Result<Swept, StoreError> {
        self.cutoffs.lock().unwrap().push(cutoff);
        self.deadlines.lock().unwrap().push((now, deadline));
        *self.logged_before.lock().unwrap() = self.log.lines();
        let deleted = self
            .deleted
            .ok_or_else(|| StoreError("the bucket refused".into()))?;
        Ok(Swept {
            deleted,
            pass: self.pass,
        })
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

/// An invocation's deadline, as Lambda gives it: 29 seconds from now.
fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(29)
}

#[tokio::test]
async fn the_schedule_s_input_runs_the_sweep_with_a_cutoff_a_day_back() {
    let handler = handler();
    let log = Log::default();
    let sweep = Recorded::answering(&log, Some(3));
    let input: Value = serde_json::from_str(SWEEP_INPUT).unwrap();

    let failed = invoke(&handler, &sweep, input, NOW, deadline(), &|l| log.write(l)).await;

    assert_eq!(sweep.cutoffs(), vec![NOW - SWEEP_AGE]);
    assert_eq!(SWEEP_AGE, 24 * 60 * 60 * 1000);
    assert!(!failed);
    let lines = log.lines();
    assert_eq!(lines.len(), 2);
    let (start, end) = (&lines[0], &lines[1]);
    assert_eq!(start["sweep"], "orphans");
    assert_eq!(start["phase"], "start");
    assert_eq!(start["cutoff"], NOW - SWEEP_AGE);
    assert_eq!(end["sweep"], "orphans");
    assert_eq!(end["phase"], "end");
    assert_eq!(end["deleted"], 3);
    assert_eq!(end["cutoff"], NOW - SWEEP_AGE);
    assert!(handler.connections().is_empty());
}

#[tokio::test]
async fn websocket_events_go_to_the_handler_and_never_reach_the_sweep() {
    let handler = handler();
    let log = Log::default();
    let sweep = Recorded::answering(&log, Some(0));
    let now = ncdb_relay_lambda::contract::now();

    for json in [CONNECT, MESSAGE, DISCONNECT] {
        let failed = invoke(
            &handler,
            &sweep,
            websocket(json, now),
            now,
            deadline(),
            &|l| log.write(l),
        )
        .await;
        assert!(!failed);
        let line = log.last();
        assert!(line.get("route").is_some(), "{line}");
        assert!(line.get("sweep").is_none(), "{line}");
    }
    assert_eq!(log.lines().len(), 3);
    assert_eq!(sweep.cutoffs(), Vec::<u64>::new());
}

#[tokio::test]
async fn a_message_whose_text_is_the_schedule_s_input_is_only_a_message() {
    let handler = handler();
    let log = Log::default();
    let sweep = Recorded::answering(&log, Some(0));
    let now = ncdb_relay_lambda::contract::now();
    invoke(
        &handler,
        &sweep,
        websocket(CONNECT, now),
        now,
        deadline(),
        &|l| log.write(l),
    )
    .await;

    let mut message = websocket(MESSAGE, now);
    message["body"] = json!(SWEEP_INPUT);
    message["isBase64Encoded"] = json!(false);
    invoke(&handler, &sweep, message, now, deadline(), &|l| {
        log.write(l)
    })
    .await;

    assert_eq!(log.last()["route"], "message");
    assert_eq!(sweep.cutoffs(), Vec::<u64>::new());
}

#[tokio::test]
async fn any_other_event_is_refused_and_sweeps_nothing() {
    let handler = handler();
    let log = Log::default();
    let sweep = Recorded::answering(&log, Some(0));
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
        invoke(&handler, &sweep, other.clone(), NOW, deadline(), &|l| {
            log.write(l)
        })
        .await;
        let line = log.last();
        assert!(line.get("refused").is_some(), "{other}: {line}");
    }
    assert_eq!(sweep.cutoffs(), Vec::<u64>::new());
}

#[tokio::test]
async fn a_failed_sweep_fails_the_invocation_and_logs_no_detail() {
    let handler = handler();
    let log = Log::default();
    let sweep = Recorded::answering(&log, None);
    let input: Value = serde_json::from_str(SWEEP_INPUT).unwrap();

    let failed = invoke(&handler, &sweep, input, NOW, deadline(), &|l| log.write(l)).await;

    assert!(failed);
    let end = log.last();
    assert_eq!(end["sweep"], "orphans");
    assert_eq!(end["phase"], "end");
    assert_eq!(end["failed"], true);
    assert!(!end.to_string().contains("bucket refused"), "{end}");
}

#[tokio::test]
async fn a_sweep_logs_its_start_before_it_runs() {
    // A run the timeout cuts off never logs its end; its start is already
    // in the log, which the stack's alarm counts.
    let handler = handler();
    let log = Log::default();
    let sweep = Recorded::answering(&log, Some(0));
    let input: Value = serde_json::from_str(SWEEP_INPUT).unwrap();

    invoke(&handler, &sweep, input, NOW, deadline(), &|l| log.write(l)).await;

    let before = sweep.logged_before.lock().unwrap().clone();
    assert_eq!(before.len(), 1);
    assert_eq!(before[0]["sweep"], "orphans");
    assert_eq!(before[0]["phase"], "start");
}

#[tokio::test]
async fn the_sweep_stops_its_margin_before_the_invocation_s_deadline() {
    // NOC-293: a run that stops before Lambda's timeout logs its end and
    // stores its cursor; one cut off by the timeout would do neither.
    let handler = handler();
    let log = Log::default();
    let sweep = Recorded::answering(&log, Some(0));
    let input: Value = serde_json::from_str(SWEEP_INPUT).unwrap();
    let at = deadline();

    invoke(&handler, &sweep, input, NOW, at, &|l| log.write(l)).await;

    assert_eq!(SWEEP_MARGIN, Duration::from_secs(5));
    let given = sweep.deadlines.lock().unwrap().clone();
    assert_eq!(given, vec![(NOW, at - SWEEP_MARGIN)]);
}

#[tokio::test]
async fn the_end_line_says_how_the_pass_stands() {
    for (pass, name) in [
        (Pass::Paused, "paused"),
        (Pass::Completed, "completed"),
        (Pass::Skipped, "skipped"),
    ] {
        let handler = handler();
        let log = Log::default();
        let sweep = Recorded::ending(&log, Some(2), pass);
        let input: Value = serde_json::from_str(SWEEP_INPUT).unwrap();

        let failed = invoke(&handler, &sweep, input, NOW, deadline(), &|l| log.write(l)).await;

        assert!(!failed, "a paused or skipped run is no failure");
        let end = log.last();
        assert_eq!(end["phase"], "end");
        assert_eq!(end["pass"], name);
        assert_eq!(end["deleted"], 2);
    }
}
