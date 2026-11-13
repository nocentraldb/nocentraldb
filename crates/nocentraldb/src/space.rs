//! Spaces and their handles (RFC 0009, "Spaces"): creating a space writes
//! its genesis and the creator's own key envelope on the device (RFC 0008,
//! "The first change of a space"); a handle writes into its space and
//! reads from it.

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use ncdb_core::change::{Change, Payload, WriterModel};
use ncdb_core::checks::SealChecks;
use ncdb_core::envelope;
use ncdb_core::ids::{ObjectId as CoreObjectId, SpaceId as CoreSpaceId};
use ncdb_core::keys::SpaceKey;
use ncdb_core::log::State;
use ncdb_core::membership::{self, ControlBody, Entry, Genesis, Role};
use ncdb_core::seal;
use ncdb_core::view::View;

use crate::db::{Db, Inner, SpaceState, lock};
use crate::error::{Error, ErrorCode, storage};
use crate::id::{ChangeId, ObjectId, SpaceId};
use crate::list::SpaceModel;
use crate::members::{Members, holds};
use crate::next;
use crate::object::Object;
use crate::subscribe::touched;
use crate::tx::Transaction;
use crate::value::Value;

/// The spaces of the member, from [`Db::spaces`].
#[derive(Debug)]
pub struct Spaces<'a> {
    pub(crate) db: &'a Db,
}

/// The calls of one space, from [`Spaces::create`] or [`Spaces::get`].
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
        // The account's first space comes with the account space, created
        // before it: on a relay that lets an account it does not list
        // create one space, that space must be the account space (RFC
        // 0007, "Who may create a space"; NOC-287).
        crate::account::create(&mut inner)?;
        let model = inner.model_id;
        let space = create_space(&mut inner, model)?;
        // The account space records the space the device joined (RFC 0010).
        crate::account::reconcile(&mut inner)?;
        Ok(Space {
            inner: self.db.inner.clone(),
            id: SpaceId(space),
        })
    }

    /// The handle of the space `id`, which the member created or was added
    /// to. A space is held once the copy holds its genesis and the space
    /// key of its first and of its current generation; until then it is
    /// `NotFound`. A member added after a key rotation holds an envelope
    /// for the current generation only, and reaches the first through the
    /// key chain (RFC 0003, "Key-chain subkey"). A space whose genesis
    /// names another `model.id` than the
    /// lock's is `SpaceOfAnotherModel` (RFC 0009, "Spaces of another
    /// model").
    pub async fn get(&self, id: SpaceId) -> Result<Space, Error> {
        let inner = self.db.lock();
        writable(&inner, id)?;
        Ok(Space {
            inner: self.db.inner.clone(),
            id,
        })
    }
}

