//! The fold into the view's tables (RFC 0006, "One change", step 3): the
//! [`Fold`] that [`Log::receive`](crate::log::Log) calls for each operation
//! of a change, inside the transaction that appends it. A field operation
//! loads the field's merge state from `fs_<collection>`, applies the
//! operation through its field type, writes the state back and the read
//! value into the field's column of `c_<collection>`. A create, a delete or
//! a move goes into the object's state in `objects`; a moved object keeps
//! no fields (`super::trace`). Un-folding clears a field's state or an
//! object's first (`super::clear`).

use rusqlite::types::Value as Sql;
use rusqlite::{Connection, params};

use super::Error;
pub(super) use super::columns::open_personal;
use super::columns::{column_value, personal_column, reference_column};
use super::fold_statements::{FoldWork, Statements};
use super::model::{Field, FieldState, Model, RESERVED, reserved_name};
use super::plugin;
use super::schema::{STAGING, column, objects_in, rows_in, states_in};
use super::state::{self, ObjectState};
use super::trace::out_of_indexes;
use std::collections::{BTreeMap, BTreeSet};

use crate::change::{Change, Operation};
use crate::field_op::{Encoding, Origin};
use crate::ids::{ChangeId, ObjectId, SpaceId};
use crate::keys::PersonKey;
use crate::log::OperationParkReason;
use crate::object::ObjectOperation;
use crate::personal::Place;
use crate::receive::Fold;

/// The person keys of a space, by person-key id.
pub(super) type PersonKeys = BTreeMap<[u8; 16], PersonKey>;

/// No person keys: a fold that opens no personal value.
pub(super) static NO_KEYS: PersonKeys = BTreeMap::new();

/// The fold of one change's operations into the tables of a space's
/// collections, on the connection of the transaction that appends it.
pub(super) struct SqlFold<'a> {
    pub(super) conn: &'a Connection,
    /// Every statement the fold runs on the view's tables.
    pub(super) statements: Statements<'a>,
    /// Whether each object the fold read or wrote is a trace and out of
    /// the indexes, as the `objects` table holds it now: every operation
    /// of a field asks the first, every new row the second, and the table
    /// changes only through [`SqlFold::keep_object`] (NOC-542). Two flags,
    /// not the state, so that a change of many objects does not hold all
    /// their states at once.
    objects: std::cell::RefCell<BTreeMap<ObjectId, Flags>>,
    pub(super) model: &'a Model,
    pub(super) space: SpaceId,
    /// The prefix of the tables it folds into: empty for the view's own,
    /// `r_` for a rebuild's staging tables.
    pub(super) prefix: &'static str,
    /// The person keys the fold opens the space's personal values with.
    keys: &'a PersonKeys,
    /// Whether the space folds by the model the SDK ships: its collections
    /// are then the model's `$` ones, under the names its changes give
    /// without the `$` (NOC-266).
    shipped: bool,
    /// The first error; the fold's calls cannot return one, so the
    /// transaction rolls back after the change.
    error: Option<Error>,
    /// The `(collection, object)` of each object the fold wrote or
    /// cleared, by the view's name of the collection (RFC 0006,
    /// "Folding", step 6).
    touched: BTreeSet<(String, ObjectId)>,
    /// The objects whose move was taken back: the fold kept none of their
    /// fields while they were traces, so the log's fold takes them again
    /// ([`Fold::take_revived`]; RFC 0016, "Moving back, and moving again").
    pub(super) revived: BTreeSet<ObjectId>,
    /// Whether the rich-text operation just folded is earlier than others
    /// the field holds that give its ids other content (RFC 0013, "The
    /// same id twice").
    displaces: std::cell::Cell<bool>,
    /// The fields to fold again in stamp order, as the changes name them.
    refolds: BTreeSet<(ObjectId, String, String)>,
    /// The field a refold in stamp order folds now, by object and field
    /// name, whose state the fold keeps while it runs even on a trace
    /// ([`Fold::refolding`]).
    refolding: Option<(ObjectId, String)>,
    /// Whether the operation folded now is a rejected change's, placed as
    /// placeholders ([`Fold::place`]).
    placing: bool,
}

