//! The removal the relay hands a removed member (RFC 0007, "Subscribe" and
//! "The errors"; Carsten, 2026-11-20, NOC-383): the one control change
//! that removes it, so its own copy folds the removal and rejects its own
//! changes above the cutoff, as every other copy does (RFC 0008, "A member
//! removed while offline who keeps writing"). The relay reveals nothing
//! else of the space to it.

use ncdb_core::ids::{MemberId, SpaceId};

use crate::Relay;
use crate::error::ErrorCode;
use crate::known::Known;
use crate::store::{Entry, RelayStore};

impl<S: RelayStore> Relay<S> {
    /// The entry of the accepted control change that removed `member` from
    /// `space`, by `resolve` of the space's control changes: its latest
    /// accepted remove, when it is no longer a member. None for a current
    /// member and for a member id the space never held.
    pub async fn removal(
        &self,
        member: MemberId,
        space: SpaceId,
    ) -> Result<Option<Entry>, ErrorCode> {
        let known = self.known(space).await?;
        let removal = removal(&known, member);
        self.keep(space, known);
        Ok(removal)
    }
}

/// The entry of `member`'s latest accepted remove among the control
/// changes `known` holds, when it is no longer a member.
fn removal(known: &Known, member: MemberId) -> Option<Entry> {
    let state = known.resolved.member(member)?;
    if state.role.is_some() {
        return None;
    }
    let revocation = state.revocations.iter().rev().find(|r| r.role.is_none())?;
    known
        .entries
        .iter()
        .map(|(entry, _)| entry)
        .find(|entry| entry.change == revocation.change)
        .cloned()
}
