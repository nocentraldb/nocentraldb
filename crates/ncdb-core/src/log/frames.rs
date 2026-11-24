//! Where a log reads a change whose bytes it no longer holds in memory
//! (NOC-452): the store that keeps the log's frames, such as the view's
//! `log` table. A log keeps every change in memory until its store says
//! it keeps the frame ([`Log::release`](super::Log::release)); a log
//! without a store, such as the relay's or a test's, never releases one.

use std::borrow::Cow;

use crate::change::Change;
use crate::ids::{ChangeId, SpaceId};
use crate::receive::Fold;

/// A store of a log's frames.
pub trait Frames {
    /// The change `id` of `space` as the store keeps it, or `None` when it
    /// keeps none. A store that cannot read it records the error for its
    /// caller and gives `None`.
    fn stored(&mut self, space: SpaceId, id: ChangeId) -> Option<Change>;
}

/// A fold reads frames from the store it folds into: [`Fold::stored`].
impl<F: Fold + ?Sized> Frames for F {
    fn stored(&mut self, space: SpaceId, id: ChangeId) -> Option<Change> {
        <F as Fold>::stored(self, space, id)
    }
}

/// No store: for a log that holds every change in memory.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoFrames;

impl Frames for NoFrames {
    fn stored(&mut self, _space: SpaceId, _id: ChangeId) -> Option<Change> {
        None
    }
}

/// A change the log holds in memory, borrowed, or one read from its
/// store, owned.
pub type Held<'a> = Cow<'a, Change>;