impl<'a> SqlFold<'a> {
    pub(super) fn new(conn: &'a Connection, model: &'a Model, space: SpaceId) -> SqlFold<'a> {
        SqlFold {
            conn,
            statements: Statements::new(conn),
            objects: std::cell::RefCell::new(BTreeMap::new()),
            model,
            space,
            prefix: "",
            keys: &NO_KEYS,
            shipped: false,
            error: None,
            touched: BTreeSet::new(),
            revived: BTreeSet::new(),
            displaces: std::cell::Cell::new(false),
            refolds: BTreeSet::new(),
            refolding: None,
            placing: false,
        }
    }

    /// The same fold, opening personal values with `keys`.
    pub(super) fn with_keys(self, keys: &'a PersonKeys) -> SqlFold<'a> {
        SqlFold { keys, ..self }
    }

    /// The same fold, for a space whose genesis names `space_model`, when
    /// the member can read it: it folds by the shipped model when the
    /// space is of it, and by the app's otherwise.
    pub(super) fn for_space(mut self, space_model: Option<[u8; 16]>) -> SqlFold<'a> {
        self.select_model(space_model);
        self
    }

    /// The view's name of a collection a change of the space names: the
    /// shipped model's `$` name in a space of that model; the name itself
    /// in a space of the app's, where no name with `$` is the app's.
    fn name(&self, collection: &str) -> Option<String> {
        if self.shipped {
            Some(reserved_name(collection))
        } else if collection.starts_with(RESERVED) {
            None
        } else {
            Some(collection.to_owned())
        }
    }

    /// A fold into a rebuild's staging tables (RFC 0006, "How").
    pub(super) fn staged(conn: &'a Connection, model: &'a Model, space: SpaceId) -> SqlFold<'a> {
        SqlFold {
            prefix: STAGING,
            ..SqlFold::new(conn, model, space)
        }
    }

    pub(super) fn rows(&self, collection: &str) -> String {
        rows_in(self.prefix, collection)
    }

    pub(super) fn states(&self, collection: &str) -> String {
        states_in(self.prefix, collection)
    }

    /// The first error the fold met, if any: the transaction must then
    /// roll back.
    pub(super) fn finish(self) -> Result<BTreeSet<(String, ObjectId)>, Error> {
        self.error.map_or(Ok(self.touched), Err)
    }

    /// What the fold did to the view's tables.
    pub(super) fn work(&self) -> FoldWork {
        self.statements.work()
    }

    /// The view's name of the collection `object` is created in, when the
    /// fold holds a create of it.
    fn collection_of(&self, object: ObjectId) -> Option<String> {
        let state = self.object(object).ok()?;
        match state.object().collection().value?.value {
            crate::cbor::Value::Text(name) => Some(name),
            _ => None,
        }
    }

    /// Keeps the first error.
    fn keep(&mut self, result: Result<(), Error>) {
        if let Err(error) = result
            && self.error.is_none()
        {
            self.error = Some(error);
        }
    }

    /// Writes a field's new state and its read value.
    fn write(
        &self,
        collection: &str,
        field: &str,
        object: ObjectId,
        bytes: Vec<u8>,
        read: Option<Sql>,
    ) -> Result<(), Error> {
        self.statements.execute(
            &format!(
                "INSERT INTO {} (space_id, object_id, field, state) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT DO UPDATE SET state = excluded.state",
                self.states(collection)
            ),
            params![self.space.0, object.0, field, bytes],
        )?;
        self.row(collection, object)?;
        if let Some(read) = read {
            self.statements.execute(
                &format!(
                    "UPDATE {} SET {} = ?3 WHERE space_id = ?1 AND object_id = ?2",
                    self.rows(collection),
                    column(field)
                ),
                params![self.space.0, object.0, read],
            )?;
        }
        Ok(())
    }

    /// Makes sure `c_<collection>` has a row for the object; a new row
    /// takes whether the object is out of the indexes.
    fn row(&self, collection: &str, object: ObjectId) -> Result<(), Error> {
        let inserted = self.statements.execute(
            &format!(
                "INSERT INTO {} (space_id, object_id) VALUES (?1, ?2) ON CONFLICT DO NOTHING",
                self.rows(collection)
            ),
            params![self.space.0, object.0],
        )?;
        if inserted == 1 && self.flags(object)?.out {
            self.mark_deleted(collection, object, true)?;
        }
        Ok(())
    }

    /// The object's creates, deletes and moves, empty when it has none.
    pub(super) fn object(&self, object: ObjectId) -> Result<ObjectState, Error> {
        self.statements.count_object_read();
        let bytes: Option<Vec<u8>> = self.statements.query_row(
            &format!(
                "SELECT state FROM {} WHERE space_id = ?1 AND object_id = ?2",
                objects_in(self.prefix)
            ),
            params![self.space.0, object.0],
            |r| r.get(0),
        )?;
        let state = match bytes {
            Some(bytes) => ObjectState::read(&bytes)?,
            None => ObjectState::default(),
        };
        self.objects.borrow_mut().insert(object, Flags::of(&state));
        Ok(state)
    }

    /// Whether the object is a trace and out of the indexes, read once.
    fn flags(&self, object: ObjectId) -> Result<Flags, Error> {
        if let Some(flags) = self.objects.borrow().get(&object) {
            return Ok(*flags);
        }
        Ok(Flags::of(&self.object(object)?))
    }

    /// Writes the object's state into `objects`, or removes its row when
    /// `state` is `None`, and remembers its flags. The only way the fold
    /// changes the table, so that what it remembers stays what the table
    /// holds.
    pub(super) fn keep_object(
        &self,
        object: ObjectId,
        state: Option<&ObjectState>,
    ) -> Result<(), Error> {
        match state {
            Some(state) => self.statements.execute(
                &format!(
                    "INSERT INTO {} (space_id, object_id, state) VALUES (?1, ?2, ?3)
                     ON CONFLICT DO UPDATE SET state = excluded.state",
                    objects_in(self.prefix)
                ),
                params![self.space.0, object.0, state.bytes()],
            )?,
            None => self.statements.execute(
                &format!(
                    "DELETE FROM {} WHERE space_id = ?1 AND object_id = ?2",
                    objects_in(self.prefix)
                ),
                params![self.space.0, object.0],
            )?,
        };
        let flags = state.map_or_else(Flags::default, Flags::of);
        self.objects.borrow_mut().insert(object, flags);
        Ok(())
    }

    /// Records in the object's row of `c_<collection>`, if it has one,
    /// whether the object is out of the indexes, deleted or moved: the
    /// declared indexes hold only the rows of objects that are neither
    /// (RFC 0006, "Declared indexes"; RFC 0016, "What the members of the
    /// old space keep"). The column keeps its name `deleted`, so that no
    /// view's tables change.
    pub(super) fn mark_deleted(
        &self,
        collection: &str,
        object: ObjectId,
        deleted: bool,
    ) -> Result<(), Error> {
        self.statements.execute(
            &format!(
                "UPDATE {} SET deleted = ?3 WHERE space_id = ?1 AND object_id = ?2",
                self.rows(collection)
            ),
            params![self.space.0, object.0, deleted],
        )?;
        Ok(())
    }

    /// Folds a field operation the model knows, or says why it parks.
    fn fold_field(
        &self,
        origin: Origin,
        operation: &Operation,
    ) -> Result<Result<(), OperationParkReason>, Error> {
        let collection = &operation.collection;
        let name = operation.field.as_deref().expect("a field operation");
        let field = self.model.field(collection, name).expect("a known field");
        let state = load(
            &self.statements,
            self.prefix,
            self.space,
            collection,
            name,
            operation.object,
            field,
        )?;
        let personal = self.model.person(collection, name).is_some();
        let folded = match (field, state) {
            (Field::Basic(t), FieldState::Basic(s)) if personal => apply(t, s, operation, origin)
                .map(|(bytes, read)| {
                    let place = Place {
                        space: self.space,
                        object: operation.object,
                        person_key: [0; 16],
                        collection,
                        field: name,
                    };
                    (
                        bytes,
                        Some(personal_column(t, place, read.value, self.keys)),
                    )
                }),
            (Field::Basic(t), FieldState::Basic(s)) => apply(t, s, operation, origin)
                .map(|(bytes, read)| (bytes, Some(column_value(read.value)))),
            (Field::Link(t), FieldState::Basic(s)) => apply(t, s, operation, origin)
                .map(|(bytes, read)| (bytes, Some(column_value(read.value)))),
            (Field::Ref(t), FieldState::Basic(s)) => apply(t, s, operation, origin)
                .map(|(bytes, read)| (bytes, Some(reference_column(read.value)))),
            (Field::Person(t), FieldState::Basic(s)) => {
                apply(t, s, operation, origin).map(|(bytes, _)| (bytes, None))
            }
            (Field::Set(t), FieldState::Set(s)) => {
                apply(t, s, operation, origin).map(|(bytes, _)| (bytes, None))
            }
            (Field::Links(t), FieldState::Set(s)) => {
                apply(t, s, operation, origin).map(|(bytes, _)| (bytes, None))
            }
            (Field::Plugin(t), FieldState::Plugin(s)) => {
                plugin::apply(t, &s, operation, origin)?.map(|bytes| (bytes, None))
            }
            #[cfg(ncdb_rich_text)]
            (Field::RichText(t), FieldState::RichText(s)) => {
                let (folded, displaces) = rich_text(t, s, operation, origin, self.placing);
                self.displaces.set(displaces);
                folded.map(|bytes| (bytes, None))
            }
            _ => unreachable!("a field's state matches its type"),
        };
        // The trace of a moved object keeps no field: the operation is
        // decoded and folded, so that it parks as on every copy, but the
        // view keeps nothing of it (RFC 0016). Whether a rich-text
        // operation parks depends on the field's state, so a refold in
        // stamp order decides it, keeping the state while it runs.
        let trace = self.flags(operation.object)?.moved
            && self.refolding != Some((operation.object, name.to_owned()));
        #[cfg(ncdb_rich_text)]
        if trace && matches!(field, Field::RichText(_)) {
            self.displaces.set(true);
        }
        Ok(match folded {
            Ok(_) if trace => Ok(()),
            Ok((bytes, read)) => {
                self.write(collection, name, operation.object, bytes, read)?;
                Ok(())
            }
            Err(reason) => Err(reason),
        })
    }

    fn fold_object_operation(
        &mut self,
        object: ObjectId,
        operation: &ObjectOperation,
    ) -> Result<(), Error> {
        let mut state = self.object(object)?;
        let was_out = out_of_indexes(&state);
        let was_moved = state.object().moved_to().is_some();
        state.insert(operation.clone());
        self.keep_object(object, Some(&state))?;
        // A delete, a move, a restore or a take-back takes the object's
        // rows out of the declared indexes or puts them back, in whichever
        // collection's table the object has one.
        let out = out_of_indexes(&state);
        if out != was_out {
            for collection in self.model.collections.keys() {
                self.mark_deleted(collection, object, out)?;
            }
        }
        match (was_moved, state.object().moved_to().is_some()) {
            (false, true) => self.drop_fields(object)?,
            (true, false) => {
                self.revived.insert(object);
            }
            _ => {}
        }
        // A create puts the object into the table of its collection, when
        // the space's model has it: a space of the app's model never
        // reaches a `$` table, nor a space of the shipped model the app's.
        if let ObjectOperation::Create(create) = operation
            && self.model.collections.contains_key(&create.collection)
            && self.shipped == create.collection.starts_with(RESERVED)
        {
            self.row(&create.collection, object)?;
        }
        Ok(())
    }
}

