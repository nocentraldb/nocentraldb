//! A store that lets a test run another relay's request between the core's
//! checks and its write, the way a second relay process on the same
//! storage can.

use std::sync::Mutex;

use ncdb_core::ids::{ChangeId, MemberId, SpaceId};
use ncdb_relay::store::{
    AppendOutcome, Effect, Entry, Epoch, MemberRecord, NewEntry, PersonKeyId, PersonKeyPut,
    PersonKeyRecord, Position, PutOutcome, RelayStore, StoreError,
};

/// What runs once, right before the first person-key write reaches the
/// store.
type Rival = Box<dyn FnOnce() + Send>;

/// Passes every call to `inner`, and runs the rival once before the first
/// `put_person_keys`.
pub struct Interleave<S> {
    inner: S,
    rival: Mutex<Option<Rival>>,
}

impl<S> Interleave<S> {
    pub fn new(inner: S, rival: impl FnOnce() + Send + 'static) -> Interleave<S> {
        Interleave {
            inner,
            rival: Mutex::new(Some(Box::new(rival))),
        }
    }

    fn run_rival(&self) {
        let rival = self.rival.lock().expect("the rival's lock").take();
        if let Some(rival) = rival {
            rival();
        }
    }
}

impl<S: RelayStore> RelayStore for Interleave<S> {
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
        self.run_rival();
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
