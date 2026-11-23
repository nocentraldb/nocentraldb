//! Un-folding in the view's tables (RFC 0006, "Un-folding"): forgetting
//! a field's state or an object's, and the row of an object nothing else
//! holds, before the changes still folded fold into them again.

use rusqlite::params;

use super::Error;
use super::fold::SqlFold;
use super::model::Field;
use super::schema::{column, objects_in};
use super::trace::out_of_indexes;
use crate::ids::ObjectId;

impl SqlFold<'_> {
    /// Forgets a field's state and empties its column.
    pub(super) fn clear_field_state(
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

    /// Forgets the object's creates, deletes and moves: it is no longer
    /// deleted or moved, and a row only its create held goes. A trace is
    /// revived, as by a take-back.
    pub(super) fn clear_object_state(&mut self, object: ObjectId) -> Result<(), Error> {
        let state = self.object(object)?;
        let was_out = out_of_indexes(&state);
        if state.object().moved_to().is_some() {
            self.revived.insert(object);
        }
        self.conn.execute(
            &format!(
                "DELETE FROM {} WHERE space_id = ?1 AND object_id = ?2",
                objects_in(self.prefix)
            ),
            params![self.space.0, object.0],
        )?;
        for collection in self.model.collections.keys() {
            if was_out {
                self.mark_deleted(collection, object, false)?;
            }
            self.drop_empty_row(collection, object)?;
        }
        Ok(())
    }

    /// Removes the object's row of `c_<collection>` when neither a field
    /// state of the collection nor a create into it holds it: [`Self::row`]
    /// makes a row only for those.
    pub(super) fn drop_empty_row(&self, collection: &str, object: ObjectId) -> Result<(), Error> {
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
