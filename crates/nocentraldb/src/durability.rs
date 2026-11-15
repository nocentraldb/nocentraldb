//! What a member's copy would lose if it were deleted now (RFC 0006, "What
//! the SDK reports"; NOC-175): this member's own changes the relay has not
//! confirmed, and whether the copy is at risk, by where it is stored.
//!
//! The report stays crate-internal: NOC-176 names it with the TypeScript
//! API, RFC 0006's `db.durability` (NOC-210).
//!
//! The time of the oldest unconfirmed change comes from the log's
//! `written_at`, which the SDK keeps with every change it writes (RFC 0002,
//! "On a member"; NOC-231).

use crate::db::Inner;

/// Where a member's copy is stored, as RFC 0006 names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) enum Storage {
    /// A native app's file, or a browser copy the browser keeps persistent.
    Persistent,
    /// A browser copy on OPFS without persistent storage: the browser may
    /// delete it.
    BestEffort,
    /// A copy in memory, where OPFS could not be opened.
    Temporary,
}

/// The durability of a member's copy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Durability {
    /// Where the copy is stored.
    pub(crate) storage: Storage,
    /// How many changes this member wrote that have no relay position: the
    /// relay has not confirmed them, or refused them. They exist only on
    /// this device.
    pub(crate) unconfirmed: u64,
    /// When the member wrote the oldest of them, in milliseconds since the
    /// Unix epoch; `None` when there is none, or when it was written before
    /// the log kept the time.
    pub(crate) oldest_unconfirmed_at: Option<u64>,
    /// Whether deleting the copy now would lose changes: some are
    /// unconfirmed and the copy is not persistent.
    pub(crate) at_risk: bool,
}

/// The durability of the copy `inner` holds, stored as `storage`.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn durability(inner: &Inner, storage: Storage) -> Durability {
    let me = inner.member.key().id();
    let unconfirmed = inner
        .view
        .spaces()
        .map(|space| {
            let counts = inner.view.relay_counts(space, me);
            counts.unconfirmed + counts.refused
        })
        .sum();
    Durability {
        storage,
        unconfirmed,
        oldest_unconfirmed_at: inner.view.oldest_unconfirmed_own().ok().flatten(),
        at_risk: unconfirmed > 0 && storage != Storage::Persistent,
    }
}

#[cfg(any(test, feature = "bridge"))]
impl crate::db::Db {
    /// The durability now, and a [`Watch`](crate::sync::Watch) of it from
    /// now on, for the bridge's events (NOC-315).
    pub(crate) fn durability_watch(&self) -> (Durability, crate::sync::Watch<Durability>) {
        crate::sync::watch(&self.inner, |inner: &Inner| {
            durability(inner, inner.storage)
        })
    }

    /// Reads again whether the browser keeps this copy persistent, after the
    /// page asked with `persist()` or became visible again (RFC 0006, "What
    /// the SDK reports"). A copy in memory stays temporary; natively the
    /// storage never changes.
    pub(crate) async fn refresh_storage(&self) {
        #[cfg(target_arch = "wasm32")]
        {
            if crate::db::lock(&self.inner).storage == Storage::Temporary {
                return;
            }
            let storage = match crate::browser::persisted().await {
                true => Storage::Persistent,
                false => Storage::BestEffort,
            };
            let mut inner = crate::db::lock(&self.inner);
            if inner.storage != storage {
                inner.storage = storage;
                inner.status.notify();
            }
        }
    }
}
