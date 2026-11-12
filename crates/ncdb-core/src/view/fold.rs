//! The fold into the view's tables (RFC 0006, "One change", step 3): the
//! [`Fold`] that [`Log::receive`](crate::log::Log) calls for each operation
//! of a change, inside the transaction that appends it. A field operation
//! loads the field's merge state from `fs_<collection>`, applies the
//! operation through its field type, writes the state back and the read
//! value into the field's column of `c_<collection>`. A create or a delete
//! goes into the object's state in `objects`. Un-folding clears a field's
//! state or an object's, and the row of an object nothing else holds,
//! before the changes still folded fold into them again.

use rusqlite::types::Value as Sql;
use rusqlite::{Connection, OptionalExtension, params};

use super::Error;
use super::model::{Field, FieldState, Model};
use super::schema::{STAGING, column, objects_in, rows_in, states_in};
use super::state::{self, ObjectState};
use crate::basic::Entry;
use crate::cbor::Value;
use crate::change::Operation;
use crate::field_op::{Encoding, Origin};
use crate::ids::{ObjectId, SpaceId};
use crate::log::OperationParkReason;
use crate::object::ObjectOperation;
use crate::receive::Fold;
use crate::reference;

/// The fold of one change's operations into the tables of a space's
/// collections, on the connection of the transaction that appends it.
pub(super) struct SqlFold<'a> {
    conn: &'a Connection,
    model: &'a Model,
    space: SpaceId,
    /// The prefix of the tables it folds into: empty for the view's own,
    /// `r_` for a rebuild's staging tables.
    prefix: &'static str,
    /// The first error; the fold's calls cannot return one, so the
    /// transaction rolls back after the change.
    error: Option<Error>,
}

