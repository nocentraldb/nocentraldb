//! Large changes (RFC 0007, "Large changes"): a frame too large for one
//! message goes up through a one-time upload URL and comes down through a
//! short-lived download URL. The core hands out and checks the URLs'
//! tokens; the server serves the URLs.

use ncdb_core::ids::{MemberId, SpaceId};

use crate::error::ErrorCode;
use crate::relay::Relay;
use crate::store::{Position, RelayStore, StoreError, Token, TokenUrls, Upload, UploadId};

/// How long an upload URL takes its frame: 15 minutes (RFC 0007).
pub const UPLOAD_URL_VALID_FOR: u64 = 15 * 60 * 1000;

/// How long a download URL serves its frame: 5 minutes.
pub const DOWNLOAD_URL_VALID_FOR: u64 = 5 * 60 * 1000;

/// What a filled upload URL answered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Filled {
    /// The frame is stored; a push may now name the upload.
    Stored,
    /// No upload takes a frame under this token: unknown, expired, or
    /// already filled.
    NotFound,
    /// The frame is not of the declared size. Nothing was stored.
    WrongSize,
}

/// A frame of a push: its bytes, or the upload that holds them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PushFrame {
    /// The frame, inline in the push.
    Bytes(Vec<u8>),
    /// An upload the uploader filled.
    Upload(UploadId),
}

fn random<const N: usize>() -> Result<[u8; N], ErrorCode> {
    let mut bytes = [0; N];
    getrandom::fill(&mut bytes).map_err(|_| ErrorCode::Unavailable)?;
    Ok(bytes)
}

impl<S: RelayStore> Relay<S> {
    /// Begins an upload of a frame of `size` bytes that `member` will push
    /// to `space`: its id and the token of its URL.
    pub async fn begin_upload(
        &self,
        member: MemberId,
        space: SpaceId,
        size: u64,
    ) -> Result<(UploadId, Token), ErrorCode> {
        if size > self.config().max_frame as u64 {
            return Err(ErrorCode::TooLarge);
        }
        if self.store().head(space).await?.is_none() {
            return Err(ErrorCode::UnknownSpace);
        }
        self.check_uploader(member, space).await?;
        let now = (self.config().clock)();
        let upload = Upload {
            id: UploadId(random()?),
            token: Token(random()?),
            space,
            member,
            size,
            expires_at: now.saturating_add(UPLOAD_URL_VALID_FOR),
        };
        let (id, token) = (upload.id, upload.token);
        self.store().begin_upload(upload, now).await?;
        Ok((id, token))
    }

    /// The frame of a filled upload of `uploader` in `space`, taken once.
    /// An upload not filled, unknown, or of another member or space is
    /// malformed: the push names no frame.
    pub(crate) async fn take_upload(
        &self,
        uploader: MemberId,
        space: SpaceId,
        id: UploadId,
    ) -> Result<Vec<u8>, ErrorCode> {
        self.store()
            .take_upload(space, uploader, id)
            .await?
            .ok_or(ErrorCode::Malformed)
    }
}

/// The URLs of large changes that a backend with token URLs serves itself
/// (NOC-303): the relay server and the local harness.
impl<S: TokenUrls> Relay<S> {
    /// The declared size of the frame an upload URL takes, if it takes one:
    /// its upload is known, unfilled and its URL has not expired. A server
    /// checks it before it reads the body.
    pub async fn upload_size(&self, token: Token) -> Result<Option<u64>, StoreError> {
        let now = (self.config().clock)();
        Ok(match self.store().upload(token).await? {
            Some((upload, false)) if now < upload.expires_at => Some(upload.size),
            _ => None,
        })
    }

    /// Fills an upload with its frame, once, while its URL is valid.
    pub async fn fill_upload(&self, token: Token, frame: Vec<u8>) -> Result<Filled, StoreError> {
        let Some(size) = self.upload_size(token).await? else {
            return Ok(Filled::NotFound);
        };
        if frame.len() as u64 != size {
            return Ok(Filled::WrongSize);
        }
        Ok(if self.store().fill_upload(token, frame).await? {
            Filled::Stored
        } else {
            Filled::NotFound
        })
    }

    /// A download token for the entry at `position` of `space`, which a
    /// reader has just pulled.
    pub async fn begin_download(
        &self,
        space: SpaceId,
        position: Position,
    ) -> Result<Token, ErrorCode> {
        let now = (self.config().clock)();
        let token = Token(random()?);
        let expires_at = now.saturating_add(DOWNLOAD_URL_VALID_FOR);
        self.store()
            .begin_download(token, space, position, expires_at, now)
            .await?;
        Ok(token)
    }

    /// The frame a download URL serves, while its token is valid.
    pub async fn download(&self, token: Token) -> Result<Option<Vec<u8>>, StoreError> {
        let now = (self.config().clock)();
        let Some((space, position, expires_at)) = self.store().download(token).await? else {
            return Ok(None);
        };
        if now >= expires_at {
            return Ok(None);
        }
        let entries = self.store().read(space, position - 1, 1).await?;
        Ok(entries
            .into_iter()
            .find(|entry| entry.position == position)
            .map(|entry| entry.frame))
    }
}
