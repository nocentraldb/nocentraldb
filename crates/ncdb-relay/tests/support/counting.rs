//! A store that counts how often the core reads a space's log, so a test
//! can tell whether an append made the relay read and resolve the log.

use std::sync::atomic::{AtomicUsize, Ordering};

use ncdb_core::ids::{ChangeId, MemberId, SpaceId};
use ncdb_relay::store::{
    AppendOutcome, Effect, Entry, Epoch, MemberRecord, NewEntry, PersonKeyId, PersonKeyPut,
    PersonKeyRecord, Position, PutOutcome, RelayStore, StoreError,
};

/// Passes every call to `inner`, and counts the calls to `read`.
pub struct Counting<S> {
    inner: S,
    reads: AtomicUsize,
}

impl<S> Counting<S> {
    pub fn new(inner: S) -> Counting<S> {
        Counting {
            inner,
            reads: AtomicUsize::new(0),
        }
    }

    /// The calls to `read` since the last call to this.
    pub fn take_reads(&self) -> usize {
        self.reads.swap(0, Ordering::SeqCst)
    }
}

impl<S: RelayStore> RelayStore for Counting<S> {
    async fn append(
        &self,
        space: SpaceId,
        expected_head: Position,
        entries: Vec<NewEntry>,
        effects: Vec<Effect>,
    ) -> Result<AppendOutcome, StoreError> {
        self.inner
            .append(space, expected_head, entries, effects)
            .await
    }

    async fn read(
        &self,
        space: SpaceId,
        after: Position,
        limit: u32,
    ) -> Result<Vec<Entry>, StoreError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        self.inner.read(space, after, limit).await
    }

    async fn head(&self, space: SpaceId) -> Result<Option<Position>, StoreError> {
        self.inner.head(space).await
    }

    async fn find(&self, space: SpaceId, change: ChangeId) -> Result<Option<Position>, StoreError> {
        self.inner.find(space, change).await
    }

    async fn member(
        &self,
        space: SpaceId,
        member: MemberId,
    ) -> Result<Option<MemberRecord>, StoreError> {
        self.inner.member(space, member).await
    }

    async fn members_of(&self, member: MemberId) -> Result<Vec<SpaceId>, StoreError> {
        self.inner.members_of(member).await
    }

    async fn person_keys(&self, space: SpaceId) -> Result<Vec<PersonKeyRecord>, StoreError> {
        self.inner.person_keys(space).await
    }

    async fn put_person_keys(
        &self,
        space: SpaceId,
        puts: Vec<PersonKeyPut>,
    ) -> Result<PutOutcome, StoreError> {
        self.inner.put_person_keys(space, puts).await
    }

    async fn delete_person_keys(
        &self,
        space: SpaceId,
        ids: Vec<PersonKeyId>,
    ) -> Result<(), StoreError> {
        self.inner.delete_person_keys(space, ids).await
    }

    async fn epoch(&self) -> Result<Epoch, StoreError> {
        self.inner.epoch().await
    }

    async fn new_epoch(&self) -> Result<Epoch, StoreError> {
        self.inner.new_epoch().await
    }
}
