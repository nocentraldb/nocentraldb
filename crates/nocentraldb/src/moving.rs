//! Moving objects to another space (RFC 0016; NOC-475). A move is two
//! changes: the new space gets each object, with the same id and the values
//! the copy reads now, as a create, with `$movedFrom` naming the old space;
//! then the old space gets the move, operation 6, and keeps a trace. The
//! new space is written first, so that a stop between the two leaves a
//! duplicate, never a gap ("The new space first").
//!
//! Personal fields in a move come with NOC-477: until then an object that
//! holds a personal value is refused with `NotAllowed`, as RFC 0016 says.
//! A plugin's read is no value its write takes, so an object with a set
//! plugin field is refused with `NotAllowed` too, until a plugin can carry
//! its state (NOC-475).

use std::collections::BTreeSet;

use ncdb_core::change::WriterModel;
use ncdb_core::field::FieldType;
use ncdb_core::ids::{ObjectId as CoreObjectId, SpaceId as CoreSpaceId};
use ncdb_core::membership::Role;
use ncdb_core::view::{Field, FieldState};

use crate::db::{Inner, lock};
use crate::error::{Error, ErrorCode, storage};
use crate::id::ObjectId;
use crate::members::{Notice, holds};
use crate::object::Object;
use crate::space::{Space, not_stopped, writable, write_change};
use crate::tx::Transaction;
use crate::value::{Repr, Value};

/// What [`Space::move_objects`] returns (RFC 0016, "The SDK call and its
/// notice").
#[derive(Debug)]
pub struct Moved {
    /// [`NoticeCode::ObjectMoved`](crate::NoticeCode): what the move does
    /// not do.
    pub notice: Notice,
    /// The moved ids, in the order given.
    pub ids: Vec<ObjectId>,
    /// How many links, one or of many, were cleared in the new space
    /// because their target was not moved along.
    pub cleared_links: usize,
    /// The fields that held a conflict, which the move resolved to the
    /// winning value: each object id with the field's name.
    pub conflicts: Vec<(ObjectId, String)>,
}

impl Space {
    /// Moves `objects`, read from this space, to `destination`: the same
    /// ids, with the values this copy reads now (RFC 0016, "What a move
    /// is"). The destination's change is written first, then this space's
    /// move; this space then keeps a trace of each object, which reads as
    /// moved and shows no fields.
    ///
    /// A link to an object moved along stays; one to an object left
    /// behind is cleared and counted. A reference stays as it is.
    ///
    /// Refused, with nothing written: an object of another space, one
    /// already moved, one named twice, or a destination that is this space
    /// (`InvalidValue`); an object this copy does not hold (`NotFound`); a
    /// member that may not write in either space (`NotAllowed`,
    /// `NotMember`); a destination of another model
    /// (`SpaceOfAnotherModel`); and, until NOC-477, an object that holds
    /// a personal value, or a set plugin field (`NotAllowed`).
    pub async fn move_objects(
        &self,
        objects: &[Object],
        destination: &Space,
    ) -> Result<Moved, Error> {
        let (old, new) = (self.id(), destination.id());
        if old == new {
            return Err(Error::new(
                ErrorCode::InvalidValue,
                crate::texts::error::MOVE_INTO_ITS_OWN_SPACE,
            ));
        }
        let mut inner = lock(self.inner());
        for space in [old, new] {
            writable(&inner, space)?;
            not_stopped(&inner, space)?;
            let resolved = inner.spaces[&space.0].checks.resolved();
            holds(
                resolved,
                inner.member.key().id(),
                Role::Writer,
                crate::texts::part::DOING_WRITE,
            )?;
        }
        let sources = sources(&inner, old.0, objects)?;
        let moved: BTreeSet<CoreObjectId> = sources.iter().map(|o| o.id().0).collect();
        let model = inner.model.clone();
        let mut create = Transaction::new(&model);
        let mut cleared_links = 0;
        let mut conflicts = Vec::new();
        for object in &sources {
            let carried = carried(&inner, object, &moved)?;
            cleared_links += carried.cleared_links;
            let values = carried
                .values
                .iter()
                .map(|(name, value)| (name.as_str(), value.clone()));
            create.create_at(object.collection(), object.id().0, values)?;
            create.moved_from(object.collection(), object.id().0, old.0);
            conflicts.extend(conflicted(&inner, old.0, object)?);
        }
        let pending: Vec<(String, CoreObjectId)> = sources
            .iter()
            .map(|object| (object.collection().to_owned(), object.id().0))
            .collect();
        // The record comes first: a stop at any point after it leaves a
        // record the next open finishes or drops (`resume`).
        inner
            .view
            .record_pending_moves(old.0, new.0, &pending)
            .map_err(storage)?;
        let by = writer(&inner);
        write_change(&mut inner, new.0, create, by, |_| Ok(()))?;
        #[cfg(test)]
        if STOP_AFTER_NEW_SPACE.with(std::cell::Cell::get) {
            return Err(Error::new(ErrorCode::StorageFailed, "stopped by the test"));
        }
        finish(&mut inner, old.0, new.0, &pending)?;
        Ok(Moved {
            notice: Notice::OBJECT_MOVED,
            ids: sources.iter().map(Object::id).collect(),
            cleared_links,
            conflicts,
        })
    }
}