/// What the fold asks of an object's state for every operation.
#[derive(Clone, Copy, Default)]
struct Flags {
    /// The object moved to another space: it is a trace.
    moved: bool,
    /// The object is out of the declared indexes: deleted or moved.
    out: bool,
}

impl Flags {
    fn of(state: &ObjectState) -> Flags {
        Flags {
            moved: state.object().moved_to().is_some(),
            out: out_of_indexes(state),
        }
    }
}

/// The merge state of a field, empty when nobody wrote it.
pub(super) fn load(
    statements: &Statements<'_>,
    prefix: &str,
    space: SpaceId,
    collection: &str,
    field_name: &str,
    object: ObjectId,
    field: &Field,
) -> Result<FieldState, Error> {
    let bytes: Option<Vec<u8>> = statements.query_row(
        &format!(
            "SELECT state FROM {} WHERE space_id = ?1 AND object_id = ?2 AND field = ?3",
            states_in(prefix, collection)
        ),
        params![space.0, object.0, field_name],
        |r| r.get(0),
    )?;
    Ok(match (bytes, field.empty()) {
        (None, empty) => empty,
        (Some(bytes), FieldState::Basic(_)) => FieldState::Basic(state::basic(&bytes)?),
        (Some(bytes), FieldState::Set(_)) => FieldState::Set(state::set(&bytes)?),
        (Some(bytes), FieldState::Plugin(_)) => FieldState::Plugin(bytes),
        #[cfg(ncdb_rich_text)]
        (Some(bytes), FieldState::RichText(_)) => FieldState::RichText(state::rich_text(&bytes)?),
    })
}

