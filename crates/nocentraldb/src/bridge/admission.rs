//! The app's admission function across the bridge (RFC 0015, "In the
//! SDK"): the function lives with the app, on the other side of the
//! worker, so the database's function asks the SDK and waits. Each ask
//! gets a request number and goes out as `{"event": "admission",
//! "request": n, "identity": hex}`; `admission.answer` `{"request": n,
//! "admission": text | null}` gives the waiting sync that text, or nothing.
//! An answer to a request nobody waits for is ignored.

use std::collections::BTreeMap;
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
    /// The asks not sent to the SDK yet, in order.
    queued: Vec<(u64, String)>,
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

    /// The database's admission function: it asks the SDK and waits.
    pub(super) fn fetch(&self) -> crate::admission::Fetch {
        let asks = self.clone();
        Arc::new(move |identity| {
            let (answer, answered) = oneshot::channel();
            {
                let mut state = asks.state();
                state.next += 1;
                let request = state.next;
                state.queued.push((request, identity));
                state.waiting.insert(request, answer);
                if let Some(waker) = state.waker.take() {
                    waker.wake();
                }
            }
            Box::pin(async move { answered.await.ok().flatten() })
        })
    }

    /// The `admission` events queued since the last take.
    pub(super) fn take(&self) -> Vec<Value> {
        let queued = std::mem::take(&mut self.state().queued);
        queued
            .into_iter()
            .map(|(request, identity)| {
                entries([
                    ("event", Value::Text("admission".into())),
                    ("request", Value::Unsigned(request)),
                    ("identity", Value::Text(identity)),
                ])
            })
            .collect()
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

    /// `admission.answer` `{"request": n, "admission": text | null}`.
    pub(super) fn answer(&self, args: &Value) -> Result<Value, Failed> {
        let Some(Value::Unsigned(request)) = get(args, "request") else {
            return Err(invalid(
                crate::texts::bridge::ADMISSION_ANSWER_TAKES_AREQUEST,
            ));
        };
        let text = match get(args, "admission") {
            Some(Value::Text(text)) => Some(text.clone()),
            None | Some(Value::Null) => None,
            Some(_) => {
                return Err(invalid(
                    crate::texts::bridge::ADMISSION_ANSWER_TAKES_AREQUEST,
                ));
            }
        };
        if let Some(waiting) = self.state().waiting.remove(request) {
            let _ = waiting.send(text);
        }
        Ok(Value::Null)
    }
}
