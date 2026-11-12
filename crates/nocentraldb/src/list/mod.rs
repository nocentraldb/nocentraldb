//! Every space of the member, through `db.spaces.list()` (RFC 0009,
//! "Spaces" and "Spaces of another model"): each space the copy holds,
//! with the member's role in it and whether it is of the lock's model.

use futures_core::Stream;

use crate::db::Inner;
use crate::error::Error;
use crate::id::SpaceId;
use crate::members::Role;
use crate::space::Spaces;

/// One space of the member, from [`Spaces::list`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SpaceInfo {
    /// The space id.
    pub id: SpaceId,
    /// The member's current role in the space, from the membership the
    /// copy folded: `None` once the member was removed, or while no
    /// membership entry of the member has been folded yet. A member the
    /// relay answered `not_member` or `removed_member` for the space is
    /// removed, even though the relay serves it no removal to fold.
    pub role: Option<Role>,
    /// Whether the space is of the lock's model.
    pub model: SpaceModel,
}

/// Whether a space is of the lock's model (RFC 0009, "Spaces of another
/// model").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SpaceModel {
    /// The space's genesis names the lock's `model.id`.
    Ours,
    /// The space's genesis names another `model.id`.
    Another,
}

impl SpaceModel {
    /// Whether the `model.id` a genesis names, `space`, is the lock's,
    /// `lock`.
    pub(crate) fn of(space: [u8; 16], lock: [u8; 16]) -> SpaceModel {
        match space == lock {
            true => SpaceModel::Ours,
            false => SpaceModel::Another,
        }
    }
}

impl Spaces<'_> {
    /// Every space the copy holds, in the order of their ids. A space is
    /// listed once the copy holds its genesis and the space key of its
    /// first generation, which opens the `model.id` the genesis names; a
    /// space the member joined is left out until then, as
    /// [`Spaces::get`] answers `NotFound` for it.
    pub async fn list(&self) -> Result<Vec<SpaceInfo>, Error> {
        Ok(list(&self.db.lock()))
    }

    /// Follows the space list (Carsten, 2026-11-12, NOC-241): the stream
    /// yields the list [`Spaces::list`] gives at once, then each new list
    /// when a space is added, left or removed, or the member's role in one
    /// changes, as when another device shares a space with this member. A
    /// list that did not change is not yielded again. Dropping the stream
    /// stops it.
    pub fn subscribe(&self) -> impl Stream<Item = Vec<SpaceInfo>> + Unpin + use<> {
        crate::sync::follow(&self.db.inner, list)
    }
}

/// Every space of `inner` [`Spaces::list`] lists, in the order of their ids:
/// the account space is no app space, so it is left out (RFC 0010, "The
/// account space").
fn list(inner: &Inner) -> Vec<SpaceInfo> {
    let member = inner.member.key().id();
    let account = crate::account::id();
    inner
        .spaces
        .iter()
        .filter(|(_, state)| state.checks.model_id() != Some(account))
        .filter_map(|(&space, state)| {
            Some(SpaceInfo {
                id: SpaceId(space),
                role: match crate::sync::stopped(inner, space) {
                    true => None,
                    false => state.checks.resolved().role(member).map(Role::of),
                },
                model: SpaceModel::of(state.checks.model_id()?, inner.model_id),
            })
        })
        .collect()
}
