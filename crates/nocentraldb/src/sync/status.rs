//! Following the sync report, and the confirmation of one write (RFC
//! 0009, "Sync and status"). Whatever may change the status, a write, a
//! message of the relay, a change of the connection, a pause, calls
//! [`Notify::notify`] under the database's lock, and every follower checks
//! again what it waits for. A follower registers its waker under the same
//! lock, so no change slips between its check and its wait.

use std::pin::Pin;
use std::sync::{Arc, Mutex, Weak};
use std::task::{Context, Poll, Waker};

use futures_core::Stream;
use ncdb_core::ids::ChangeId as CoreChangeId;

use super::{SyncReport, SyncStatus, status};
use crate::db::{Inner, lock};
use crate::id::ChangeId;

type Slot = Mutex<Option<Waker>>;

/// The followers of the sync status: each one's waker while it waits.
#[derive(Default)]
pub(crate) struct Notify {
    slots: Vec<Weak<Slot>>,
}

impl Notify {
    /// Wakes every follower that waits; it checks again what it waits for.
    pub(crate) fn notify(&self) {
        for slot in &self.slots {
            if let Some(slot) = slot.upgrade()
                && let Some(waker) = slot_lock(&slot).take()
            {
                waker.wake();
            }
        }
    }

    /// A new follower's slot; forgets the slots of dropped followers.
    fn slot(&mut self) -> Arc<Slot> {
        self.slots.retain(|slot| slot.strong_count() > 0);
        let slot = Arc::new(Mutex::new(None));
        self.slots.push(Arc::downgrade(&slot));
        slot
    }
}

fn slot_lock(slot: &Slot) -> std::sync::MutexGuard<'_, Option<Waker>> {
    slot.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The stream [`SyncReport::subscribe`] returns.
struct Statuses {
    inner: Arc<Mutex<Inner>>,
    slot: Arc<Slot>,
    last: Option<SyncStatus>,
}

impl Stream for Statuses {
    type Item = SyncStatus;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<SyncStatus>> {
        let inner = lock(&self.inner);
        let now = status(&inner);
        if self.last.as_ref() == Some(&now) {
            *slot_lock(&self.slot) = Some(cx.waker().clone());
            return Poll::Pending;
        }
        drop(inner);
        self.last = Some(now.clone());
        Poll::Ready(Some(now))
    }
}

/// Whether the relay recorded a position for the change `id`, in any
/// space the database holds.
fn confirmed_in(inner: &Inner, id: CoreChangeId) -> bool {
    inner
        .spaces
        .keys()
        .any(|space| inner.view.relay_position(*space, id).is_some())
}

impl SyncReport<'_> {
    /// Follows the report: the stream yields the status at once, and
    /// again whenever it changes. Dropping the stream stops it.
    pub fn subscribe(&self) -> impl Stream<Item = SyncStatus> + Unpin + use<> {
        let slot = self.db.lock().status.slot();
        Statuses {
            inner: self.db.inner.clone(),
            slot,
            last: None,
        }
    }

    /// Whether the relay holds the change `change`: it recorded a
    /// position for it in the relay's current epoch. A change the relay
    /// refused, or that the database does not hold, is not confirmed.
    pub fn is_confirmed(&self, change: ChangeId) -> bool {
        confirmed_in(&self.db.lock(), change.0)
    }

    /// Resolves when the relay records a position for the change
    /// `change`. It may never resolve: the relay can refuse the change, or
    /// never be reached (RFC 0009, S3).
    pub async fn confirmed(&self, change: ChangeId) {
        let slot = self.db.lock().status.slot();
        std::future::poll_fn(|cx| {
            let inner = self.db.lock();
            if confirmed_in(&inner, change.0) {
                return Poll::Ready(());
            }
            *slot_lock(&slot) = Some(cx.waker().clone());
            Poll::Pending
        })
        .await;
    }
}
