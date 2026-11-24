//! What a space knows of the objects moved out of it (RFC 0016, "Later
//! edits", "Two moves at once", "The SDK call and its notice"; NOC-475):
//! the later edits of each moved object, carrying chosen ones over into the
//! new space, and the objects two members moved at once.
//!
//! Later edits are computed from the old space's log when the app asks,
//! not kept in the view, so they read the same after a reopen (pm, on
//! #462). Every member of the old space reads the same list.

use std::collections::BTreeSet;

use ncdb_core::cbor;
use ncdb_core::change::Operation;
use ncdb_core::field_op::{self, DELETE, Encoding, Origin};
use ncdb_core::view::Field;

use crate::db::{Inner, lock};
use crate::error::{Error, ErrorCode};
use crate::id::{ChangeId, ObjectId, SpaceId};
use crate::object::Object;
use crate::space::{Space, Written};
use crate::value::Value;

/// The calls on what moved out of a space, from [`Space::moves`].
pub struct Moves<'a> {
    space: &'a Space,
}

/// The later edits of one moved object (RFC 0016, "Later edits").
#[derive(Clone, Debug, PartialEq)]
pub struct LaterEdits {
    /// The moved object.
    pub id: ObjectId,
    /// The space it moved to.
    pub moved_to: SpaceId,
    /// Its later edits, in clock order.
    pub edits: Vec<LaterEdit>,
}

/// One later edit: an operation of the old space on a moved object that
/// its move did not see.
#[derive(Clone, Debug, PartialEq)]
pub struct LaterEdit {
    /// The object it edits.
    pub object: ObjectId,
    /// The object's collection.
    pub collection: String,
    /// The change that holds it.
    pub change: ChangeId,
    /// Its position in that change.
    pub position: u64,
    /// What it writes.
    pub edit: Edit,
}

/// What a later edit writes, as [`Moves::carry_over`] writes it again.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Edit {
    /// A basic value, a link or a reference set, or cleared with
    /// [`Value::clear`].
    Set {
        /// The field.
        field: String,
        /// The value.
        value: Value,
    },
    /// Elements added to a set or many links.
    Add {
        /// The field.
        field: String,
        /// The elements.
        elements: Vec<Value>,
    },
    /// Elements removed from a set or many links.
    Remove {
        /// The field.
        field: String,
        /// The elements.
        elements: Vec<Value>,
    },
    /// The object deleted.
    Delete,
    /// The object restored.
    Restore,
    /// Anything this SDK cannot carry over: a personal value, a plugin's
    /// operation, a create, or a value outside the field's kind.
    Other,
}

/// An object two members moved at once (RFC 0016, "Two moves at once").
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MoveConflict {
    /// The object.
    pub id: ObjectId,
    /// The space the move that won names: where its trace leads.
    pub moved_to: SpaceId,
    /// The spaces the moves that lost name; each may hold a live copy of
    /// the object, which a member of both spaces can delete or move back.
    pub lost: Vec<SpaceId>,
}

impl Space {
    /// What moved out of this space: later edits, carrying them over, and
    /// two moves at once (RFC 0016).
    pub fn moves(&self) -> Moves<'_> {
        Moves { space: self }
    }
}

