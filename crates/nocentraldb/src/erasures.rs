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

use std::collections::{BTreeMap, BTreeSet};

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
            .flat_map(|(_, key)| erasures(resolved, key))
            .collect();
        erasures.sort_by_key(|erasure| erasure.shred);
        Ok(erasures)
    }
}

/// The erasures of a person key: each earlier shred, which was taken back,
/// and the shred that took effect (RFC 0008, "The function"; Carsten,
/// 2026-11-12, NOC-242).
fn erasures<'a>(resolved: &'a Resolved, key: &'a PersonKey) -> impl Iterator<Item = Erasure> + 'a {
    let earlier = key.earlier.iter().filter_map(move |earlier| {
        erasure(
            resolved,
            Shred {
                id: earlier.id,
                state: ErasureState::TakenBack,
                performer: earlier.performer?,
                sign_off: earlier.sign_off?,
                past_members: &earlier.past_members,
                acknowledged: &earlier.acknowledged,
            },
        )
    });
    let current = key.shred.and_then(|id| {
        let state = match (key.provisional, key.finalized, key.taken_back) {
            (false, _, _) | (true, Some(_), _) => ErasureState::Final,
            (true, None, Some(_)) => ErasureState::TakenBack,
            (true, None, None) => ErasureState::Provisional,
        };
        erasure(
            resolved,
            Shred {
                id,
                state,
                performer: key.performer?,
                sign_off: key.sign_off?,
                past_members: &key.past_members,
                acknowledged: &key.acknowledged,
            },
        )
    });
    earlier.chain(current)
}

/// What the audit reads of one shred of a person key.
struct Shred<'a> {
    id: ncdb_core::ids::ChangeId,
    state: ErasureState,
    performer: ncdb_core::ids::MemberId,
    sign_off: [u8; 32],
    past_members: &'a BTreeSet<ncdb_core::ids::MemberId>,
    acknowledged: &'a BTreeMap<ncdb_core::ids::MemberId, bool>,
}

/// The erasure of one shred: every member of its causal past or that
/// acknowledged it, with whether it is current and what it acknowledged.
fn erasure(resolved: &Resolved, shred: Shred<'_>) -> Option<Erasure> {
    let ids: BTreeSet<_> = shred
        .past_members
        .iter()
        .chain(shred.acknowledged.keys())
        .copied()
        .collect();
    let members = ids
        .into_iter()
        .filter_map(|id| {
            Some(ErasureMember {
                id: MemberId(id),
                account: account(resolved, id)?,
                current: resolved.role(id).is_some(),
                acknowledged: match shred.acknowledged.get(&id) {
                    Some(true) => Acknowledged::Deleted,
                    Some(false) => Acknowledged::Seen,
                    None => Acknowledged::Missing,
                },
            })
        })
        .collect();
    Some(Erasure {
        shred: ChangeId(shred.id),
        state: shred.state,
        performer: MemberId(shred.performer),
        performer_account: account(resolved, shred.performer)?,
        sign_off: of_account_key(resolved, shred.sign_off),
        members,
    })
}

/// The fingerprint of the account whose key is `key`: over the recovery
/// key a current member's certificate of that account states, or over the
/// key itself when none states one (RFC 0003, "Account fingerprint").
pub(crate) fn of_account_key(resolved: &Resolved, key: [u8; 32]) -> Fingerprint {
    resolved
        .members()
        .filter_map(|(id, _)| resolved.member(id))
        .map(|state| &state.certificate)
        .find(|certificate| certificate.account_key == key && certificate.statement.is_some())
        .map_or(Fingerprint::account(key), Fingerprint::of)
}

/// The fingerprint of a current or former member's account.
fn account(resolved: &Resolved, member: ncdb_core::ids::MemberId) -> Option<Fingerprint> {
    let state = resolved.member(member)?;
    Some(Fingerprint::of(&state.certificate))
}
