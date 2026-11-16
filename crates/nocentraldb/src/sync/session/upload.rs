//! Large changes on the way up (RFC 0007, "Large changes"): a frame too
//! large for one push goes to an upload URL the relay hands out, and the
//! push names the upload.

use ncdb_core::ids::{ChangeId, SpaceId};

use super::super::transport::Transport;
use super::super::{apply, large, wire};
use super::{End, Session, lost, with};
use wire::Answer;

/// What became of an upload.
pub(super) enum Uploaded {
    /// The frame is uploaded under this id; the push names it.
    Id(Vec<u8>),
    /// The relay refused the frame as too large; it is recorded as refused.
    Refused,
    /// The relay answered with another error, handled as an answer to the
    /// push, which stops.
    Stopped,
}

impl<T: Transport> Session<'_, T> {
    /// Uploads `frame`, too large for one push, to an upload URL the relay
    /// hands out (RFC 0007, "Large changes"). A frame the relay refuses as
    /// `too_large` is recorded as refused, as a push would; any other error
    /// is handled as an answer to the push, which then stops. An upload that
    /// fails ends the connection, and the change is pushed again on the next
    /// one.
    pub(super) async fn upload(
        &mut self,
        space: SpaceId,
        change: ChangeId,
        frame: &[u8],
    ) -> Result<Uploaded, End> {
        let req = self.req();
        self.send(wire::upload(req, space, frame.len())).await?;
        match self.ask(req).await? {
            Answer::Upload { id, url, .. } => {
                large::upload(&url, frame).await.map_err(|_| lost())?;
                Ok(Uploaded::Id(id))
            }
            Answer::Error { code, .. } if code == "too_large" => {
                let refused = vec![wire::Pushed::Rejected(code)];
                with(self.db, |inner| {
                    apply::pushed(inner, space, &[change], refused)
                })?
                .map_err(|_| lost())?;
                Ok(Uploaded::Refused)
            }
            answer => self.answer(space, answer).map(|()| Uploaded::Stopped),
        }
    }
}
