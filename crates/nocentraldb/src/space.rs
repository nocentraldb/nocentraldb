//! Spaces and their handles (RFC 0009, "Spaces"): creating a space writes
//! its genesis and the creator's own key envelope on the device (RFC 0008,
//! "The first change of a space"); a handle writes into its space and
//! reads from it.

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use ncdb_core::change::{Change, Payload};
use ncdb_core::checks::SealChecks;
use ncdb_core::clock::Clock;
use ncdb_core::envelope;
use ncdb_core::ids::ObjectId as CoreObjectId;
use ncdb_core::keys::SpaceKey;
use ncdb_core::log::State;
use ncdb_core::membership::{self, Certificate, ControlBody, Entry, Genesis, MemberKind};
use ncdb_core::seal;

use crate::db::{Db, Inner, SpaceState, lock};
use crate::error::{Error, ErrorCode, storage};
use crate::id::{ChangeId, ObjectId, SpaceId};
use crate::object::Object;
use crate::subscribe::touched;
use crate::tx::Transaction;

/// The spaces of the member, from [`Db::spaces`].
#[derive(Debug)]
pub struct Spaces<'a> {
    pub(crate) db: &'a Db,
}

/// The calls of one space, from [`Spaces::create`].
#[derive(Clone)]
pub struct Space {
    inner: Arc<Mutex<Inner>>,
    id: SpaceId,
}

impl std::fmt::Debug for Space {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Space").field("id", &self.id).finish()
    }
}

/// What [`Space::write`] returns: the change the write became, and the ids
/// of the objects it created, in order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Written {
    /// The change's id.
    pub change: ChangeId,
    /// The ids of the objects the write created, in the order of its
    /// creates.
    pub ids: Vec<ObjectId>,
}

impl Spaces<'_> {
    /// Creates a space: writes its genesis and the member's own key
    /// envelope into the log and folds them, on the device. The member is
    /// the space's first admin. The space id derives from the member and a
    /// random nonce.
    pub async fn create(&self) -> Result<Space, Error> {
        let mut inner = self.db.lock();
        let member = inner.member.key();
        let nonce = CoreObjectId::random().0;
        let space = membership::space_id(member.id(), &nonce);
        let generation = membership::generation_id(space, member.id(), &[]);
        let key = SpaceKey::generate();
        let genesis = Genesis {
            nonce,
            creator: member.public(),
            certificate: Certificate::sign(&inner.account, member.id(), MemberKind::Device),
            first_generation: generation,
            model_id: membership::seal_model_id(&key, space, member.id(), inner.model_id),
        };
        let genesis = membership::write(
            &member,
            space,
            1,
            generation,
            &ControlBody::new(BTreeSet::new(), Entry::Genesis(genesis)),
        );
        let sealed = envelope::seal(&key, space, generation, &member.public())
            .expect("the member's own key takes an envelope");
        let own = membership::write(
            &member,
            space,
            2,
            generation,
            &ControlBody::new(BTreeSet::from([genesis.id()]), Entry::KeyEnvelope(sealed)),
        );
        let mut state = SpaceState {
            checks: SealChecks::new(space, member),
            key,
            generation,
            sequence: 2,
            clock: Clock::new(),
            heads: own.id(),
        };
        for change in [&genesis, &own] {
            fold(&mut inner, &mut state, change)?;
        }
        inner.spaces.insert(space, state);
        crate::sync::wake(&inner);
        Ok(Space {
            inner: self.db.inner.clone(),
            id: SpaceId(space),
        })
    }
}

/// Appends the member's own change to the log and folds it. Every check
/// passes on a change the member wrote itself, so a change that does not
/// fold is a bug of the SDK.
fn fold(inner: &mut Inner, state: &mut SpaceState, change: &Change) -> Result<(), Error> {
    let space = change.header().space;
    inner
        .view
        .receive(change.bytes(), &mut state.checks)
        .map_err(storage)?;
    match inner.view.state(space, change.id()) {
        Some(State::Folded { parked }) if parked.is_empty() => Ok(()),
        other => panic!("the member's own change was not folded: {other:?}"),
    }
}

impl Space {
    /// A handle on a space the database holds, as the sync tests read a
    /// space the member was added to.
    #[cfg(test)]
    pub(crate) fn handle(inner: Arc<Mutex<Inner>>, id: SpaceId) -> Space {
        Space { inner, id }
    }

    /// The space id.
    pub fn id(&self) -> SpaceId {
        self.id
    }

    pub(crate) fn inner(&self) -> &Arc<Mutex<Inner>> {
        &self.inner
    }

    /// Records a transaction with `record` and writes it as one change of
    /// this space: signed, sealed under the space key, appended to the log
    /// and folded into the view in one SQLite transaction. It returns when
    /// the change is folded, so a read right after it sees the write.
    ///
    /// `record` records; it does not read, and must not call the database.
    /// When it returns an error, or a recorded write is refused, nothing is
    /// written.
    pub async fn write<F>(&self, record: F) -> Result<Written, Error>
    where
        F: FnOnce(&mut Transaction<'_>) -> Result<(), Error>,
    {
        let model = lock(&self.inner).model.clone();
        let mut tx = Transaction::new(&model);
        record(&mut tx)?;
        let mut inner = lock(&self.inner);
        let space = self.id.0;
        let member = inner.member.key();
        let (operations, ids) = tx.operations(&inner.view, space, member.id())?;
        let collections: BTreeSet<String> =
            operations.iter().map(|op| op.collection.clone()).collect();
        let mut state = inner
            .spaces
            .remove(&space)
            .expect("a space handle's space is held");
        let wall = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX));
        let result = (|| {
            let clock = state.clock.now(wall).map_err(|error| {
                Error::new(
                    ErrorCode::StorageFailed,
                    format!("the space's clock: {error}"),
                )
            })?;
            let payload = Payload::new(clock, BTreeSet::from([state.heads]), operations);
            let sequence = state.sequence + 1;
            let change = seal::seal(
                &member,
                &state.key,
                space,
                sequence,
                state.generation,
                &payload,
            );
            fold(&mut inner, &mut state, &change)?;
            state.sequence = sequence;
            state.heads = change.id();
            touched(&mut inner.watchers, space, &collections);
            Ok(Written {
                change: ChangeId(change.id()),
                ids,
            })
        })();
        inner.spaces.insert(space, state);
        if result.is_ok() {
            crate::sync::wake(&inner);
        }
        result
    }

    /// Reads the object `id` of `collection` in this space, or `None` when
    /// the copy holds no such object in that collection. A deleted object
    /// is read too.
    pub async fn get(&self, collection: &str, id: ObjectId) -> Result<Option<Object>, Error> {
        let inner = lock(&self.inner);
        if !inner.model.collections.contains_key(collection) {
            return Err(Error::new(
                ErrorCode::NotFound,
                format!("the model has no collection {collection}"),
            ));
        }
        Object::read(&inner, self.id.0, collection, id.0)
    }
}
