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
        self.move_objects_in_steps(objects, destination, true)
    }

    /// [`Space::move_objects`], stopping after the new space's change
    /// when `finish` is false, as a device that stops between the two
    /// changes (`crate::moving_tests`). Only a test asks for that; every
    /// other caller finishes the move.
    fn move_objects_in_steps(
        &self,
        objects: &[Object],
        destination: &Space,
        finish_now: bool,
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
        #[cfg(ncdb_rich_text)]
        for object in &sources {
            back_without_rich_text(&inner, new.0, object)?;
        }
        let moved: BTreeSet<CoreObjectId> = sources.iter().map(|o| o.id().0).collect();
        let model = inner.model.clone();
        let mut create = Transaction::new(&model);
        let mut cleared_links = 0;
        let mut conflicts = Vec::new();
        #[cfg(ncdb_rich_text)]
        let mut leased = Vec::new();
        for object in &sources {
            #[cfg_attr(not(ncdb_rich_text), allow(unused_mut))]
            let mut carried = carried(&inner, object, &moved)?;
            cleared_links += carried.cleared_links;
            #[cfg(ncdb_rich_text)]
            for (name, document) in std::mem::take(&mut carried.documents) {
                let collection = object.collection();
                let re_created = crate::rich_text_copy::re_create(
                    &mut inner,
                    new.0,
                    collection,
                    object.id().0,
                    &name,
                    &document,
                );
                let re_created = match re_created {
                    Ok(re_created) => re_created,
                    Err(error) => {
                        crate::rich_text_copy::release(&mut inner, &leased, false)?;
                        return Err(error);
                    }
                };
                carried.values.push((name, re_created.value.clone()));
                leased.push(re_created);
            }
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
        let written = write_change(&mut inner, new.0, create, by, |_| Ok(()));
        #[cfg(ncdb_rich_text)]
        crate::rich_text_copy::release(&mut inner, &leased, written.is_ok())?;
        written?;
        if finish_now {
            finish(&mut inner, old.0, new.0, &pending)?;
        }
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

/// Moves `objects` of `space` to `destination`, stopping after the new
/// space's change: the pending record stays, as on a device that stops
/// between the two changes (`crate::moving_tests`).
#[cfg(test)]
pub(crate) fn move_without_finishing(
    space: &Space,
    objects: &[Object],
    destination: &Space,
) -> Result<Moved, Error> {
    space.move_objects_in_steps(objects, destination, false)
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
    /// Each written rich-text field and its document, which the move
    /// re-creates in the new space (RFC 0016, "Its rich-text fields").
    #[cfg(ncdb_rich_text)]
    documents: Vec<(String, Vec<u8>)>,
}

/// The values of `object` in the new space: each set field; a link to an
/// object of `moved`, or none; and the documents of its written rich-text
/// fields.
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
        #[cfg(ncdb_rich_text)]
        documents: Vec::new(),
    };
    for (name, value) in object.fields() {
        // A plugin field whose type no plugin implements is in no model and
        // reads unset: nothing to carry.
        let Some(field) = fields.get(name) else {
            continue;
        };
        // A rich-text field reads no value: its document is re-created.
        #[cfg(ncdb_rich_text)]
        if matches!(field, Field::RichText(_)) {
            let document = object.rich_text_document(inner, name)?;
            if !document.is_empty() {
                carried.documents.push((name.clone(), document));
            }
        }
        let Some(value) = value else { continue };
        let elements = match &value.0 {
            Repr::List(elements) => elements.as_slice(),
            _ => &[],
        };
        let written = match field {
            Field::Basic(_) | Field::Ref(_) => Some(value.clone()),
            Field::Set(_) => (!elements.is_empty()).then(|| Value::add(elements.to_vec())),
            Field::Link(_) => {
                let target = kept(value);
                carried.cleared_links += usize::from(target.is_none());
                target
            }
            Field::Links(_) => {
                let targets: Vec<Value> = elements.iter().filter_map(kept).collect();
                carried.cleared_links += elements.len() - targets.len();
                (!targets.is_empty()).then(|| Value::add(targets))
            }
            Field::Plugin(_) => {
                return Err(Error::new(
                    ErrorCode::NotAllowed,
                    crate::texts::error::move_plugin_later(object.id(), name),
                ));
            }
            // `$person` is no field an object reads.
            Field::Person(_) => None,
            // A rich-text field reads no value; its document is taken
            // above.
            #[cfg(ncdb_rich_text)]
            Field::RichText(_) => None,
        };
        if let Some(written) = written {
            carried.values.push((name.clone(), written));
        }
    }
    Ok(carried)
}

/// Refuses a move of `object` into `space` when the copy of `space`
/// already holds the object, as after a move back, and one of its
/// rich-text fields was written: re-creating the document there would show
/// its text twice, and how a document moves back is open (NOC-560).
#[cfg(ncdb_rich_text)]
fn back_without_rich_text(inner: &Inner, space: CoreSpaceId, object: &Object) -> Result<(), Error> {
    let held = inner
        .view
        .get(space, object.collection(), object.id().0)
        .map_err(storage)?;
    if held.is_none() {
        return Ok(());
    }
    let fields = &inner.model.collections[object.collection()];
    for (name, field) in fields {
        let Field::RichText(_) = field else { continue };
        if !object.rich_text_document(inner, name)?.is_empty() {
            return Err(Error::new(
                ErrorCode::NotAllowed,
                crate::texts::error::move_rich_text_back_later(object.id(), name),
            ));
        }
    }
    Ok(())
}

/// The fields of `object` whose basic value, link or reference holds a
/// conflict in the copy of `space`, or whose rich-text document holds a
/// lost operation: the move writes their winning values (RFC 0016,
/// "Conflicts at the move", "Its rich-text fields").
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
        .filter(|(name, state)| match fields.get(*name) {
            Some(Field::Basic(t)) => {
                matches!(state, FieldState::Basic(s) if !t.read(s).conflicts.is_empty())
            }
            Some(Field::Link(t)) => {
                matches!(state, FieldState::Basic(s) if !t.read(s).conflicts.is_empty())
            }
            Some(Field::Ref(t)) => {
                matches!(state, FieldState::Basic(s) if !t.read(s).conflicts.is_empty())
            }
            #[cfg(ncdb_rich_text)]
            Some(Field::RichText(t)) => {
                matches!(state, FieldState::RichText(s) if !t.read(s).lost.is_empty())
            }
            // Sets and links of many keep every element; the person field
            // and a plugin's state hold no conflict the move writes the
            // winner of.
            Some(Field::Set(_) | Field::Links(_) | Field::Person(_) | Field::Plugin(_)) | None => {
                false
            }
        })
        .map(|(name, _)| (object.id(), name.clone()))
        .collect())
}
