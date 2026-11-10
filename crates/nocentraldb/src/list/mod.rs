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

impl Spaces<'_> {
    /// Every space the copy holds.
    pub async fn list(&self) -> Result<Vec<SpaceInfo>, Error> {
        let inner = self.db.lock();
        let member = inner.member.key().id();
        Ok(inner
            .spaces
            .iter()
            .map(|(&space, state)| SpaceInfo {
                id: SpaceId(space),
                role: state.checks.resolved().role(member).map(Role::of),
                model: SpaceModel::Ours,
            })
            .collect())
    }
}

#[cfg(test)]
mod tests;