/// The writer of a change by this database's lock (RFC 0002, "The
/// writer's model").
fn writer(inner: &Inner) -> WriterModel {
    WriterModel {
        id: inner.model.id,
        version: inner.model.version,
    }
}

/// Writes the old space's move of `objects` to `new`, then forgets their
/// pending record.
fn finish(
    inner: &mut Inner,
    old: CoreSpaceId,
    new: CoreSpaceId,
    objects: &[(String, CoreObjectId)],
) -> Result<(), Error> {
    let model = inner.model.clone();
    let mut leave = Transaction::new(&model);
    for (collection, id) in objects {
        leave.move_to(collection, *id, new)?;
    }
    let by = writer(inner);
    write_change(inner, old, leave, by, |_| Ok(()))?;
    let ids: Vec<CoreObjectId> = objects.iter().map(|(_, id)| *id).collect();
    inner.view.clear_pending_moves(old, &ids).map_err(storage)
}

/// Finishes the moves a stop cut off, when the database opens (RFC 0016,
/// "The new space first"): an object the new space holds and the old one
/// holds live gets its move; any other pending record is dropped, since
/// either the move was written, or the new space never got the object and
/// nothing moved. A move this member may no longer write stays pending,
/// for a later open, and the open goes on.
pub(crate) fn resume(inner: &mut Inner) -> Result<(), Error> {
    let pending = inner.view.pending_moves().map_err(storage)?;
    let mut finish_now: std::collections::BTreeMap<
        (CoreSpaceId, CoreSpaceId),
        Vec<(String, CoreObjectId)>,
    > = Default::default();
    for record in pending {
        let (old, new, object) = (record.old_space, record.new_space, record.object);
        let held =
            |space: ncdb_core::ids::SpaceId| Object::read(inner, space, &record.collection, object);
        let arrived = held(new)?.is_some();
        let live = held(old)?.is_some_and(|o| o.moved_to().is_none());
        if arrived && live {
            finish_now
                .entry((old, new))
                .or_default()
                .push((record.collection.clone(), object));
        } else {
            inner
                .view
                .clear_pending_moves(old, &[object])
                .map_err(storage)?;
        }
    }
    for ((old, new), objects) in finish_now {
        let allowed = writable(inner, crate::id::SpaceId(old))
            .and_then(|()| not_stopped(inner, crate::id::SpaceId(old)));
        if allowed.is_ok() {
            finish(inner, old, new, &objects)?;
        }
    }
    Ok(())
}