/// Folds a rich-text operation into `state`, or places it there as a
/// rejected change's (`placing`): the new state's bytes, or why it parks,
/// and whether it displaces operations or placeholders the state holds,
/// which park or give way when the field is folded again in stamp order.
#[cfg(ncdb_rich_text)]
fn rich_text(
    field_type: &crate::rich_text::RichText,
    mut state: crate::rich_text::State,
    operation: &Operation,
    origin: Origin,
    placing: bool,
) -> (Result<Vec<u8>, OperationParkReason>, bool) {
    use crate::field::FieldType;
    use crate::rich_text::Fit;
    let edit = match field_type.decode(operation, origin) {
        Ok(edit) => edit,
        Err(reason) => return (Err(reason), false),
    };
    if placing {
        let displaces = matches!(field_type.fit_placeholder(&state, &edit), Fit::Displaces(_));
        field_type.place(&mut state, &edit);
        return (Ok(field_type.state_bytes(&state)), displaces);
    }
    let displaces = matches!(field_type.fit(&state, &edit), Fit::Displaces(_));
    let folded = field_type
        .try_apply(&mut state, &edit)
        .map(|()| field_type.state_bytes(&state));
    (folded, displaces)
}

/// Decodes `operation` with its field type, applies it to `state`, and
/// returns the new state's bytes and read value.
fn apply<T: Encoding>(
    field_type: &T,
    mut state: T::State,
    operation: &Operation,
    origin: Origin,
) -> Result<(Vec<u8>, T::Read), OperationParkReason> {
    let op = field_type.decode(operation, origin)?;
    field_type.apply(&mut state, &op);
    Ok((field_type.state_bytes(&state), field_type.read(&state)))
}

