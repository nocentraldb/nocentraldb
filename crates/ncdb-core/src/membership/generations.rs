//! The generation graph of RFC 0008 ("Generation ids"): the space's first
//! generation, and every generation a key chain entry that is valid when
//! written names, accepted or later rejected (decision M17), each linked to
//! its previous generations. "Descends from" follows those links.

use std::collections::{BTreeMap, BTreeSet};

use crate::ids::{ChangeId, MemberId};

/// One generation of the space key.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Generation {
    /// The generations it follows; none for the first.
    pub previous: BTreeSet<u64>,
    /// The key chain entries that name it, valid when written, with their
    /// authors; none for the first. Two of them share an id only through
    /// equivocation or a hash collision (decision M23).
    pub entries: BTreeMap<ChangeId, MemberId>,
    /// The recipients of every key envelope for it in the set, whatever its
    /// verdict, since a rejected envelope may still have been delivered.
    pub recipients: BTreeSet<MemberId>,
}

/// The generation graph, by generation id.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Generations(pub(crate) BTreeMap<u64, Generation>);
