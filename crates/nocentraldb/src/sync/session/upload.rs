//! Large changes on the way up (RFC 0007, "Large changes"): a frame too
//! large for one push goes to an upload URL the relay hands out, and the
//! push names the upload.
//!
//! An upload S3 refuses with an HTTP status keeps the connection
//! (Carsten, 2026-11-16, NOC-349, option A): `SyncStatus::error` shows
//! `upload_failed`, the change stays unconfirmed, and its space pushes
//! again after a wait that starts at the reconnect's first and doubles up
//! to its longest. An upload that goes through clears the code. Any other
//! failure of the upload, such as a lost network, ends the connection.

use std::collections::BTreeMap;
use std::time::Duration;

use ncdb_core::ids::{ChangeId, SpaceId};

use super::super::http::Failed;
use super::super::run::{FIRST, LONGEST};
use super::super::time::Instant;
use super::super::transport::Transport;
use super::super::{apply, large, wire};
use super::{End, Session, lost, with};
use crate::texts::part::UPLOAD_FAILED;
use wire::Answer;

/// The spaces whose upload S3 refused: when each pushes again, if it is
/// not already due, and the wait that led there.
#[derive(Default)]
pub(super) struct Retries(BTreeMap<SpaceId, (Option<Instant>, Duration)>);

impl Retries {
    /// Whether `space` waits to push again.
    pub(super) fn waiting(&self, space: SpaceId, now: Instant) -> bool {
        matches!(self.0.get(&space), Some((Some(at), _)) if *at > now)
    }

    /// The spaces whose wait ended; each is due once.
    pub(super) fn due(&mut self, now: Instant) -> Vec<SpaceId> {
        let mut due = Vec::new();
        for (space, (at, _)) in &mut self.0 {
            if at.is_some_and(|at| at <= now) {
                *at = None;
                due.push(*space);
            }
        }
        due
    }

    /// When the next wait ends.
    pub(super) fn next(&self) -> Option<Instant> {
        self.0.values().filter_map(|(at, _)| *at).min()
    }

    /// S3 refused the upload of `space`: it waits twice as long as before.
    fn refused(&mut self, space: SpaceId) {
        let wait = self
            .0
            .get(&space)
            .map_or(FIRST, |(_, wait)| (*wait * 2).min(LONGEST));
        self.0.insert(space, (Some(Instant::now() + wait), wait));
    }
}

/// What became of an upload.
pub(super) enum Uploaded {
    /// The frame is uploaded under this id; the push names it.
    Id(Vec<u8>),
    /// The relay refused the frame as too large; it is recorded as refused.
    Refused,
    /// The relay answered with another error, handled as an answer to the
    /// push, which stops.
    Stopped,
    /// S3 refused the upload: the push stops, and the space pushes again
    /// once its wait ends.
    Failed,
}

impl<T: Transport> Session<'_, T> {
    /// Uploads `frame`, too large for one push, to an upload URL the relay
    /// hands out (RFC 0007, "Large changes"). A frame the relay refuses as
    /// `too_large` is recorded as refused, as a push would; any other error
    /// is handled as an answer to the push, which then stops. An upload S3
    /// refuses waits and goes up again on this connection; any other
    /// failure ends the connection, and the change is pushed again on the
    /// next one.
    pub(super) async fn upload(
        &mut self,
        space: SpaceId,
        change: ChangeId,
        frame: &[u8],
    ) -> Result<Uploaded, End> {
        let req = self.req();
        self.send(wire::upload(req, space, frame.len())).await?;
        match self.ask(req).await? {
            Answer::Upload { id, url, .. } => match large::upload(&url, frame).await {
                Ok(()) => {
                    self.uploads.0.remove(&space);
                    with(self.db, |inner| {
                        if let Some(sync) = inner.sync.as_mut()
                            && sync.error.as_deref() == Some(UPLOAD_FAILED)
                        {
                            sync.error = None;
                        }
                    })?;
                    Ok(Uploaded::Id(id))
                }
                Err(Failed::Status(_)) => {
                    self.uploads.refused(space);
                    with(self.db, |inner| {
                        if let Some(sync) = inner.sync.as_mut() {
                            sync.error = Some(UPLOAD_FAILED.to_owned());
                        }
                    })?;
                    Ok(Uploaded::Failed)
                }
                Err(_) => Err(lost()),
            },
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
