//! Subscriptions (RFC 0009, "Subscribe"; RFC 0006, "Folding", step 6): a
//! stream of a query's page, at once and again after each fold that
//! touched an object of the query's collection. It passes the whole page,
//! never a diff, so a listener cannot miss a step, and folds that land
//! before the next poll yield one page. It reacts to what was folded and
//! never causes a network request. Dropping the stream stops it.

use std::collections::BTreeSet;
use std::pin::Pin;
use std::sync::{Arc, Mutex, Weak};
use std::task::{Context, Poll, Waker};

use futures_core::Stream;
use ncdb_core::ids::SpaceId as CoreSpaceId;

use crate::db::{Db, Inner, lock};
use crate::error::Error;
use crate::query::{Page, Query};
use crate::space::Space;

/// Whether a subscription's page may have changed since it last yielded,
/// and who waits for it.
#[derive(Default)]
struct Watch {
    stale: bool,
    waker: Option<Waker>,
}

/// What the database keeps of a subscription: what it follows, and its
/// watch while the stream lives.
pub(crate) struct Watcher {
    collection: String,
    space: Option<CoreSpaceId>,
    watch: Weak<Mutex<Watch>>,
}

/// The stream [`Db::subscribe`] and [`Space::subscribe`] return.
struct Subscription {
    inner: Arc<Mutex<Inner>>,
    query: Query,
    space: Option<CoreSpaceId>,
    watch: Arc<Mutex<Watch>>,
}

fn watch(watch: &Mutex<Watch>) -> std::sync::MutexGuard<'_, Watch> {
    watch
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl Stream for Subscription {
    type Item = Result<Page, Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        {
            let mut watch = watch(&self.watch);
            if !watch.stale {
                watch.waker = Some(cx.waker().clone());
                return Poll::Pending;
            }
            watch.stale = false;
        }
        Poll::Ready(Some(self.query.page(&lock(&self.inner), self.space)))
    }
}

/// A subscription to `query`, in `space` or across every space, whose
/// first poll yields the page.
fn subscribe(
    inner: &Arc<Mutex<Inner>>,
    query: Query,
    space: Option<CoreSpaceId>,
) -> impl Stream<Item = Result<Page, Error>> + Unpin + use<> {
    let state = Arc::new(Mutex::new(Watch {
        stale: true,
        waker: None,
    }));
    lock(inner).watchers.push(Watcher {
        collection: query.collection().to_owned(),
        space,
        watch: Arc::downgrade(&state),
    });
    Subscription {
        inner: inner.clone(),
        query,
        space,
        watch: state,
    }
}

/// Marks stale every subscription a fold in `space` that touched
/// `collections` may change, and wakes it; forgets the dropped ones.
pub(crate) fn touched(
    watchers: &mut Vec<Watcher>,
    space: CoreSpaceId,
    collections: &BTreeSet<String>,
) {
    watchers.retain(|watcher| {
        let Some(state) = watcher.watch.upgrade() else {
            return false;
        };
        let follows =
            collections.contains(&watcher.collection) && watcher.space.is_none_or(|s| s == space);
        if follows {
            let mut state = watch(&state);
            state.stale = true;
            if let Some(waker) = state.waker.take() {
                waker.wake();
            }
        }
        true
    });
}

impl Db {
    /// Follows `query`, of an index that spans every space: the stream
    /// yields its page at once and after every fold that touched an object
    /// of its collection.
    pub fn subscribe(
        &self,
        query: Query,
    ) -> impl Stream<Item = Result<Page, Error>> + Unpin + use<> {
        subscribe(&self.inner, query, None)
    }
}

impl Space {
    /// Follows `query`, of an index that runs in one space, in this space.
    pub fn subscribe(
        &self,
        query: Query,
    ) -> impl Stream<Item = Result<Page, Error>> + Unpin + use<> {
        subscribe(self.inner(), query, Some(self.id().0))
    }
}
