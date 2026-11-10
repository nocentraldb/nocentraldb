//! Every space of the member, through `db.spaces.list()` (RFC 0009,
//! "Spaces" and "Spaces of another model"): each space the copy holds,
//! with the member's role in it and whether it is of the lock's model.

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
    /// membership entry of the member has been folded yet.
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
        let inner = self.db.lock();
        let member = inner.member.key().id();
        Ok(inner
            .spaces
            .iter()
            .filter_map(|(&space, state)| {
                Some(SpaceInfo {
                    id: SpaceId(space),
                    role: state.checks.resolved().role(member).map(Role::of),
                    model: SpaceModel::of(state.checks.model_id()?, inner.model_id),
                })
            })
            .collect())
    }
}