impl Moves<'_> {
    /// The later edits of each object moved out of this space, by object
    /// id (RFC 0016, "Later edits"). An object without any is left out.
    pub async fn later_edits(&self) -> Result<Vec<LaterEdits>, Error> {
        let inner = lock(self.space.inner());
        later_edits(&inner, self.space.id())
    }

    /// Writes `edits`, later edits of this space, into the space their
    /// objects moved to, as one write, in the order given (RFC 0016,
    /// "Later edits"). Each is written as a new write there, so it wins
    /// over what was written before it in that space.
    ///
    /// Refused, with nothing written: an edit that is no later edit of
    /// this space now, or edits of objects that moved to different spaces
    /// (`InvalidValue`); an [`Edit::Other`] (`NotAllowed`).
    pub async fn carry_over(&self, edits: &[LaterEdit]) -> Result<Written, Error> {
        let (to, plan) = {
            let inner = lock(self.space.inner());
            plan(&inner, self.space.id(), edits)?
        };
        let destination = Space::handle(self.space.inner().clone(), to);
        destination
            .write(|tx| {
                for edit in &plan {
                    let (collection, id) = (edit.collection.as_str(), edit.object);
                    match &edit.edit {
                        Edit::Set { field, value } => {
                            tx.update(collection, id, [(field.as_str(), value.clone())])?
                        }
                        Edit::Add { field, elements } => tx.update(
                            collection,
                            id,
                            [(field.as_str(), Value::add(elements.clone()))],
                        )?,
                        Edit::Remove { field, elements } => tx.update(
                            collection,
                            id,
                            [(field.as_str(), Value::remove(elements.clone()))],
                        )?,
                        Edit::Delete => tx.delete(collection, id)?,
                        Edit::Restore => tx.restore(collection, id)?,
                        Edit::Other => unreachable!("refused by plan"),
                    }
                }
                Ok(())
            })
            .await
    }

    /// The objects of this space that members moved to two spaces or more
    /// at once (RFC 0016, "Two moves at once"), by object id. Every member
    /// of this space reads the same.
    pub async fn conflicts(&self) -> Result<Vec<MoveConflict>, Error> {
        let inner = lock(self.space.inner());
        let space = self.space.id();
        let state = &inner.spaces[&space.0];
        let Some(log) = inner.view.log(space.0) else {
            return Ok(Vec::new());
        };
        let objects = ncdb_core::moves::moved_objects(log, &state.checks);
        Ok(objects
            .into_iter()
            .filter_map(|(id, object)| {
                let moved_to = SpaceId(object.moved_to()?);
                let lost: Vec<SpaceId> = object
                    .moving()
                    .conflicts
                    .iter()
                    .filter_map(|entry| match &entry.value {
                        cbor::Value::Bytes(bytes) => bytes.clone().try_into().ok(),
                        _ => None,
                    })
                    .map(|space| SpaceId(ncdb_core::ids::SpaceId(space)))
                    .collect();
                (!lost.is_empty()).then_some(MoveConflict {
                    id: ObjectId(id),
                    moved_to,
                    lost,
                })
            })
            .collect())
    }
}

/// The later edits of each object moved out of `space`.
fn later_edits(inner: &Inner, space: SpaceId) -> Result<Vec<LaterEdits>, Error> {
    let Some(state) = inner.spaces.get(&space.0) else {
        return Ok(Vec::new());
    };
    let Some(log) = inner.view.log(space.0) else {
        return Ok(Vec::new());
    };
    let later = ncdb_core::moves::later_edits(log, &state.checks);
    let mut all = Vec::new();
    for (object, edits) in later {
        let Some(first) = edits.first() else { continue };
        let collection = first.operation.collection.clone();
        let Some(moved_to) =
            Object::read(inner, space.0, &collection, object)?.and_then(|read| read.moved_to())
        else {
            continue;
        };
        let edits = edits
            .into_iter()
            .map(|edit| LaterEdit {
                object: ObjectId(object),
                collection: edit.operation.collection.clone(),
                change: ChangeId(edit.id.change),
                position: edit.id.position,
                edit: read_edit(inner, &edit.operation, edit.origin()),
            })
            .collect();
        all.push(LaterEdits {
            id: ObjectId(object),
            moved_to,
            edits,
        });
    }
    Ok(all)
}