impl Fold for SqlFold<'_> {
    /// From the `log` table, in the transaction the fold writes in, so a
    /// change appended earlier in it is found too. A frame the table does
    /// not hold or cannot decode fails the transaction.
    fn stored(&mut self, space: SpaceId, id: ChangeId) -> Option<Change> {
        match super::log_table::stored(self.conn, space, id) {
            Ok(change) => Some(change),
            Err(error) => {
                self.keep(Err(error));
                None
            }
        }
    }

    fn model_id(&self) -> [u8; 16] {
        match (self.shipped, self.model.reserved) {
            (true, Some(reserved)) => reserved.id,
            _ => self.model.id,
        }
    }

    fn select_model(&mut self, space_model: Option<[u8; 16]>) -> bool {
        let shipped = self.model.reserved.map(|reserved| reserved.id);
        match space_model {
            Some(model) if Some(model) == shipped => {
                self.shipped = true;
                true
            }
            Some(model) if model != self.model.id => false,
            _ => {
                self.shipped = false;
                true
            }
        }
    }

    fn knows_collection(&self, collection: &str) -> bool {
        self.name(collection)
            .is_some_and(|name| self.model.collections.contains_key(&name))
    }

    fn knows_field(&self, collection: &str, field: &str) -> bool {
        self.name(collection)
            .is_some_and(|name| self.model.field(&name, field).is_some())
    }

    fn unregistered(&self, collection: &str, field: &str) -> bool {
        self.name(collection)
            .is_some_and(|name| self.model.unregistered(&name, field))
    }

    fn fold(&mut self, origin: Origin, operation: &Operation) -> Result<(), OperationParkReason> {
        if self.error.is_some() {
            return Ok(());
        }
        let original_collection = operation.collection.clone();
        let renamed;
        let operation = match self.name(&operation.collection) {
            Some(name) if name != operation.collection => {
                let mut op = operation.clone();
                op.collection = name;
                renamed = op;
                &renamed
            }
            _ => operation,
        };
        let named = (
            operation.object,
            original_collection.clone(),
            operation.field.clone().expect("a field operation"),
        );
        match self.fold_field(origin, operation) {
            Ok(folded) => {
                if folded.is_ok() {
                    self.touched
                        .insert((operation.collection.clone(), operation.object));
                }
                if self.displaces.take() {
                    self.refolds.insert(named);
                }
                folded
            }
            Err(error) => {
                self.keep(Err(error));
                Ok(())
            }
        }
    }

    fn fold_object(&mut self, object: ObjectId, operation: &ObjectOperation) {
        if self.error.is_none() {
            let operation = match operation {
                // A create names its collection as the change does; the
                // object's state keeps the view's name of it.
                ObjectOperation::Create(create) => {
                    let mut create = create.clone();
                    create.collection = self
                        .name(&create.collection)
                        .unwrap_or_else(|| create.collection.clone());
                    ObjectOperation::Create(create)
                }
                other => other.clone(),
            };
            let result = self.fold_object_operation(object, &operation);
            let collection = match &operation {
                ObjectOperation::Create(create) => Some(create.collection.clone()),
                ObjectOperation::Delete(_)
                | ObjectOperation::Move(_)
                | ObjectOperation::MovedFrom(_) => self.collection_of(object),
            };
            if result.is_ok()
                && let Some(collection) = collection
            {
                self.touched.insert((collection, object));
            }
            self.keep(result);
        }
    }

    fn clear_field(&mut self, object: ObjectId, collection: &str, field: &str) {
        let Some(collection) = self.name(collection) else {
            return;
        };
        if self.error.is_none() && self.model.field(&collection, field).is_some() {
            let result = self.clear_field_state(&collection, field, object);
            if result.is_ok() {
                self.touched.insert((collection, object));
            }
            self.keep(result);
        }
    }

    fn take_revived(&mut self) -> BTreeSet<ObjectId> {
        std::mem::take(&mut self.revived)
    }

    fn keeps_placeholders(&self, collection: &str, field: &str) -> bool {
        #[cfg(ncdb_rich_text)]
        if let Some(name) = self.name(collection) {
            return matches!(self.model.field(&name, field), Some(Field::RichText(_)));
        }
        let _ = (collection, field);
        false
    }

    fn place(&mut self, origin: Origin, operation: &Operation) {
        if !self.keeps_placeholders(
            &operation.collection,
            operation.field.as_deref().unwrap_or(""),
        ) {
            return;
        }
        self.placing = true;
        // A placeholder that does not decode leaves the field as it was.
        let _ = self.fold(origin, operation);
        self.placing = false;
    }

    fn take_refolds(&mut self) -> Vec<(ObjectId, String, String)> {
        std::mem::take(&mut self.refolds).into_iter().collect()
    }

    fn refolding(&mut self, field: Option<(ObjectId, &str, &str)>) {
        if let Some((object, _, name)) = field {
            self.refolding = Some((object, name.to_owned()));
            return;
        }
        // Done: a trace's field state, kept for the refold, goes again.
        let Some((object, name)) = self.refolding.take() else {
            return;
        };
        let trace = self
            .object(object)
            .map(|state| state.object().moved_to().is_some());
        match trace {
            Ok(true) => {
                // The view's name of the collection, which the states of
                // its fields are kept under.
                if let Some(collection) = self.collection_of(object) {
                    let cleared = self.clear_field_state(&collection, &name, object);
                    if cleared.is_ok() {
                        self.touched.insert((collection, object));
                    }
                    self.keep(cleared);
                }
            }
            Ok(false) => {}
            Err(error) => self.keep(Err(error)),
        }
    }

    fn clear_object(&mut self, object: ObjectId) {
        if self.error.is_none() {
            let collection = self.collection_of(object);
            let result = self.clear_object_state(object);
            if result.is_ok()
                && let Some(collection) = collection
            {
                self.touched.insert((collection, object));
            }
            self.keep(result);
        }
    }
}
