//! The trace of a moved object in the view (RFC 0016, "What the members
//! of the old space keep"): the object leaves every declared index, and
//! the view keeps no plaintext of its fields, neither read values nor
//! merge states, as for an erased field (RFC 0006, "What the view stores
//! in plaintext"). A read by id gives the object's state, which says where
//! it moved, and no fields.
//!
//! The fold drops the fields when the object becomes a trace and keeps
//! nothing of a field operation that arrives while it is one. When the
//! move is taken back, the fold names the object in
//! [`Fold::take_revived`](crate::receive::Fold::take_revived), and the
//! log's fold takes the object's field operations again from the changes
//! still folded. Each field's state is a function of its set of
//! operations, so the view then holds what a copy that never saw the move
//! holds, in any order of arrival.

use rusqlite::params;

use super::Error;
use super::fold::SqlFold;
use super::schema::column;
use super::state::ObjectState;
use crate::ids::ObjectId;

/// Whether an object is out of the declared indexes: deleted or moved.
pub(super) fn out_of_indexes(state: &ObjectState) -> bool {
    let object = state.object();
    object.is_deleted() || object.moved_to().is_some()
}

impl SqlFold<'_> {
    /// Drops every field state of `object` and empties every column of its
    /// rows, in every collection, and the rows only a field state held: the
    /// object has become a trace.
    pub(super) fn drop_fields(&self, object: ObjectId) -> Result<(), Error> {
        for (collection, fields) in &self.model.collections {
            self.conn.execute(
                &format!(
                    "DELETE FROM {} WHERE space_id = ?1 AND object_id = ?2",
                    self.states(collection)
                ),
                params![self.space.0, object.0],
            )?;
            let columns: Vec<String> = fields
                .iter()
                .filter(|(_, field)| field.has_column())
                .map(|(name, _)| format!("{} = NULL", column(name)))
                .collect();
            if !columns.is_empty() {
                self.conn.execute(
                    &format!(
                        "UPDATE {} SET {} WHERE space_id = ?1 AND object_id = ?2",
                        self.rows(collection),
                        columns.join(", ")
                    ),
                    params![self.space.0, object.0],
                )?;
            }
            self.drop_empty_row(collection, object)?;
        }
        Ok(())
    }
}
