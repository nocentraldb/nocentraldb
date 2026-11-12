//! The audit of a space's erasures (RFC 0009, "Erasures"; the shapes the
//! owner decided on NOC-246, Carsten, 2026-11-12): for each shred of the
//! space, whether it is final, provisional or taken back, who performed it
//! and which account signed it off, and every member that could have held
//! the person key, with whether it is still a member and whether it
//! acknowledged "deleted", only "seen", or nothing yet (RFC 0008,
//! "Acknowledgement").
//!
//! The members listed are those of the state of the shred's causal past,
//! and any member that acknowledged it later. Every member may list the
//! erasures, a reader too.

use std::collections::BTreeSet;

use ncdb_core::membership::{PersonKey, Resolved};

use crate::db::lock;
use crate::error::{Error, ErrorCode};
use crate::id::ChangeId;
use crate::members::{Fingerprint, MemberId};
use crate::space::Space;

/// The erasures of one space, from [`Space::erasures`].
#[derive(Debug)]
pub struct Erasures<'a> {
    pub(crate) space: &'a Space,
}

/// Where an erasure stands (RFC 0008, "A shred is final when accepted"
/// and "A provisional erasure in a space with one account").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ErasureState {
    /// A shred, or a provisional shred a decision finalized: members
    /// destroy the person key.
    Final,
    /// A provisional shred no decision has ended yet: the fields read as
    /// erased, and members keep the key.
    Provisional,
    /// A provisional shred its account took back: nothing was destroyed.
    TakenBack,
}

/// What a member acknowledged of an erasure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Acknowledged {
    /// It confirmed that it deleted its copy of the person key.
    Deleted,
    /// It saw the shred, but has not confirmed a deletion.
    Seen,
    /// It never acknowledged the shred.
    Missing,
}

/// A member that could have held an erased person key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct ErasureMember {
    /// Its id.
    pub id: MemberId,
    /// The fingerprint of its account.
    pub account: Fingerprint,
    /// Whether it is still a member of the space.
    pub current: bool,
    /// What it acknowledged.
    pub acknowledged: Acknowledged,
}

/// One erasure of a space, from [`Erasures::list`].
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Erasure {
    /// The change id of its shred entry.
    pub shred: ChangeId,
    /// Whether it is final, provisional or taken back.
    pub state: ErasureState,
    /// The member that performed it.
    pub performer: MemberId,
    /// The fingerprint of the performer's account.
    pub performer_account: Fingerprint,
    /// The fingerprint of the account that signed it off.
    pub sign_off: Fingerprint,
    /// The members that could have held the key, in member-id order.
    pub members: Vec<ErasureMember>,
}

impl Space {
    /// The erasures of this space, for their audit.
    pub fn erasures(&self) -> Erasures<'_> {
        Erasures { space: self }
    }
}

impl Erasures<'_> {
    /// The erasures of the space, in shred-id order, as this copy's
    /// membership state holds them: an acknowledgement this copy has not
    /// received yet shows as missing.
    pub async fn list(&self) -> Result<Vec<Erasure>, Error> {
        let inner = lock(self.space.inner());
        let space = self.space.id();
        let state = inner.spaces.get(&space.0).ok_or_else(|| {
            Error::new(
                ErrorCode::NotFound,
                format!("the copy holds no space {space}"),
            )
        })?;
        let resolved = state.checks.resolved();
        let mut erasures: Vec<Erasure> = resolved
            .person_keys()
            .filter_map(|(_, key)| erasure(resolved, key))
            .collect();
        erasures.sort_by_key(|erasure| erasure.shred);
        Ok(erasures)
    }
}

/// The erasure of a person key, if a shred erased it.
fn erasure(resolved: &Resolved, key: &PersonKey) -> Option<Erasure> {
    let shred = key.shred?;
    let performer = key.performer?;
    let state = match (key.provisional, key.finalized, key.taken_back) {
        (false, _, _) | (true, Some(_), _) => ErasureState::Final,
        (true, None, Some(_)) => ErasureState::TakenBack,
        (true, None, None) => ErasureState::Provisional,
    };
    let ids: BTreeSet<_> = key
        .past_members
        .iter()
        .chain(key.acknowledged.keys())
        .copied()
        .collect();
    let members = ids
        .into_iter()
        .filter_map(|id| {
            Some(ErasureMember {
                id: MemberId(id),
                account: account(resolved, id)?,
                current: resolved.role(id).is_some(),
                acknowledged: match key.acknowledged.get(&id) {
                    Some(true) => Acknowledged::Deleted,
                    Some(false) => Acknowledged::Seen,
                    None => Acknowledged::Missing,
                },
            })
        })
        .collect();
    Some(Erasure {
        shred: ChangeId(shred),
        state,
        performer: MemberId(performer),
        performer_account: account(resolved, performer)?,
        sign_off: Fingerprint(key.sign_off?),
        members,
    })
}

/// The fingerprint of a current or former member's account.
fn account(resolved: &Resolved, member: ncdb_core::ids::MemberId) -> Option<Fingerprint> {
    let state = resolved.member(member)?;
    Some(Fingerprint(state.certificate.account_key))
}
