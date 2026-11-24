//! Whether this member is a space's only member (RFC 0009, "Sync and
//! status"; RFC 0010, "What it restores"; NOC-494). Recovery restores the
//! account, not its spaces: a space comes back only when another member
//! adds the new device, so a space whose only member is this one is lost
//! with this member's copy.

use ncdb_core::ids::MemberId;
use ncdb_core::membership::Resolved;

/// Whether `me` holds a role in the space and no other member does.
pub(super) fn only_member(resolved: &Resolved, me: MemberId) -> bool {
    let mut members = resolved.members().map(|(id, _)| id);
    members.next() == Some(me) && members.next().is_none()
}
