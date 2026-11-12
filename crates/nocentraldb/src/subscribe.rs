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
pub(crate) struct Watch {
    stale: bool,
    waker: Option<Waker>,
}

/// What the database keeps of a subscription: what it follows, and its
/// watch while the stream lives; or a fold feed while it lives.
pub(crate) enum Watcher {
    Query {
        collection: String,
        space: Option<CoreSpaceId>,
        watch: Weak<Mutex<Watch>>,
    },
    #[cfg(any(test, feature = "bridge"))]
    Folds(Weak<Mutex<Feed>>),
}

/// The folds a fold feed has not given out yet, in order, and who waits
/// for them.
#[cfg(any(test, feature = "bridge"))]
#[derive(Default)]
pub(crate) struct Feed {
    folds: Vec<(CoreSpaceId, BTreeSet<String>)>,
    closed: bool,
    waker: Option<Waker>,
}

/// The space and collections of every fold, for the TypeScript SDK's
/// worker, which re-runs the reads its subscriptions follow (NOC-176;
/// RFC 0009, "Subscribe"). It carries names and ids, never values.
/// Dropping every clone stops it.
#[cfg(any(test, feature = "bridge"))]
#[derive(Clone)]
pub(crate) struct FoldFeed(Arc<Mutex<Feed>>);

#[cfg(any(test, feature = "bridge"))]
impl FoldFeed {
    fn state(&self) -> std::sync::MutexGuard<'_, Feed> {
        self.0
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The folds since the last take, each its space and the names of the
    /// collections it touched, in order.
    pub(crate) fn take(&self) -> Vec<(crate::id::SpaceId, Vec<String>)> {
        std::mem::take(&mut self.state().folds)
            .into_iter()
            .map(|(space, collections)| {
                (crate::id::SpaceId(space), collections.into_iter().collect())
            })
            .collect()
    }

    /// Resolves when a fold waits to be taken, or the feed is closed.
    pub(crate) fn wait(&self) -> impl std::future::Future<Output = ()> + use<> {
        let feed = self.0.clone();
        std::future::poll_fn(move |cx| {
            let mut state = feed.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            if state.closed || !state.folds.is_empty() {
                return Poll::Ready(());
            }
            state.waker = Some(cx.waker().clone());
            Poll::Pending
        })
    }

    /// Closes the feed: a waiter returns, and no fold is added.
    pub(crate) fn close(&self) {
        let mut state = self.state();
        state.closed = true;
        if let Some(waker) = state.waker.take() {
            waker.wake();
        }
    }

    /// Whether the feed is closed.
    pub(crate) fn is_closed(&self) -> bool {
        self.state().closed
    }
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
    lock(inner).watchers.push(Watcher::Query {
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
    watchers.retain(|watcher| match watcher {
        Watcher::Query {
            collection,
            space: follows_space,
            watch: state,
        } => {
            let Some(state) = state.upgrade() else {
                return false;
            };
            let follows =
                collections.contains(collection) && follows_space.is_none_or(|s| s == space);
            if follows {
                let mut state = watch(&state);
                state.stale = true;
                if let Some(waker) = state.waker.take() {
                    waker.wake();
                }
            }
            true
        }
        #[cfg(any(test, feature = "bridge"))]
        Watcher::Folds(feed) => {
            let Some(feed) = feed.upgrade() else {
                return false;
            };
            let mut feed = feed.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            if feed.closed {
                return false;
            }
            feed.folds.push((space, collections.clone()));
            if let Some(waker) = feed.waker.take() {
                waker.wake();
            }
            true
        }
    });
}

impl Db {
    /// A fold feed of this database: the space and collections of every
    /// fold from now on (NOC-176).
    #[cfg(any(test, feature = "bridge"))]
    pub(crate) fn folds(&self) -> FoldFeed {
        let feed = Arc::new(Mutex::new(Feed::default()));
        lock(&self.inner)
            .watchers
            .push(Watcher::Folds(Arc::downgrade(&feed)));
        FoldFeed(feed)
    }

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
