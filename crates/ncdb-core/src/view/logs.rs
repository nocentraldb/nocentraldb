//! What the view reads from the logs it keeps in memory: the spaces, each
//! space's log, its tips, its counts and the states of its changes, and the
//! `log` table as the store of the frames the logs released (NOC-452).

use super::View;
use super::log_table::StoredFrames;
use crate::ids::{ChangeId, SpaceId};
use crate::log::{Log, State, StateCounts};
use crate::receive::Checks;
use crate::tips::Tips;

impl View {
    /// The spaces whose log the view holds, in id order.
    pub fn spaces(&self) -> impl Iterator<Item = SpaceId> + '_ {
        self.logs.keys().copied()
    }

    /// The log of `space`, when the view holds one.
    pub fn log(&self, space: SpaceId) -> Option<&Log> {
        self.logs.get(&space)
    }

    /// The tips of the log of `space`, when the view holds one: what the
    /// member's next change in the space takes from it (NOC-138). The view
    /// reads each change's state back from the `log` table without opening
    /// it, so after the view opens, the first read opens each folded
    /// sealed change once with `checks`; see [`Log::tips`].
    pub fn tips(&mut self, space: SpaceId, checks: &impl Checks) -> Option<&Tips> {
        let mut frames = StoredFrames::new(&self.conn);
        Some(self.logs.get_mut(&space)?.tips_in(checks, &mut frames))
    }

    /// The `log` table as the store of the frames the view's logs released
    /// (NOC-452), for a pass over a log's sealed changes, such as
    /// [`Log::after`] or [`crate::moves::later_edits`].
    pub fn stored_frames(&self) -> StoredFrames<'_> {
        StoredFrames::new(&self.conn)
    }

    /// The newest model version the parked data of `space` needs, of the
    /// view's own model (RFC 0002, "What the model version is for"): see
    /// [`Log::newest_model`]. `checks` open the changes it reads.
    pub fn newest_model(&self, space: SpaceId, checks: &impl Checks) -> Option<u64> {
        let mut frames = StoredFrames::new(&self.conn);
        self.logs
            .get(&space)?
            .newest_model_in(checks, self.model.id, &mut frames)
    }

    /// How many changes of `space` are parked and rejected, and how many
    /// operations of its folded changes are parked, per reason (RFC 0009,
    /// "Sync and status"); nothing for a space the view does not hold.
    pub fn state_counts(&self, space: SpaceId) -> StateCounts {
        self.logs
            .get(&space)
            .map(|log| log.state_counts().clone())
            .unwrap_or_default()
    }

    /// The state of a change the log of `space` holds.
    pub fn state(&self, space: SpaceId, id: ChangeId) -> Option<State> {
        self.logs.get(&space)?.state(id).cloned()
    }
}
