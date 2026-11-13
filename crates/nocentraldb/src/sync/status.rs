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

/// A stream of what `read` reads of the database: at once, and again
/// whenever it differs from what the stream last yielded. Whatever may
/// change it calls [`Notify::notify`]. Dropping the stream stops it.
struct Follow<T, F> {
    inner: Arc<Mutex<Inner>>,
    slot: Arc<Slot>,
    last: Option<T>,
    read: F,
}

impl<T, F> Stream for Follow<T, F>
where
    T: Clone + PartialEq + Unpin,
    F: Fn(&Inner) -> T + Unpin,
{
    type Item = T;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<T>> {
        let inner = lock(&self.inner);
        let now = (self.read)(&inner);
        if self.last.as_ref() == Some(&now) {
            *slot_lock(&self.slot) = Some(cx.waker().clone());
            return Poll::Pending;
        }
        drop(inner);
        self.last = Some(now.clone());
        Poll::Ready(Some(now))
    }
}

/// Follows what `read` reads of the database `inner`; see [`Follow`].
pub(crate) fn follow<T, F>(
    inner: &Arc<Mutex<Inner>>,
    read: F,
) -> impl Stream<Item = T> + Unpin + use<T, F>
where
    T: Clone + PartialEq + Unpin,
    F: Fn(&Inner) -> T + Unpin,
{
    let slot = lock(inner).status.slot();
    Follow {
        inner: inner.clone(),
        slot,
        last: None,
        read,
    }
}

/// What `read` reads of a database, followed without keeping the database
/// open: the bridge's events hold it (NOC-176), and a closed database
/// must not stay open for them. Whatever may change it calls
/// [`Notify::notify`], as for [`Follow`].
#[cfg(any(test, feature = "bridge"))]
pub(crate) struct Watch<T> {
    inner: Weak<Mutex<Inner>>,
    slot: Arc<Slot>,
    last: T,
    read: Box<dyn Fn(&Inner) -> T + Send>,
}

#[cfg(any(test, feature = "bridge"))]
impl<T: Clone + PartialEq> Watch<T> {
    /// What `read` reads now, when it differs from what was last taken;
    /// `None` when it does not, or the database is gone.
    pub(crate) fn take(&mut self) -> Option<T> {
        let inner = self.inner.upgrade()?;
        let now = (self.read)(&lock(&inner));
        if now == self.last {
            return None;
        }
        self.last = now.clone();
        Some(now)
    }

    /// Whether what `read` reads differs from what was last taken. Wakes
    /// `waker` on the next change, so a caller that registers before it
    /// checks misses none.
    pub(crate) fn changed(&self, waker: &Waker) -> bool {
        let Some(inner) = self.inner.upgrade() else {
            return false;
        };
        let inner = lock(&inner);
        *slot_lock(&self.slot) = Some(waker.clone());
        (self.read)(&inner) != self.last
    }
}

/// Watches what `read` reads of the database `inner`, from what it reads
/// now, which it returns; see [`Watch`].
#[cfg(any(test, feature = "bridge"))]
pub(crate) fn watch<T: Clone>(
    inner: &Arc<Mutex<Inner>>,
    read: impl Fn(&Inner) -> T + Send + 'static,
) -> (T, Watch<T>) {
    let mut locked = lock(inner);
    let slot = locked.status.slot();
    let now = read(&locked);
    drop(locked);
    (
        now.clone(),
        Watch {
            inner: Arc::downgrade(inner),
            slot,
            last: now,
            read: Box::new(read),
        },
    )
}

/// Whether the relay recorded a position for the change `id`, in any
/// space the database holds.
fn confirmed_in(inner: &Inner, id: CoreChangeId) -> bool {
    inner
        .spaces
        .keys()
        .any(|space| inner.view.relay_position(*space, id).is_some())
}

#[cfg(any(test, feature = "bridge"))]
impl SyncReport<'_> {
    /// The report now, and a [`Watch`] of it from now on.
    pub(crate) fn watch(&self) -> (SyncStatus, Watch<SyncStatus>) {
        watch(&self.db.inner, status)
    }
}

impl SyncReport<'_> {
    /// Follows the report: the stream yields the status at once, and
    /// again whenever it changes. Dropping the stream stops it.
    pub fn subscribe(&self) -> impl Stream<Item = SyncStatus> + Unpin + use<> {
        follow(&self.db.inner, status)
    }

    /// Whether the relay holds the change `change`: it recorded a
    /// position for it in the relay's current epoch. A change the relay
    /// refused, or that the database does not hold, is not confirmed.
    pub fn is_confirmed(&self, change: ChangeId) -> bool {
        confirmed_in(&self.db.lock(), change.0)
    }

    /// Resolves when the relay records a position for the change
    /// `change`. It may never resolve: the relay can refuse the change, or
    /// never be reached (RFC 0009, "One write, one change").
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