/// Creates a space of the model `model`: writes its genesis and the
/// member's own key envelope into the log and folds them, on the device.
/// The member is the space's first admin. The space id derives from the
/// member and a random nonce. The app's spaces are of its lock's model,
/// the account space of the model the SDK ships (`crate::account`).
pub(crate) fn create_space(inner: &mut Inner, model: [u8; 16]) -> Result<CoreSpaceId, Error> {
    let member = inner.member.key();
    let certificate = inner
        .account
        .certificate(member.id(), inner.kind)
        .ok_or_else(|| {
            Error::new(
                ErrorCode::NotAllowed,
                "a database waiting to be paired creates no space until its pairing completes",
            )
        })?;
    let nonce = CoreObjectId::random().0;
    let space = membership::space_id(member.id(), &nonce);
    let generation = membership::generation_id(space, member.id(), &[]);
    let key = SpaceKey::generate();
    let genesis = Genesis {
        nonce,
        creator: member.public(),
        certificate,
        first_generation: generation,
        model_id: membership::seal_model_id(&key, space, member.id(), model),
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
    let mut state = SpaceState::new(space, member);
    for change in [&genesis, &own] {
        fold(&mut inner.view, &mut state.checks, change)?;
    }
    inner.spaces.insert(space, state);
    crate::sync::wake(inner);
    Ok(space)
}

/// Whether the member can write into the space `id`: the copy holds it, it
/// is of the lock's model, and the member holds the key of its current
/// generation.
pub(crate) fn writable(inner: &Inner, id: SpaceId) -> Result<(), Error> {
    writable_by(inner, id.0, inner.model_id)
}

/// Whether the member can write into the space `id`, of the app's model or
/// the account space.
pub(crate) fn writable_or_account(inner: &Inner, id: SpaceId) -> Result<(), Error> {
    if crate::account::space(inner) == Some(id.0) {
        return writable_by(inner, id.0, crate::account::id());
    }
    writable(inner, id)
}

/// Whether the member can write into the space `id` as a space of the
/// model `model`: the app's lock's, or the account space's
/// (`crate::account`).
pub(crate) fn writable_by(inner: &Inner, space: CoreSpaceId, model: [u8; 16]) -> Result<(), Error> {
    let id = SpaceId(space);
    let not_found = || Error::new(ErrorCode::NotFound, format!("the copy holds no space {id}"));
    let state = inner.spaces.get(&id.0).ok_or_else(not_found)?;
    let model_id = state.checks.model_id().ok_or_else(not_found)?;
    if SpaceModel::of(model_id, model) == SpaceModel::Another {
        return Err(Error::new(
            ErrorCode::SpaceOfAnotherModel,
            format!(
                "the space {id} is of the model {}, the lock is of the model {}",
                hex(&model_id),
                hex(&model)
            ),
        ));
    }
    let generation = state.checks.resolved().current_generation();
    match generation.and_then(|generation| state.checks.space_key(generation)) {
        Some(_) => Ok(()),
        None => Err(not_found()),
    }
}

/// Whether the relay still serves this member the space `id`. Once it
/// answered `not_member` or `removed_member` (RFC 0007, "The errors"), the
/// member was removed, and a write or a membership call is `NotMember`
/// (RFC 0009, "Errors"), even though its copy has not folded the removal.
pub(crate) fn not_stopped(inner: &Inner, id: SpaceId) -> Result<(), Error> {
    match crate::sync::stopped(inner, id.0) {
        true => Err(Error::new(
            ErrorCode::NotMember,
            format!("the relay no longer serves this member the space {id}"),
        )),
        false => Ok(()),
    }
}

/// A model id as 32 lowercase hex digits, as `model.lock.json` writes it.
fn hex(bytes: &[u8; 16]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Appends the member's own change to the log, with the time it was
/// written (NOC-231), and folds it. Every check passes on a change the
/// member wrote itself, so a change that does not fold is a bug of the SDK.
pub(crate) fn fold(view: &mut View, checks: &mut SealChecks, change: &Change) -> Result<(), Error> {
    let space = change.header().space;
    view.receive_own(change.bytes(), checks, crate::clock::now_millis())
        .map_err(storage)?;
    crate::status::note_shreds(view, space, checks.resolved()).map_err(storage)?;
    match view.state(space, change.id()) {
        Some(State::Folded { parked }) if parked.is_empty() => Ok(()),
        other => panic!("the member's own change was not folded: {other:?}"),
    }
}

impl Space {
    /// A handle on a space the database holds, without the checks of
    /// [`Spaces::get`], as the sync tests read a space of another model.
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

    /// The members of this space.
    pub fn members(&self) -> Members<'_> {
        Members { space: self }
    }

    /// Records a transaction with `record` and writes it as one change of
    /// this space: signed, sealed under the space key, appended to the log
    /// and folded into the view in one SQLite transaction. It returns when
    /// the change is folded, so a read right after it sees the write.
    ///
    /// `record` records; it does not read, and must not call the database.
    /// When it returns an error, or a recorded write is refused, nothing is
    /// written.
    ///
    /// Only a writer or an admin writes content (RFC 0008, "Roles and
    /// rights"): a reader is refused with `NotAllowed`, a member whose copy
    /// folded its removal, or that the relay answered `not_member` or
    /// `removed_member` for the space, with `NotMember` (RFC 0009,
    /// "Errors"), before any change is made.
    pub async fn write<F>(&self, record: F) -> Result<Written, Error>
    where
        F: FnOnce(&mut Transaction<'_>) -> Result<(), Error>,
    {
        let model = lock(&self.inner).model.clone();
        let mut tx = Transaction::new(&model);
        record(&mut tx)?;
        let mut inner = lock(&self.inner);
        // Key 64: the lock this database was opened with wrote the change
        // (RFC 0002, "The writer's model").
        let writer = WriterModel {
            id: inner.model.id,
            version: inner.model.version,
        };
        let id = self.id;
        write_change(&mut inner, self.id.0, tx, writer, |inner| {
            writable(inner, id)?;
            not_stopped(inner, id)
        })
    }
}

/// Writes what `tx` recorded as one change of `space`, of the model
/// `writer` names: signed, sealed under the space key, appended and folded
/// in one SQLite transaction. `allowed` checks that the member may write
/// into the space, after the transaction's own checks, before anything is
/// made; the member must be a writer or an admin. Space::write writes the
/// app's spaces by the app's lock, the account space its records by the
/// model the SDK ships (`crate::account`).
pub(crate) fn write_change(
    inner: &mut Inner,
    space: CoreSpaceId,
    tx: Transaction<'_>,
    writer: WriterModel,
    allowed: impl FnOnce(&Inner) -> Result<(), Error>,
) -> Result<Written, Error> {
    let member = inner.member.key();
    let no_keys = std::collections::BTreeMap::new();
    let held = inner
        .spaces
        .get(&space)
        .map_or(&no_keys, |state| &state.person_keys);
    let mut persons = crate::personal::Persons::new(held);
    let (operations, ids) = tx.operations(&inner.view, space, member.id(), &mut persons)?;
    let drawn = persons.drawn;
    let collections: BTreeSet<String> = operations.iter().map(|op| op.collection.clone()).collect();
    allowed(inner)?;
    let resolved = inner.spaces[&space].checks.resolved();
    holds(resolved, member.id(), Role::Writer, "writes content")?;
    let wall = crate::clock::now_millis();
    let Inner {
        view,
        spaces,
        watchers,
        ..
    } = inner;
    let state = spaces.get_mut(&space).expect("a writable space is held");
    // Each person this write drew gets its key first: its person-key
    // entry, folded before the content that is sealed under it (RFC
    // 0003, "The person key").
    let mut records = Vec::new();
    for drawn in drawn {
        records.push(person_key(view, state, space, &member, drawn)?);
    }
    let next = next::next(view, space, &state.checks, member.id());
    let generation = next.generation.expect("a writable space has a generation");
    state.clock.observe(next.seen);
    let clock = state.clock.now(wall).map_err(|error| {
        Error::new(
            ErrorCode::StorageFailed,
            format!("the space's clock: {error}"),
        )
    })?;
    let mut payload = Payload::new(clock, next.heads, operations);
    payload.set_writer_model(writer);
    let key = state
        .checks
        .space_key(generation)
        .expect("a writable space's key is held");
    let change = seal::seal(&member, key, space, next.sequence, generation, &payload);
    fold(view, &mut state.checks, &change)?;
    crate::personal::refresh(view, space, state).map_err(storage)?;
    touched(watchers, space, &collections);
    if !records.is_empty() {
        crate::sync::put_person_keys(inner, space, records);
    }
    crate::sync::wake(inner);
    Ok(Written {
        change: ChangeId(change.id()),
        ids,
    })
}

impl Space {
    /// Creates an object in `collection` with `values`, as one change: a
    /// [`write`](Space::write) with one [`create`](Transaction::create)
    /// (RFC 0009, "One write, one change"). The new id is the answer's
    /// only id.
    pub async fn create<'v>(
        &self,
        collection: &str,
        values: impl IntoIterator<Item = (&'v str, Value)>,
    ) -> Result<Written, Error> {
        self.write(|tx| tx.create(collection, values).map(|_| ()))
            .await
    }

    /// Writes `values` to the object `id` of `collection`, as one change:
    /// a [`write`](Space::write) with one [`update`](Transaction::update).
    pub async fn update<'v>(
        &self,
        collection: &str,
        id: ObjectId,
        values: impl IntoIterator<Item = (&'v str, Value)>,
    ) -> Result<Written, Error> {
        self.write(|tx| tx.update(collection, id, values)).await
    }

    /// Deletes the object `id` of `collection`, as one change: a
    /// [`write`](Space::write) with one [`delete`](Transaction::delete).
    pub async fn delete(&self, collection: &str, id: ObjectId) -> Result<Written, Error> {
        self.write(|tx| tx.delete(collection, id)).await
    }

    /// Restores the deleted object `id` of `collection`, as one change: a
    /// [`write`](Space::write) with one [`restore`](Transaction::restore).
    pub async fn restore(&self, collection: &str, id: ObjectId) -> Result<Written, Error> {
        self.write(|tx| tx.restore(collection, id)).await
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

/// Writes the person-key entry of the person `drawn`, under the space's
/// current generation, folds it, and holds the key; returns the record to
/// put at the relay's person-key store (RFC 0003, "The person key"; RFC
/// 0007, "Keys and the person-key store").
fn person_key(
    view: &mut View,
    state: &mut SpaceState,
    space: ncdb_core::ids::SpaceId,
    member: &ncdb_core::keys::MemberKey,
    drawn: crate::personal::Drawn,
) -> Result<crate::sync::Record, Error> {
    let id = drawn.person.person_key;
    let next = next::next(view, space, &state.checks, member.id());
    let generation = next.generation.expect("a writable space has a generation");
    let space_key = state
        .checks
        .space_key(generation)
        .expect("a writable space's key is held");
    let wrapped = membership::wrap_person_key(space_key, space, generation, id, &drawn.key);
    let signature = membership::sign_record(member, space, id, generation, &wrapped);
    let entry = Entry::PersonKey(membership::PersonKeyEntry {
        person_key: id,
        commitment: drawn.key.commitment(space, id),
        generation,
        record_hash: membership::record_hash(&wrapped),
    });
    let change = membership::write(
        member,
        space,
        next.sequence,
        generation,
        &ControlBody::new(crate::members::control_heads(view, space), entry),
    );
    fold(view, &mut state.checks, &change)?;
    state.person_keys.insert(id, drawn.key);
    Ok(crate::sync::Record {
        person_key: id,
        generation,
        writer: member.id().0,
        wrapped: wrapped.to_vec(),
        signature,
    })
}
