//! Spaces and their handles (RFC 0009, "Spaces"): creating a space writes
//! its genesis and the creator's own key envelope on the device (RFC 0008,
//! "The first change of a space"); a handle writes into its space and
//! reads from it.

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use ncdb_core::change::{Change, Payload, WriterModel};
use ncdb_core::checks::SealChecks;
use ncdb_core::envelope;
use ncdb_core::ids::ObjectId as CoreObjectId;
use ncdb_core::keys::SpaceKey;
use ncdb_core::log::State;
use ncdb_core::membership::{self, Certificate, ControlBody, Entry, Genesis, Role};
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
        let member = inner.member.key();
        let nonce = CoreObjectId::random().0;
        let space = membership::space_id(member.id(), &nonce);
        let generation = membership::generation_id(space, member.id(), &[]);
        let key = SpaceKey::generate();
        let genesis = Genesis {
            nonce,
            creator: member.public(),
            certificate: Certificate::sign(&inner.account, member.id(), inner.kind),
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
        let mut state = SpaceState::new(space, member);
        for change in [&genesis, &own] {
            fold(&mut inner.view, &mut state.checks, change)?;
        }
        inner.spaces.insert(space, state);
        crate::sync::wake(&inner);
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

/// Whether the member can write into the space `id`: the copy holds it, it
/// is of the lock's model, and the member holds the key of its current
/// generation.
pub(crate) fn writable(inner: &Inner, id: SpaceId) -> Result<(), Error> {
    let not_found = || Error::new(ErrorCode::NotFound, format!("the copy holds no space {id}"));
    let state = inner.spaces.get(&id.0).ok_or_else(not_found)?;
    let model_id = state.checks.model_id().ok_or_else(not_found)?;
    if SpaceModel::of(model_id, inner.model_id) == SpaceModel::Another {
        return Err(Error::new(
            ErrorCode::SpaceOfAnotherModel,
            format!(
                "the space {id} is of the model {}, the lock is of the model {}",
                hex(&model_id),
                hex(&inner.model_id)
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
        let space = self.id.0;
        let member = inner.member.key();
        let no_keys = std::collections::BTreeMap::new();
        let held = inner
            .spaces
            .get(&space)
            .map_or(&no_keys, |state| &state.person_keys);
        let mut persons = crate::personal::Persons::new(held);
        let (operations, ids) = tx.operations(&inner.view, space, member.id(), &mut persons)?;
        let drawn = persons.drawn;
        let collections: BTreeSet<String> =
            operations.iter().map(|op| op.collection.clone()).collect();
        writable(&inner, self.id)?;
        not_stopped(&inner, self.id)?;
        let resolved = inner.spaces[&space].checks.resolved();
        holds(resolved, member.id(), Role::Writer, "writes content")?;
        // Key 64: the lock this database was opened with wrote the change
        // (RFC 0002, "The writer's model").
        let writer = WriterModel {
            id: inner.model.id,
            version: inner.model.version,
        };
        let wall = crate::clock::now_millis();
        let Inner {
            view,
            spaces,
            watchers,
            ..
        } = &mut *inner;
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
        if !records.is_empty() {
            view.set_person_keys(space, crate::personal::open_keys(state))
                .map_err(storage)?;
        }
        touched(watchers, space, &collections);
        if !records.is_empty() {
            crate::sync::put_person_keys(&mut inner, space, records);
        }
        crate::sync::wake(&inner);
        Ok(Written {
            change: ChangeId(change.id()),
            ids,
        })
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
