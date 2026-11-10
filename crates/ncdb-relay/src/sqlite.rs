//! The SQLite backend of RFC 0007 ("The backend interface").

use std::path::Path;

use ncdb_core::ids::{ChangeId, MemberId, SpaceId};

use crate::store::{
    AppendOutcome, Effect, Entry, Epoch, MemberRecord, NewEntry, PersonKeyId, PersonKeyRecord,
    Position, RelayStore, StoreError,
};

/// A handle on a relay's SQLite database.
pub struct SqliteStore;

impl SqliteStore {
    /// Opens the database at `path`, creating it if needed.
    pub fn open(_path: &Path) -> Result<SqliteStore, StoreError> {
        Ok(SqliteStore)
    }
}

fn todo<T>() -> Result<T, StoreError> {
    Err(StoreError("not built yet".into()))
}

impl RelayStore for SqliteStore {
    async fn append(
        &self,
        _: SpaceId,
        _: Position,
        _: Vec<NewEntry>,
        _: Vec<Effect>,
    ) -> Result<AppendOutcome, StoreError> {
        todo()
    }
    async fn read(&self, _: SpaceId, _: Position, _: u32) -> Result<Vec<Entry>, StoreError> {
        todo()
    }
    async fn head(&self, _: SpaceId) -> Result<Option<Position>, StoreError> {
        todo()
    }
    async fn find(&self, _: SpaceId, _: ChangeId) -> Result<Option<Position>, StoreError> {
        todo()
    }
    async fn member(&self, _: SpaceId, _: MemberId) -> Result<Option<MemberRecord>, StoreError> {
        todo()
    }
    async fn members_of(&self, _: MemberId) -> Result<Vec<SpaceId>, StoreError> {
        todo()
    }
    async fn person_keys(&self, _: SpaceId) -> Result<Vec<PersonKeyRecord>, StoreError> {
        todo()
    }
    async fn put_person_keys(&self, _: SpaceId, _: Vec<PersonKeyRecord>) -> Result<(), StoreError> {
        todo()
    }
    async fn delete_person_keys(&self, _: SpaceId, _: Vec<PersonKeyId>) -> Result<(), StoreError> {
        todo()
    }
    async fn epoch(&self) -> Result<Epoch, StoreError> {
        todo()
    }
    async fn new_epoch(&self) -> Result<Epoch, StoreError> {
        todo()
    }
}
