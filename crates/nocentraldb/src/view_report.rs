//! The view's own report (RFC 0009, "Sync and status": `db.view`; RFC
//! 0006, "What a rebuild costs"), with the Rust names the owner decided on
//! NOC-227: `db.view()` gives a [`ViewReport`], whose `current()` gives a
//! [`ViewStatus`] and whose `subscribe()` follows it. It says whether a
//! rebuild of the view runs, and how far it is, in frames of the log.
//!
//! `Db::open` resolves before a rebuild ends (RFC 0009, "What `nc.open`
//! does", step 4). The app reads the old view meanwhile, and a thread of
//! the SDK runs the rebuild a page at a time. It holds the database's lock
//! for one page only, so reads and writes go on between pages. Once the
//! core swaps the new view in, the SDK reads and writes by the new model,
//! and every subscription is told its data may have changed.

use futures_core::Stream;

use crate::db::{Db, Inner};

#[cfg(not(target_arch = "wasm32"))]
pub(crate) use rebuild::start;

/// The report of the view, from [`Db::view`].
#[derive(Debug)]
pub struct ViewReport<'a> {
    db: &'a Db,
}

/// What [`ViewReport::current`] gives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct ViewStatus {
    /// How far a rebuild of the view is, or `None` when none runs.
    pub rebuild: Option<RebuildProgress>,
}

/// How far a rebuild of the view is, in frames of the log: `folded` of
/// `total`. The total grows while changes arrive during the rebuild.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct RebuildProgress {
    /// The frames the rebuild has folded.
    pub folded: u64,
    /// The frames of the log.
    pub total: u64,
}

impl Db {
    /// The report of the view: whether a rebuild runs, and how far it is.
    pub fn view(&self) -> ViewReport<'_> {
        ViewReport { db: self }
    }
}

impl ViewReport<'_> {
    /// The status as it is now.
    pub fn current(&self) -> ViewStatus {
        status(&self.db.lock())
    }

    /// Follows the status: the stream yields it at once, and again
    /// whenever it changes. Dropping the stream stops it.
    pub fn subscribe(&self) -> impl Stream<Item = ViewStatus> + Unpin + use<> {
        crate::sync::follow(&self.db.inner, status)
    }
}

#[cfg(any(test, feature = "bridge"))]
impl ViewReport<'_> {
    /// The status now, and a [`Watch`](crate::sync::Watch) of it from now
    /// on, for the bridge's events (NOC-315).
    pub(crate) fn watch(&self) -> (ViewStatus, crate::sync::Watch<ViewStatus>) {
        crate::sync::watch(&self.db.inner, status)
    }
}

/// The view's status in the database `inner`.
fn status(inner: &Inner) -> ViewStatus {
    ViewStatus {
        rebuild: inner
            .view
            .rebuild_progress()
            .map(|progress| RebuildProgress {
                folded: progress.folded,
                total: progress.total,
            }),
    }
}

/// The thread that runs a rebuild, natively: wasm32 starts no thread yet
/// (`Db::open`), so a browser's view rebuilds when a later slice runs it.
#[cfg(not(target_arch = "wasm32"))]
mod rebuild {
    use std::collections::BTreeSet;
    use std::sync::{Arc, Mutex, Weak};

    use crate::db::{Inner, lock};
    use crate::subscribe::{Folded, Origin, touched};

    /// How many frames of the log the rebuild folds while it holds the lock.
    const PAGE: usize = 128;

    /// Runs the rebuild of the database `inner`'s view in a thread of its own,
    /// when one runs. The thread stops once the rebuild ends, fails or the
    /// database is dropped. A rebuild that fails stays reported and starts
    /// again at the next open.
    pub(crate) fn start(inner: &Arc<Mutex<Inner>>) {
        if lock(inner).view.rebuild_progress().is_none() {
            return;
        }
        let db = Arc::downgrade(inner);
        let _ = std::thread::Builder::new()
            .name("nocentraldb-rebuild".into())
            .spawn(move || run(&db));
    }

    fn run(db: &Weak<Mutex<Inner>>) {
        while let Some(inner) = db.upgrade() {
            let mut inner = lock(&inner);
            let step = inner.view.rebuild_step(PAGE);
            if matches!(step, Ok(None)) {
                swapped(&mut inner);
            }
            inner.status.notify();
            if !matches!(step, Ok(Some(_))) {
                return;
            }
            drop(inner);
            std::thread::yield_now();
        }
    }

    /// After the swap: the SDK reads and writes by the view's new model, and
    /// every subscription reads its data again.
    fn swapped(inner: &mut Inner) {
        inner.model = Arc::new(crate::account::app_model(inner.view.model()));
        let collections: BTreeSet<String> = inner.model.collections.keys().cloned().collect();
        let spaces: Vec<_> = inner.spaces.keys().copied().collect();
        // A swap folds no change: subscriptions read again, and no object
        // is named as touched.
        for space in spaces {
            touched(
                &mut inner.watchers,
                &Folded {
                    space,
                    collections: &collections,
                    touched: &[],
                    origin: Origin::Remote,
                },
            );
        }
    }
}
