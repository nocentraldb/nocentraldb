//! A store that a test can swap for another under a running relay, the
//! way a restore of the relay's tables replaces the log under a warm
//! process (RFC 0007, "Restores and the relay epoch").

use std::sync::atomic::{AtomicBool, Ordering};

use ncdb_core::ids::{ChangeId, MemberId, SpaceId};
use ncdb_relay::store::{
    AppendOutcome, Effect, Entry, Epoch, MemberRecord, NewEntry, PersonKeyId, PersonKeyPut,
    PersonKeyRecord, Position, PutOutcome, RelayStore, StoreError, Upload, UploadId,
};

/// Passes every call to the first store until [`Swap::swap`], and to the
/// second from then on.
pub struct Swap<S> {
    first: S,
    second: S,
    swapped: AtomicBool,
}

impl<S> Swap<S> {
    pub fn new(first: S, second: S) -> Swap<S> {
        Swap {
            first,
            second,
            swapped: AtomicBool::new(false),
        }
    }

    /// From now on, every call goes to the second store.
    pub fn swap(&self) {
        self.swapped.store(true, Ordering::SeqCst);
    }

    fn current(&self) -> &S {
        match self.swapped.load(Ordering::SeqCst) {
            false => &self.first,
            true => &self.second,
        }
    }
}

impl<S: RelayStore> RelayStore for Swap<S> {
    async fn append(
        &self,
        space: SpaceId,
        expected_head: Position,
        entries: Vec<NewEntry>,
        effects: Vec<Effect>,
    ) -> Result<AppendOutcome, StoreError> {
        self.current()
            .append(space, expected_head, entries, effects)
            .await
    }

    async fn read(
        &self,
        space: SpaceId,
        after: Position,
        limit: u32,
    ) -> Result<Vec<Entry>, StoreError> {
        self.current().read(space, after, limit).await
    }

    async fn read_controls(
        &self,
        space: SpaceId,
        after: Position,
        limit: u32,
    ) -> Result<Vec<Entry>, StoreError> {
        self.current().read_controls(space, after, limit).await
    }

    async fn read_bounded(
        &self,
        space: SpaceId,
        after: Position,
        limit: u32,
        max_bytes: u64,
    ) -> Result<Vec<Entry>, StoreError> {
        self.current()
            .read_bounded(space, after, limit, max_bytes)
            .await
    }

    async fn head(&self, space: SpaceId) -> Result<Option<Position>, StoreError> {
        self.current().head(space).await
    }

    async fn find(&self, space: SpaceId, change: ChangeId) -> Result<Option<Position>, StoreError> {
        self.current().find(space, change).await
    }

    async fn member(
        &self,
        space: SpaceId,
        member: MemberId,
    ) -> Result<Option<MemberRecord>, StoreError> {
        self.current().member(space, member).await
    }

    async fn members_of(&self, member: MemberId) -> Result<Vec<SpaceId>, StoreError> {
        self.current().members_of(member).await
    }

    async fn person_keys(&self, space: SpaceId) -> Result<Vec<PersonKeyRecord>, StoreError> {
        self.current().person_keys(space).await
    }

    async fn put_person_keys(
        &self,
        space: SpaceId,
        puts: Vec<PersonKeyPut>,
    ) -> Result<PutOutcome, StoreError> {
        self.current().put_person_keys(space, puts).await
    }

    async fn delete_person_keys(
        &self,
        space: SpaceId,
        ids: Vec<PersonKeyId>,
    ) -> Result<(), StoreError> {
        self.current().delete_person_keys(space, ids).await
    }

    async fn epoch(&self) -> Result<Epoch, StoreError> {
        self.current().epoch().await
    }

    async fn new_epoch(&self) -> Result<Epoch, StoreError> {
        self.current().new_epoch().await
    }

    async fn begin_upload(&self, upload: Upload, now: u64) -> Result<(), StoreError> {
        self.current().begin_upload(upload, now).await
    }

    async fn take_upload(
        &self,
        space: SpaceId,
        member: MemberId,
        id: UploadId,
    ) -> Result<Option<Vec<u8>>, StoreError> {
        self.current().take_upload(space, member, id).await
    }
}