impl<'a> SqlFold<'a> {
    pub(super) fn new(conn: &'a Connection, model: &'a Model, space: SpaceId) -> SqlFold<'a> {
        SqlFold {
            conn,
            model,
            space,
            prefix: "",
            error: None,
        }
    }

    /// A fold into a rebuild's staging tables (RFC 0006, "How").
    pub(super) fn staged(conn: &'a Connection, model: &'a Model, space: SpaceId) -> SqlFold<'a> {
        SqlFold {
            prefix: STAGING,
            ..SqlFold::new(conn, model, space)
        }
    }

    fn rows(&self, collection: &str) -> String {
        rows_in(self.prefix, collection)
    }

    fn states(&self, collection: &str) -> String {
        states_in(self.prefix, collection)
    }

    /// The first error the fold met, if any: the transaction must then
    /// roll back.
    pub(super) fn finish(self) -> Result<(), Error> {
        self.error.map_or(Ok(()), Err)
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
        self.conn.execute(
            &format!(
                "INSERT INTO {} (space_id, object_id, field, state) VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT DO UPDATE SET state = excluded.state",
                self.states(collection)
            ),
            params![self.space.0, object.0, field, bytes],
        )?;
        self.row(collection, object)?;
        if let Some(read) = read {
            self.conn.execute(
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
    /// takes whether the object is deleted.
    fn row(&self, collection: &str, object: ObjectId) -> Result<(), Error> {
        let inserted = self.conn.execute(
            &format!(
                "INSERT INTO {} (space_id, object_id) VALUES (?1, ?2) ON CONFLICT DO NOTHING",
                self.rows(collection)
            ),
            params![self.space.0, object.0],
        )?;
        if inserted == 1 && self.object(object)?.object().is_deleted() {
            self.mark_deleted(collection, object, true)?;
        }
        Ok(())
    }

    /// The object's creates and deletes, empty when it has none.
    fn object(&self, object: ObjectId) -> Result<ObjectState, Error> {
        let bytes: Option<Vec<u8>> = self
            .conn
            .query_row(
                &format!(
                    "SELECT state FROM {} WHERE space_id = ?1 AND object_id = ?2",
                    objects_in(self.prefix)
                ),
                params![self.space.0, object.0],
                |r| r.get(0),
            )
            .optional()?;
        match bytes {
            Some(bytes) => ObjectState::read(&bytes),
            None => Ok(ObjectState::default()),
        }
    }

    /// Records in the object's row of `c_<collection>`, if it has one,
    /// whether the object is deleted: the declared indexes hold only the
    /// rows of objects that are not (RFC 0006, "Declared indexes").
    fn mark_deleted(&self, collection: &str, object: ObjectId, deleted: bool) -> Result<(), Error> {
        self.conn.execute(
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
            self.conn,
            self.prefix,
            self.space,
            collection,
            name,
            operation.object,
            field,
        )?;
        let folded = match (field, state) {
            (Field::Basic(t), FieldState::Basic(s)) => apply(t, s, operation, origin)
                .map(|(bytes, read)| (bytes, Some(column_value(read.value)))),
            (Field::Link(t), FieldState::Basic(s)) => apply(t, s, operation, origin)
                .map(|(bytes, read)| (bytes, Some(column_value(read.value)))),
            (Field::Ref(t), FieldState::Basic(s)) => apply(t, s, operation, origin)
                .map(|(bytes, read)| (bytes, Some(reference_column(read.value)))),
            (Field::Set(t), FieldState::Set(s)) => {
                apply(t, s, operation, origin).map(|(bytes, _)| (bytes, None))
            }
            (Field::Links(t), FieldState::Set(s)) => {
                apply(t, s, operation, origin).map(|(bytes, _)| (bytes, None))
            }
            _ => unreachable!("a field's state matches its type"),
        };
        Ok(match folded {
            Ok((bytes, read)) => {
                self.write(collection, name, operation.object, bytes, read)?;
                Ok(())
            }
            Err(reason) => Err(reason),
        })
    }

    fn fold_object_operation(
        &self,
        object: ObjectId,
        operation: &ObjectOperation,
    ) -> Result<(), Error> {
        let mut state = self.object(object)?;
        let was_deleted = state.object().is_deleted();
        state.insert(operation.clone());
        self.conn.execute(
            &format!(
                "INSERT INTO {} (space_id, object_id, state) VALUES (?1, ?2, ?3)
                 ON CONFLICT DO UPDATE SET state = excluded.state",
                objects_in(self.prefix)
            ),
            params![self.space.0, object.0, state.bytes()],
        )?;
        // A delete or a restore takes the object's rows out of the
        // declared indexes or puts them back, in whichever collection's
        // table the object has one.
        let deleted = state.object().is_deleted();
        if deleted != was_deleted {
            for collection in self.model.collections.keys() {
                self.mark_deleted(collection, object, deleted)?;
            }
        }
        // A create puts the object into the table of its collection.
        if let ObjectOperation::Create(create) = operation
            && self.model.collections.contains_key(&create.collection)
        {
            self.row(&create.collection, object)?;
        }
        Ok(())
    }
}

impl SqlFold<'_> {
    /// Forgets a field's state and empties its column.
    fn clear_field_state(
        &self,
        collection: &str,
        field: &str,
        object: ObjectId,
    ) -> Result<(), Error> {
        self.conn.execute(
            &format!(
                "DELETE FROM {} WHERE space_id = ?1 AND object_id = ?2 AND field = ?3",
                self.states(collection)
            ),
            params![self.space.0, object.0, field],
        )?;
        if let Some(Field::Basic(_) | Field::Link(_)) = self.model.field(collection, field) {
            self.conn.execute(
                &format!(
                    "UPDATE {} SET {} = NULL WHERE space_id = ?1 AND object_id = ?2",
                    self.rows(collection),
                    column(field)
                ),
                params![self.space.0, object.0],
            )?;
        }
        self.drop_empty_row(collection, object)
    }

    /// Forgets the object's creates and deletes: it is no longer deleted,
    /// and a row only its create held goes.
    fn clear_object_state(&self, object: ObjectId) -> Result<(), Error> {
        let was_deleted = self.object(object)?.object().is_deleted();
        self.conn.execute(
            &format!(
                "DELETE FROM {} WHERE space_id = ?1 AND object_id = ?2",
                objects_in(self.prefix)
            ),
            params![self.space.0, object.0],
        )?;
        for collection in self.model.collections.keys() {
            if was_deleted {
                self.mark_deleted(collection, object, false)?;
            }
            self.drop_empty_row(collection, object)?;
        }
        Ok(())
    }

    /// Removes the object's row of `c_<collection>` when neither a field
    /// state of the collection nor a create into it holds it: [`Self::row`]
    /// makes a row only for those.
    fn drop_empty_row(&self, collection: &str, object: ObjectId) -> Result<(), Error> {
        let has_state: bool = self.conn.query_row(
            &format!(
                "SELECT EXISTS (SELECT 1 FROM {} WHERE space_id = ?1 AND object_id = ?2)",
                self.states(collection)
            ),
            params![self.space.0, object.0],
            |r| r.get(0),
        )?;
        if !has_state && !self.object(object)?.creates_in(collection) {
            self.conn.execute(
                &format!(
                    "DELETE FROM {} WHERE space_id = ?1 AND object_id = ?2",
                    self.rows(collection)
                ),
                params![self.space.0, object.0],
            )?;
        }
        Ok(())
    }
}

/// The merge state of a field, empty when nobody wrote it.
pub(super) fn load(
    conn: &Connection,
    prefix: &str,
    space: SpaceId,
    collection: &str,
    field_name: &str,
    object: ObjectId,
    field: &Field,
) -> Result<FieldState, Error> {
    let bytes: Option<Vec<u8>> = conn
        .query_row(
            &format!(
                "SELECT state FROM {} WHERE space_id = ?1 AND object_id = ?2 AND field = ?3",
                states_in(prefix, collection)
            ),
            params![space.0, object.0, field_name],
            |r| r.get(0),
        )
        .optional()?;
    Ok(match (bytes, field.empty()) {
        (None, empty) => empty,
        (Some(bytes), FieldState::Basic(_)) => FieldState::Basic(state::basic(&bytes)?),
        (Some(bytes), FieldState::Set(_)) => FieldState::Set(state::set(&bytes)?),
    })
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

/// The column value of a basic value's or a link's winning write: its
/// value as the SQLite type the field names, or NULL when no write is
/// live, the field is cleared, or the value is outside the reader's
/// schema (it stays readable in the merge state).
fn column_value(winner: Option<Entry>) -> Sql {
    match winner.filter(|entry| entry.known) {
        Some(entry) => sql_value(entry.value),
        None => Sql::Null,
    }
}

/// The column value of a reference's winning write: the target's space id
/// followed by its object id, 32 bytes, as stored and whatever its state
/// (RFC 0006, "Declared indexes"). NULL when no write is live, the reference is
/// cleared, or its value does not fit (RFC 0004, decision 17).
fn reference_column(winner: Option<Entry>) -> Sql {
    winner.map_or(Sql::Null, |entry| reference_sql(&entry.value))
}

/// A reference's value as the SQLite value its column holds: 32 bytes, or
/// NULL for a value that names no target.
pub(super) fn reference_sql(value: &Value) -> Sql {
    match reference::target(value) {
        Some(target) => Sql::Blob([target.space.0, target.object.0].concat()),
        None => Sql::Null,
    }
}

/// A basic value as the SQLite value its column holds: text as `TEXT`,
/// an integer and a boolean as `INTEGER`, a float as `REAL`, bytes (an
/// object id) as `BLOB`, anything else as NULL.
pub(super) fn sql_value(value: Value) -> Sql {
    match value {
        Value::Text(text) => Sql::Text(text),
        Value::Unsigned(n) => i64::try_from(n).map_or(Sql::Null, Sql::Integer),
        Value::Negative(n) => i64::try_from(n).map_or(Sql::Null, |n| Sql::Integer(-1 - n)),
        Value::Float(x) => Sql::Real(x),
        Value::Bool(b) => Sql::Integer(i64::from(b)),
        Value::Bytes(bytes) => Sql::Blob(bytes),
        Value::Null | Value::Array(_) | Value::Map(_) => Sql::Null,
    }
}

impl Fold for SqlFold<'_> {
    fn model_id(&self) -> [u8; 16] {
        self.model.id
    }

    fn knows_collection(&self, collection: &str) -> bool {
        self.model.collections.contains_key(collection)
    }

    fn knows_field(&self, collection: &str, field: &str) -> bool {
        self.model.field(collection, field).is_some()
    }

    fn fold(&mut self, origin: Origin, operation: &Operation) -> Result<(), OperationParkReason> {
        if self.error.is_some() {
            return Ok(());
        }
        match self.fold_field(origin, operation) {
            Ok(folded) => folded,
            Err(error) => {
                self.keep(Err(error));
                Ok(())
            }
        }
    }

    fn fold_object(&mut self, object: ObjectId, operation: &ObjectOperation) {
        if self.error.is_none() {
            let result = self.fold_object_operation(object, operation);
            self.keep(result);
        }
    }

    fn clear_field(&mut self, object: ObjectId, collection: &str, field: &str) {
        if self.error.is_none() && self.model.field(collection, field).is_some() {
            let result = self.clear_field_state(collection, field, object);
            self.keep(result);
        }
    }

    fn clear_object(&mut self, object: ObjectId) {
        if self.error.is_none() {
            let result = self.clear_object_state(object);
            self.keep(result);
        }
    }
}
