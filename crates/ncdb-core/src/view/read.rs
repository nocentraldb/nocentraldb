//! Reading the view (RFC 0006): an object by its id. A read runs against
//! the database on the device only (RFC 0006, "Reads never touch the
//! network").

use std::collections::BTreeMap;

use rusqlite::{OptionalExtension, params};

use super::model::{Field, FieldState};
use super::schema::states;
use super::state::{self, ObjectState};
use super::{Error, View};
use crate::ids::{ObjectId, SpaceId};
use crate::object::Object;

/// What the view holds of one object in one collection: the object, with
/// its creates and its deletion, and the merge state of each field of the
/// collection that was written. A field type reads its value from the
/// state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// The object.
    pub object: Object,
    /// The fields written, by name.
    pub fields: BTreeMap<String, FieldState>,
}

impl View {
    /// Reads `object` in `collection` of `space`, or `None` when the view
    /// holds no create, delete or field of the collection for it.
    pub fn get(
        &self,
        space: SpaceId,
        collection: &str,
        object: ObjectId,
    ) -> Result<Option<Row>, Error> {
        let Some(model) = self.model.collections.get(collection) else {
            return Ok(None);
        };
        let known: Option<Vec<u8>> = self
            .conn
            .query_row(
                "SELECT state FROM objects WHERE space_id = ?1 AND object_id = ?2",
                params![space.0, object.0],
                |r| r.get(0),
            )
            .optional()?;
        let mut statement = self.conn.prepare(&format!(
            "SELECT field, state FROM {} WHERE space_id = ?1 AND object_id = ?2",
            states(collection)
        ))?;
        let mut rows = statement.query(params![space.0, object.0])?;
        let mut fields = BTreeMap::new();
        while let Some(row) = rows.next()? {
            let name: String = row.get(0)?;
            let bytes: Vec<u8> = row.get(1)?;
            let state = match model.get(&name).ok_or(Error::Corrupt("field"))? {
                Field::Basic(_) | Field::Link(_) | Field::Ref(_) => {
                    FieldState::Basic(state::basic(&bytes)?)
                }
                Field::Set(_) | Field::Links(_) => FieldState::Set(state::set(&bytes)?),
            };
            fields.insert(name, state);
        }
        if known.is_none() && fields.is_empty() {
            return Ok(None);
        }
        let object = match known {
            Some(bytes) => ObjectState::read(&bytes)?.object(),
            None => Object::default(),
        };
        Ok(Some(Row { object, fields }))
    }
}
