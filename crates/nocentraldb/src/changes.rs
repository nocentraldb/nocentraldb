//! The change event (RFC 0009, "Subscribe": `db.changes.subscribe`; RFC
//! 0006, "Folding", step 6), in Rust: `db.changes()` gives a [`Changes`],
//! whose `subscribe()` yields one [`Change`] per collection each fold
//! touched, in the order of the folds. A change names its space, its
//! collection, the ids it touched and its [`Origin`]: this member's own
//! write, or one that sync brought. It carries ids, never values, so an
//! adapter or an agent's wake-up logic reads the objects itself.

use std::collections::VecDeque;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_core::Stream;

use crate::db::Db;
use crate::id::{ObjectId, SpaceId};
use crate::subscribe::{FoldFeed, Origin};

/// The change event of a database, from [`Db::changes`].
#[derive(Debug)]
pub struct Changes<'a> {
    db: &'a Db,
}

/// What one fold did in one collection, as
/// [`Changes::subscribe`] yields it: TypeScript's `{ space, collection,
/// ids, origin }`.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Change {
    /// The space the fold was in.
    pub space: SpaceId,
    /// The collection of the objects it touched.
    pub collection: String,
    /// The objects it touched in that collection, without their values.
    pub ids: Vec<ObjectId>,
    /// Whether the change was this member's own write or came by sync.
    pub origin: Origin,
}

impl Db {
    /// The change event: what each fold touched, from now on.
    pub fn changes(&self) -> Changes<'_> {
        Changes { db: self }
    }
}

impl Changes<'_> {
    /// Follows the folds from now on: the stream yields one [`Change`] per
    /// collection each fold touched, in order. It never yields a value
    /// and never causes a network request. Dropping the stream stops it.
    pub fn subscribe(&self) -> impl Stream<Item = Change> + Unpin + use<> {
        Following {
            feed: self.db.folds(),
            ready: VecDeque::new(),
        }
    }
}

/// The stream [`Changes::subscribe`] returns: a fold feed, and the changes
/// it gave out that the stream has not yielded yet.
struct Following {
    feed: FoldFeed,
    ready: VecDeque<Change>,
}

impl Following {
    /// Queues the changes of `folds` to be yielded, in order.
    fn queue(&mut self, folds: Vec<crate::subscribe::Fold>) {
        for fold in folds {
            let space = fold.space;
            self.ready
                .extend(fold.changes.into_iter().map(|changed| Change {
                    space,
                    collection: changed.collection,
                    ids: changed.ids,
                    origin: changed.origin,
                }));
        }
    }
}

impl Stream for Following {
    type Item = Change;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Change>> {
        loop {
            if let Some(change) = self.ready.pop_front() {
                return Poll::Ready(Some(change));
            }
            let folds = self.feed.take();
            if !folds.is_empty() {
                self.queue(folds);
                continue;
            }
            if self.feed.poll_wait(cx).is_pending() {
                return Poll::Pending;
            }
            // Ready with no fold waiting: the feed was closed.
            let folds = self.feed.take();
            if folds.is_empty() {
                return Poll::Ready(None);
            }
            self.queue(folds);
        }
    }
}