/// What `operation` writes, as the app reads it.
fn read_edit(inner: &Inner, operation: &Operation, origin: Origin) -> Edit {
    let collection = &operation.collection;
    let Some(name) = operation.field.as_deref() else {
        if operation.op != DELETE {
            return Edit::Other;
        }
        return match field_op::decode_object(operation, origin) {
            Ok(ncdb_core::object::ObjectOperation::Delete(write)) => match write.value {
                cbor::Value::Bool(true) => Edit::Delete,
                cbor::Value::Bool(false) => Edit::Restore,
                _ => Edit::Other,
            },
            _ => Edit::Other,
        };
    };
    if inner.model.person(collection, name).is_some() {
        return Edit::Other;
    }
    let field = name.to_owned();
    let set = |value: Option<Value>| match value {
        Some(value) => Edit::Set {
            field: field.clone(),
            value,
        },
        None => Edit::Other,
    };
    let value_of = |write: &ncdb_core::basic::Write,
                    read: &dyn Fn(&cbor::Value) -> Option<Value>| {
        match &write.value {
            cbor::Value::Null => Some(Value::clear()),
            value => read(value),
        }
    };
    match inner.model.field(collection, name) {
        Some(Field::Basic(t)) => match t.decode(operation, origin) {
            Ok(write) => set(value_of(&write, &|v| Value::from_basic(&t.kind, v))),
            Err(_) => Edit::Other,
        },
        Some(Field::Link(t)) => match t.decode(operation, origin) {
            Ok(write) => set(value_of(&write, &|v| Value::from_cbor(v, true))),
            Err(_) => Edit::Other,
        },
        Some(Field::Ref(t)) => match t.decode(operation, origin) {
            Ok(write) => set(value_of(&write, &Value::from_reference)),
            Err(_) => Edit::Other,
        },
        Some(Field::Set(t)) => elements(t.decode(operation, origin), field, |v| {
            Value::from_basic(&t.kind, v)
        }),
        Some(Field::Links(t)) => elements(t.decode(operation, origin), field, |v| {
            Value::from_cbor(v, true)
        }),
        _ => Edit::Other,
    }
}

/// An add or a remove of one element, as an [`Edit`].
fn elements(
    edit: Result<ncdb_core::set::Edit, ncdb_core::log::OperationParkReason>,
    field: String,
    read: impl Fn(&cbor::Value) -> Option<Value>,
) -> Edit {
    let Ok(edit) = edit else { return Edit::Other };
    let Some(element) = read(&edit.element) else {
        return Edit::Other;
    };
    match edit.action {
        ncdb_core::set::Action::Add { .. } => Edit::Add {
            field,
            elements: vec![element],
        },
        ncdb_core::set::Action::Remove { .. } => Edit::Remove {
            field,
            elements: vec![element],
        },
    }
}

/// The space `edits` go to and the edits as this space holds them,
/// checked against its later edits now: the caller's copy of an edit is
/// only a name for it.
fn plan(
    inner: &Inner,
    space: SpaceId,
    edits: &[LaterEdit],
) -> Result<(SpaceId, Vec<LaterEdit>), Error> {
    let now = later_edits(inner, space)?;
    let mut to = BTreeSet::new();
    let mut plan = Vec::new();
    for edit in edits {
        let held = now.iter().find_map(|later| {
            let held = later.edits.iter().find(|e| {
                e.object == edit.object && e.change == edit.change && e.position == edit.position
            })?;
            Some((later.moved_to, held))
        });
        let Some((moved_to, held)) = held else {
            return Err(Error::new(
                ErrorCode::InvalidValue,
                crate::texts::error::no_later_edit(edit.position, edit.change, edit.object),
            ));
        };
        if let Edit::Other = held.edit {
            return Err(Error::new(
                ErrorCode::NotAllowed,
                crate::texts::error::carry_over_unreadable(edit.position, edit.change),
            ));
        }
        to.insert(moved_to);
        plan.push(held.clone());
    }
    let mut to = to.into_iter();
    match (to.next(), to.next()) {
        (Some(destination), None) => Ok((destination, plan)),
        (None, _) => Ok((space, plan)),
        _ => Err(Error::new(
            ErrorCode::InvalidValue,
            crate::texts::error::CARRY_OVER_TO_SEVERAL_SPACES,
        )),
    }
}
