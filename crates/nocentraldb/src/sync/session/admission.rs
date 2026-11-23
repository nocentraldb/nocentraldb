//! Admission on the way up (RFC 0015, "In the SDK"): when the relay
//! refuses a push whose first change is a genesis with `not_allowed`, and
//! the app passed an admission function, the session calls it with the
//! account's identity, presents the answer with `admit`, and pushes the
//! same frames again, once. Whatever goes wrong on the way, the app's
//! function failing, a text that is no admission, or the relay refusing
//! it, leaves the first answer as it was, so the genesis is refused as
//! without the function (pm, 2026-11-23, NOC-469).
//!
//! Only `not_allowed` on a push's first result starts it: the relay sends
//! that code only for a genesis it does not admit (RFC 0007, "Who may
//! create a space").

use ncdb_core::admission::Admission;
use ncdb_core::ids::SpaceId;

use super::super::transport::Transport;
use super::super::wire::{self, Answer, Pushed};
use super::{End, Session, with};

impl<T: Transport> Session<'_, T> {
    /// The results of the push of `frames` to `space`: `results`, or, when
    /// they refuse a genesis and an admission admits it, those of the push
    /// again.
    pub(super) async fn admitted(
        &mut self,
        space: SpaceId,
        frames: &[Vec<u8>],
        results: Vec<Pushed>,
    ) -> Result<Vec<Pushed>, End> {
        let refused =
            matches!(results.first(), Some(Pushed::Rejected(code)) if code == "not_allowed");
        // A genesis is small: a frame that went through an upload is none.
        if !refused || !frames.iter().all(|frame| wire::fits_a_push(frame)) {
            return Ok(results);
        }
        let (fetch, identity) = with(self.db, |inner| {
            (inner.admission.clone(), crate::admission::identity(inner))
        })?;
        let (Some(fetch), Some(identity)) = (fetch, identity) else {
            return Ok(results);
        };
        let Some(text) = fetch(identity).await else {
            return Ok(results);
        };
        let Ok(admission) = text.trim().parse::<Admission>() else {
            return Ok(results);
        };
        let req = self.req();
        self.send(wire::admit(req, admission.encode())).await?;
        match self.ask(req).await? {
            Answer::Error { .. } => return Ok(results),
            Answer::Other { .. } => {}
            answer => {
                self.answer(space, answer)?;
                return Ok(results);
            }
        }
        let req = self.req();
        self.send(wire::push(req, space, frames)).await?;
        match self.ask(req).await? {
            Answer::Pushed { results, .. } => Ok(results),
            answer => {
                self.answer(space, answer)?;
                Ok(results)
            }
        }
    }
}
