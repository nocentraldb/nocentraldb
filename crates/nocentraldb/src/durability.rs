//! What a member's copy would lose if it were deleted now (RFC 0006, "What
//! the SDK reports"; NOC-175): this member's own changes the relay has not
//! confirmed, and whether the copy is at risk, by where it is stored.
//!
//! The report stays crate-internal: NOC-176 names it with the TypeScript
//! API, RFC 0006's `db.durability` (NOC-210).
//!
//! **What this does not do yet.** It has no `oldest_unconfirmed_at`: the
//! log keeps no time per change, and where that time comes from is not
//! decided (NOC-231). Nothing reads it before the database on OPFS opens.

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
        at_risk: unconfirmed > 0 && storage != Storage::Persistent,
    }
}
