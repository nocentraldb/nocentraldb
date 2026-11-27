//! The app's functions across the bridge: they live with the app, on the
//! other side of the worker, so the database's function asks the SDK and
//! waits. Each ask gets a request number and goes out as an event; the
//! SDK's answer gives the waiting sync a text, or nothing when the app's
//! function failed. An answer to a request nobody waits for is ignored,
//! such as the late answer to a relay ask the sync stopped waiting for
//! after 30 s (NOC-572).
//!
//! - RFC 0015's admission ("In the SDK"): `{"event": "admission",
//!   "request": n, "identity": hex}`, answered by `admission.answer`
//!   `{"request": n, "admission": text | null}`.
//! - The relay URL (NOC-496), asked before each connect: `{"event":
//!   "relay", "request": n}`, answered by `relay.answer` `{"request": n,
//!   "relay": text | null}`.

use std::collections::BTreeMap;
use std::future::Future;
use std::sync::{Arc, Mutex, MutexGuard};
use std::task::{Context, Waker};

use ncdb_core::cbor::Value;
use tokio::sync::oneshot;

use super::{Failed, entries, get, invalid};

/// The asks of one open database.
#[derive(Clone, Default)]
pub(super) struct Asks(Arc<Mutex<State>>);

#[derive(Default)]
struct State {
    next: u64,
    /// The asks' events not sent to the SDK yet, in order.
    queued: Vec<Value>,
    /// The asks waiting for their answer.
    waiting: BTreeMap<u64, oneshot::Sender<Option<String>>>,
    /// Who waits for the next queued ask: the events' `wait`.
    waker: Option<Waker>,
}

impl Asks {
    fn state(&self) -> MutexGuard<'_, State> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Queues the event `event`, with `fields` beside its request number,
    /// and gives what the SDK answers.
    fn ask(
        &self,
        event: &str,
        fields: Vec<(&str, Value)>,
    ) -> impl Future<Output = Option<String>> + use<> {
        let (answer, answered) = oneshot::channel();
        {
            let mut state = self.state();
            state.next += 1;
            let request = state.next;
            let mut message = vec![
                ("event", Value::Text(event.into())),
                ("request", Value::Unsigned(request)),
            ];
            message.extend(fields);
            state.queued.push(entries(message));
            state.waiting.insert(request, answer);
            if let Some(waker) = state.waker.take() {
                waker.wake();
            }
        }
        async move { answered.await.ok().flatten() }
    }

    /// The database's admission function: it asks the SDK and waits.
    pub(super) fn fetch(&self) -> crate::admission::Fetch {
        let asks = self.clone();
        Arc::new(move |identity| {
            Box::pin(asks.ask("admission", vec![("identity", Value::Text(identity))]))
        })
    }

    /// The database's relay function: it asks the SDK and waits.
    pub(super) fn relay(&self) -> crate::relay::Given {
        let asks = self.clone();
        Arc::new(move || Box::pin(asks.ask("relay", Vec::new())))
    }

    /// The events of the asks queued since the last take.
    pub(super) fn take(&self) -> Vec<Value> {
        std::mem::take(&mut self.state().queued)
    }

    /// Whether an ask is queued; if not, `cx` is woken when one is.
    pub(super) fn changed(&self, cx: &Context<'_>) -> bool {
        let mut state = self.state();
        if !state.queued.is_empty() {
            return true;
        }
        state.waker = Some(cx.waker().clone());
        false
    }

    /// An answer `{"request": n, <name>: text | null}`, as
    /// `admission.answer` and `relay.answer` give it; `refused` is the
    /// message of one that is not.
    pub(super) fn answer(
        &self,
        args: &Value,
        name: &str,
        refused: &'static str,
    ) -> Result<Value, Failed> {
        let Some(Value::Unsigned(request)) = get(args, "request") else {
            return Err(invalid(refused));
        };
        let text = match get(args, name) {
            Some(Value::Text(text)) => Some(text.clone()),
            None | Some(Value::Null) => None,
            Some(_) => return Err(invalid(refused)),
        };
        if let Some(waiting) = self.state().waiting.remove(request) {
            let _ = waiting.send(text);
        }
        Ok(Value::Null)
    }
}
