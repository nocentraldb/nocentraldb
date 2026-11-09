//! The fold into the view's tables (RFC 0006, "One change", step 3): the
//! [`Fold`] that [`Log::receive`](crate::log::Log) calls for each operation
//! of a change, inside the transaction that appends it.

use rusqlite::Connection;

use super::Error;
use super::model::Model;
use crate::change::Operation;
use crate::field_op::Origin;
use crate::ids::{ObjectId, SpaceId};
use crate::log::OperationParkReason;
use crate::object::ObjectOperation;
use crate::receive::Fold;

/// The fold of one change's operations into the tables of a space's
/// collections, on the connection of the transaction that appends it.
pub(super) struct SqlFold<'a> {
    conn: &'a Connection,
    model: &'a Model,
    space: SpaceId,
    /// The first SQLite error; the fold's calls cannot return one.
    error: Option<rusqlite::Error>,
}

impl<'a> SqlFold<'a> {
    pub(super) fn new(conn: &'a Connection, model: &'a Model, space: SpaceId) -> SqlFold<'a> {
        SqlFold {
            conn,
            model,
            space,
            error: None,
        }
    }

    /// The first SQLite error the fold met, if any: the transaction must
    /// then roll back.
    pub(super) fn finish(self) -> Result<(), Error> {
        match self.error {
            Some(error) => Err(error.into()),
            None => Ok(()),
        }
    }
}

impl Fold for SqlFold<'_> {
    fn knows_collection(&self, collection: &str) -> bool {
        self.model.collections.contains_key(collection)
    }

    fn knows_field(&self, collection: &str, field: &str) -> bool {
        self.model.field(collection, field).is_some()
    }

    fn fold(&mut self, _origin: Origin, _operation: &Operation) -> Result<(), OperationParkReason> {
        let _ = (self.conn, self.space);
        Ok(())
    }

    fn fold_object(&mut self, _object: ObjectId, _operation: &ObjectOperation) {}
}