#[cfg(test)]
thread_local! {
    /// Stops a move after its new space's change, as a device that stops
    /// in between (`crate::moving_tests`).
    static STOP_AFTER_NEW_SPACE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Makes the moves of this thread stop after the new space's change.
#[cfg(test)]
pub(crate) fn stop_after_new_space(stop: bool) {
    STOP_AFTER_NEW_SPACE.with(|cell| cell.set(stop));
}

/// Each object of `objects`, read again from the view of `space`: refused
/// when it is of another space, named twice, not held, moved already, or
/// holds a personal value.
fn sources(inner: &Inner, space: CoreSpaceId, objects: &[Object]) -> Result<Vec<Object>, Error> {
    let mut named = BTreeSet::new();
    objects
        .iter()
        .map(|object| {
            let id = object.id();
            let invalid = |text: String| Err(Error::new(ErrorCode::InvalidValue, text));
            if object.space().0 != space {
                return invalid(crate::texts::error::move_of_another_space(
                    id,
                    object.space(),
                ));
            }
            if !named.insert(id) {
                return invalid(crate::texts::error::move_named_twice(id));
            }
            let Some(read) = Object::read(inner, space, object.collection(), id.0)? else {
                return Err(Error::new(
                    ErrorCode::NotFound,
                    crate::texts::error::no_such_object(id, object.collection()),
                ));
            };
            if read.moved_to().is_some() {
                return invalid(crate::texts::error::move_already_moved(id));
            }
            let personal = read.fields().iter().any(|(name, value)| {
                value.is_some() && inner.model.person(read.collection(), name).is_some()
            });
            if personal {
                return Err(Error::new(
                    ErrorCode::NotAllowed,
                    crate::texts::error::move_personal_later(id),
                ));
            }
            Ok(read)
        })
        .collect()
}

/// What the new space's create of one object is written with.
struct Carried {
    values: Vec<(String, Value)>,
    cleared_links: usize,
}

/// The values of `object` in the new space: each set field; a link to an
/// object of `moved`, or none.
fn carried(
    inner: &Inner,
    object: &Object,
    moved: &BTreeSet<CoreObjectId>,
) -> Result<Carried, Error> {
    let fields = &inner.model.collections[object.collection()];
    let kept = |value: &Value| match value.0 {
        Repr::Id(target) if moved.contains(&target.0) => Some(value.clone()),
        _ => None,
    };
    let mut carried = Carried {
        values: Vec::new(),
        cleared_links: 0,
    };
    for (name, value) in object.fields() {
        let Some(value) = value else { continue };
        let elements = match &value.0 {
            Repr::List(elements) => elements.as_slice(),
            _ => &[],
        };
        let written = match fields.get(name) {
            Some(Field::Basic(_) | Field::Ref(_)) => Some(value.clone()),
            Some(Field::Set(_)) if !elements.is_empty() => Some(Value::add(elements.to_vec())),
            Some(Field::Link(_)) => {
                let target = kept(value);
                carried.cleared_links += usize::from(target.is_none());
                target
            }
            Some(Field::Links(_)) => {
                let targets: Vec<Value> = elements.iter().filter_map(kept).collect();
                carried.cleared_links += elements.len() - targets.len();
                (!targets.is_empty()).then(|| Value::add(targets))
            }
            Some(Field::Plugin(_)) => {
                return Err(Error::new(
                    ErrorCode::NotAllowed,
                    crate::texts::error::move_plugin_later(object.id(), name),
                ));
            }
            _ => None,
        };
        if let Some(written) = written {
            carried.values.push((name.clone(), written));
        }
    }
    Ok(carried)
}

/// The fields of `object` whose basic value, link or reference holds a
/// conflict in the copy of `space`: the move writes their winning values
/// (RFC 0016, "Conflicts at the move").
fn conflicted(
    inner: &Inner,
    space: CoreSpaceId,
    object: &Object,
) -> Result<Vec<(ObjectId, String)>, Error> {
    let Some(row) = inner
        .view
        .get(space, object.collection(), object.id().0)
        .map_err(storage)?
    else {
        return Ok(Vec::new());
    };
    let fields = &inner.model.collections[object.collection()];
    Ok(row
        .fields
        .iter()
        .filter(|(name, state)| {
            let FieldState::Basic(state) = state else {
                return false;
            };
            let read = match fields.get(*name) {
                Some(Field::Basic(t)) => t.read(state),
                Some(Field::Link(t)) => t.read(state),
                Some(Field::Ref(t)) => t.read(state),
                _ => return false,
            };
            !read.conflicts.is_empty()
        })
        .map(|(name, _)| (object.id(), name.clone()))
        .collect())
}
