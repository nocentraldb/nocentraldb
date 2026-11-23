//! The SQLite backend's tokens for large changes it serves itself (RFC
//! 0007, "Large changes"; NOC-303): an upload waits in `uploads` under its
//! token, and a download token names a log row in `downloads`.

use ncdb_core::ids::{MemberId, SpaceId};
use rusqlite::{OptionalExtension, params};

use super::{SqliteStore, array, failed, immediate, int, sweep, unsigned};
use crate::store::{Position, StoreError, Token, TokenUrls, Upload, UploadId};

impl TokenUrls for SqliteStore {
    async fn upload(&self, token: Token) -> Result<Option<(Upload, bool)>, StoreError> {
        let row = self
            .connection()
            .query_row(
                "SELECT id, space, member, size, expires_at, frame IS NOT NULL FROM uploads
                 WHERE token = ?1",
                [token.0],
                |row| {
                    Ok((
                        row.get::<_, Vec<u8>>(0)?,
                        row.get::<_, Vec<u8>>(1)?,
                        row.get::<_, Vec<u8>>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, bool>(5)?,
                    ))
                },
            )
            .optional()
            .map_err(failed("upload"))?;
        let Some((id, space, member, size, expires_at, filled)) = row else {
            return Ok(None);
        };
        let upload = Upload {
            id: UploadId(array(id)?),
            token,
            space: SpaceId(array(space)?),
            member: MemberId(array(member)?),
            size: unsigned(size),
            expires_at: unsigned(expires_at),
        };
        Ok(Some((upload, filled)))
    }

    async fn fill_upload(&self, token: Token, frame: Vec<u8>) -> Result<bool, StoreError> {
        let filled = self
            .connection()
            .execute(
                "UPDATE uploads SET frame = ?2 WHERE token = ?1 AND frame IS NULL",
                params![token.0, frame],
            )
            .map_err(failed("fill upload"))?;
        Ok(filled == 1)
    }

    async fn begin_download(
        &self,
        token: Token,
        space: SpaceId,
        position: Position,
        expires_at: u64,
        now: u64,
    ) -> Result<(), StoreError> {
        let mut connection = self.connection();
        let tx = immediate(&mut connection)?;
        sweep(&tx, now)?;
        tx.execute(
            "INSERT INTO downloads (token, space, position, expires_at) VALUES (?1, ?2, ?3, ?4)",
            params![token.0, space.0, int(position), int(expires_at)],
        )
        .map_err(failed("begin download"))?;
        tx.commit().map_err(failed("begin download"))
    }

    async fn download(&self, token: Token) -> Result<Option<(SpaceId, Position, u64)>, StoreError> {
        let row = self
            .connection()
            .query_row(
                "SELECT space, position, expires_at FROM downloads WHERE token = ?1",
                [token.0],
                |row| {
                    Ok((
                        row.get::<_, Vec<u8>>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(failed("download"))?;
        row.map(|(space, position, expires_at)| {
            Ok((
                SpaceId(array(space)?),
                unsigned(position),
                unsigned(expires_at),
            ))
        })
        .transpose()
    }
}
