//! A store that counts how often the core reads a space's log, so a test
//! can tell whether an append made the relay read and resolve the log, and
//! how many frame bytes its reads loaded.

use std::sync::atomic::{AtomicUsize, Ordering};

use ncdb_core::ids::{ChangeId, MemberId, SpaceId};
use ncdb_relay::store::{
    AppendOutcome, Effect, Entry, Epoch, MemberRecord, NewEntry, PersonKeyId, PersonKeyPut,
    PersonKeyRecord, Position, PutOutcome, RelayStore, StoreError, Token, TokenUrls, Upload,
    UploadId,
};

/// Passes every call to `inner`, and counts the calls to `read`.
pub struct Counting<S> {
    inner: S,
    reads: AtomicUsize,
    bytes: AtomicUsize,
    largest: AtomicUsize,
}

impl<S> Counting<S> {
    pub fn new(inner: S) -> Counting<S> {
        Counting {
            inner,
            reads: AtomicUsize::new(0),
            bytes: AtomicUsize::new(0),
            largest: AtomicUsize::new(0),
        }
    }

    /// The calls to `read` since the last call to this.
    pub fn take_reads(&self) -> usize {
        self.reads.swap(0, Ordering::SeqCst)
    }

    /// The frame bytes all reads loaded since the last call to this, and the
    /// most one read loaded.
    pub fn take_bytes(&self) -> (usize, usize) {
        (
            self.bytes.swap(0, Ordering::SeqCst),
            self.largest.swap(0, Ordering::SeqCst),
        )
    }

    /// Counts a read that loaded `entries`.
    fn loaded(&self, entries: &[Entry]) {
        self.reads.fetch_add(1, Ordering::SeqCst);
        let bytes = entries.iter().map(|e| e.frame.len()).sum();
        self.bytes.fetch_add(bytes, Ordering::SeqCst);
        self.largest.fetch_max(bytes, Ordering::SeqCst);
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
        let entries = self.inner.read(space, after, limit).await?;
        self.loaded(&entries);
        Ok(entries)
    }

    async fn read_controls(
        &self,
        space: SpaceId,
        after: Position,
        limit: u32,
    ) -> Result<Vec<Entry>, StoreError> {
        let entries = self.inner.read_controls(space, after, limit).await?;
        self.loaded(&entries);
        Ok(entries)
    }

    async fn read_bounded(
        &self,
        space: SpaceId,
        after: Position,
        limit: u32,
        max_bytes: u64,
    ) -> Result<Vec<Entry>, StoreError> {
        let entries = self
            .inner
            .read_bounded(space, after, limit, max_bytes)
            .await?;
        self.loaded(&entries);
        Ok(entries)
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

    async fn begin_upload(&self, upload: Upload, now: u64) -> Result<(), StoreError> {
        self.inner.begin_upload(upload, now).await
    }

    async fn take_upload(
        &self,
        space: SpaceId,
        member: MemberId,
        id: UploadId,
    ) -> Result<Option<Vec<u8>>, StoreError> {
        self.inner.take_upload(space, member, id).await
    }
}

impl<S: TokenUrls> TokenUrls for Counting<S> {
    async fn upload(&self, token: Token) -> Result<Option<(Upload, bool)>, StoreError> {
        self.inner.upload(token).await
    }

    async fn fill_upload(&self, token: Token, frame: Vec<u8>) -> Result<bool, StoreError> {
        self.inner.fill_upload(token, frame).await
    }

    async fn begin_download(
        &self,
        token: Token,
        space: SpaceId,
        position: Position,
        expires_at: u64,
        now: u64,
    ) -> Result<(), StoreError> {
        self.inner
            .begin_download(token, space, position, expires_at, now)
            .await
    }

    async fn download(&self, token: Token) -> Result<Option<(SpaceId, Position, u64)>, StoreError> {
        self.inner.download(token).await
    }
}
