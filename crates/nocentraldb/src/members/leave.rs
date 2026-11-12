//! Leaving a space (RFC 0009, "Spaces"; RFC 0008, "Roles and rights" and
//! "Membership entry"): the member writes a removal of itself, with its own
//! highest author sequence as the cutoff. It writes no rotation: a member
//! that leaves must not know the next key, so an admin rotates for it, and
//! until then the space stays on a generation the leaver holds, which every
//! member's status reports as exposed. An admin's SDK that syncs writes
//! that rotation on its own, a merge rotation, after a random delay of up
//! to a minute (`members::merge`, `sync::merge`; NOC-220).

use ncdb_core::membership::{self, Action, Entry, MembershipEntry};

use super::change::{admins, write};
use super::{Notice, holds};
use crate::db::lock;
use crate::error::{Error, ErrorCode};
use crate::space::{Space, not_stopped, writable};

impl Space {
    /// Leaves the space: writes a removal of this member with its own
    /// highest author sequence as the cutoff, folds it, and returns the
    /// [`Notice`] `SpaceLeft`. The member keeps its copy of everything it
    /// received before; until an admin rotates the space key, the others
    /// write under a key it still holds.
    ///
    /// The space's last admin is refused with `LastAdmin`, as RFC 0008
    /// requires; two admins that leave at the same time cannot be kept from
    /// leaving the space without one. A member that was removed already is
    /// refused with `NotMember`. Nothing is written when a call is refused.
    pub async fn leave(&self) -> Result<Notice, Error> {
        let mut inner = lock(self.inner());
        let space = self.id();
        writable(&inner, space)?;
        not_stopped(&inner, space)?;
        let me = inner.member.key().id();
        {
            let resolved = inner.spaces[&space.0].checks.resolved();
            holds(resolved, me, membership::Role::Reader, "leaves it")?;
            if resolved.role(me) == Some(membership::Role::Admin) && admins(resolved) == 1 {
                return Err(Error::new(
                    ErrorCode::LastAdmin,
                    "the space's last admin may not leave it",
                ));
            }
        }
        write(&mut inner, space.0, me, |batch, _, key, cutoff| {
            batch.write(
                key,
                Entry::Membership(MembershipEntry {
                    subject: me,
                    action: Action::Remove { cutoff },
                }),
                None,
            );
        })?;
        // The account space no longer records the space (RFC 0010).
        crate::account::reconcile(&mut inner)?;
        Ok(Notice::SPACE_LEFT)
    }
}
