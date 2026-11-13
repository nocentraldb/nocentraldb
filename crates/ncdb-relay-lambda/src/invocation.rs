//! What one invocation of the function is, and where it goes (NOC-290).
//!
//! The function has two callers:
//!
//! - **API Gateway**, with a WebSocket event per route. It goes to the
//!   [`Handler`].
//! - **The daily schedule** (`infra/aws`), whose event is exactly
//!   [`SWEEP_INPUT`]. It runs the orphan sweep over frames last written more
//!   than [`SWEEP_AGE`] ago (RFC 0007, "How large changes go to S3"), so it
//!   never deletes the frame of an append still in flight.
//!
//! A WebSocket event never reaches the sweep: API Gateway's events always
//! carry `requestContext`, and a member's message is only ever their
//! `body`. Anything that is neither is refused.

use std::future::Future;
use std::time::Instant;

use aws_lambda_events::apigw::ApiGatewayWebsocketProxyRequest;
use ncdb_relay::store::{RelayStore, StoreError};
use serde_json::Value;

use crate::connections::{ConnectionApi, Connections};
use crate::event::Event;
use crate::handler::{Handler, Urls};

/// The schedule's event, which the stack sets as its input.
pub const SWEEP_INPUT: &str = r#"{"ncdb-relay":"sweep-orphans"}"#;

/// How old an orphan frame must be before the sweep deletes it: a day, in
/// milliseconds.
pub const SWEEP_AGE: u64 = 24 * 60 * 60 * 1000;

/// The orphan sweep, as the AWS backend runs it
/// (`ncdb_relay_aws::AwsStore::sweep_orphans`).
pub trait Sweep: Send + Sync {
    /// Deletes the orphan frames last written before `cutoff`, in
    /// milliseconds since the Unix epoch, and returns how many.
    fn sweep(&self, cutoff: u64) -> impl Future<Output = Result<u32, StoreError>> + Send;
}

/// One invocation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Invocation {
    /// A WebSocket event of API Gateway.
    WebSocket(Event),
    /// The daily schedule's event.
    SweepOrphans,
}

/// Why an invocation's event is neither.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    /// Not an event of API Gateway's WebSocket API, nor the schedule's.
    Unknown,
    /// An event of the WebSocket API that does not read as one.
    NotAnEvent(crate::event::NotAnEvent),
}

impl Invocation {
    /// Reads the event Lambda hands the function.
    pub fn read(payload: Value) -> Result<Invocation, Refused> {
        let sweep: Value = serde_json::from_str(SWEEP_INPUT).map_err(|_| Refused::Unknown)?;
        if payload == sweep {
            return Ok(Invocation::SweepOrphans);
        }
        if !payload.get("requestContext").is_some_and(Value::is_object) {
            return Err(Refused::Unknown);
        }
        let request: ApiGatewayWebsocketProxyRequest =
            serde_json::from_value(payload).map_err(|_| Refused::Unknown)?;
        Event::from_request(&request)
            .map(Invocation::WebSocket)
            .map_err(Refused::NotAnEvent)
    }
}

/// Handles one invocation at `now`, in milliseconds since the Unix epoch,
/// writes its log lines through `log`, and returns whether it failed. Only
/// a failed sweep fails; the WebSocket API is always answered.
///
/// A sweep writes two lines, `{"sweep":"orphans","phase":"start",…}` before
/// it runs and `…"phase":"end"…` after, so a run Lambda's timeout cut off
/// shows as a start without an end, which the stack's alarm counts.
pub async fn invoke<S, C, A, U, W>(
    handler: &Handler<S, C, A, U>,
    sweep: &W,
    payload: Value,
    now: u64,
    log: &(dyn Fn(&str) + Sync),
) -> bool
where
    S: RelayStore,
    C: Connections,
    A: ConnectionApi,
    U: Urls<S>,
    W: Sweep,
{
    match Invocation::read(payload) {
        Ok(Invocation::WebSocket(event)) => {
            log(&handler.handle(event).await.to_string());
            false
        }
        Ok(Invocation::SweepOrphans) => {
            let started = Instant::now();
            let cutoff = now.saturating_sub(SWEEP_AGE);
            log(&format!(
                "{{\"sweep\":\"orphans\",\"phase\":\"start\",\"cutoff\":{cutoff},\"at\":{now}}}"
            ));
            let result = sweep.sweep(cutoff).await;
            let ms = started.elapsed().as_millis();
            // A store error's text can name the bucket and keys; the line
            // keeps only that it failed.
            let (detail, failed) = match result {
                Ok(deleted) => (format!("\"deleted\":{deleted}"), false),
                Err(_) => (String::from("\"failed\":true"), true),
            };
            log(&format!(
                "{{\"sweep\":\"orphans\",\"phase\":\"end\",\"cutoff\":{cutoff},{detail},\"at\":{now},\"ms\":{ms}}}"
            ));
            failed
        }
        Err(refused) => {
            log(&format!("{{\"refused\":\"{refused:?}\"}}"));
            false
        }
    }
}

#[cfg(feature = "aws")]
impl Sweep for ncdb_relay_aws::AwsStore {
    fn sweep(&self, cutoff: u64) -> impl Future<Output = Result<u32, StoreError>> + Send {
        self.sweep_orphans(cutoff)
    }
}
