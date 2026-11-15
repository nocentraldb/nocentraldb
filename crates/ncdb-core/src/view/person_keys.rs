//! A person's person-key ids in a space, by person id (NOC-325; RFC 0003,
//! "The person key"; pm, NOC-325, on the coding agent's proposal). An
//! erasure finds the person's keys by person id in every space the account
//! reaches; the person id and the key's id are kept in the hidden field
//! `$person` of each object of the person (RFC 0005, "Where the person id
//! is kept"), a basic value that the view keeps only in its merge state,
//! since a map has no column. The lookup reads that state in each
//! collection whose objects can be a person.

use std::collections::BTreeSet;

use rusqlite::params;

use super::model::Field;
use super::schema::states_in;
use super::state;
use super::{Error, View};
use crate::field::FieldType;
use crate::ids::SpaceId;
use crate::personal::{PERSON_FIELD, Person, PersonField};

impl View {
    /// The ids of the person keys that a live `$person` in `space` names
    /// for the person id `person`: one per key the person has there,
    /// usually one, two when two devices drew a first key at once (RFC
    /// 0009, "Two first keys"). Empty when no object of the space is that
    /// person's.
    pub fn person_key_ids(
        &self,
        space: SpaceId,
        person: [u8; 16],
    ) -> Result<BTreeSet<[u8; 16]>, Error> {
        let mut found = BTreeSet::new();
        let persons = self
            .model
            .collections
            .iter()
            .filter(|(_, fields)| matches!(fields.get(PERSON_FIELD), Some(Field::Person(_))));
        for (collection, _) in persons {
            let mut select = self.conn.prepare(&format!(
                "SELECT state FROM {} WHERE space_id = ?1 AND field = ?2",
                states_in("", collection)
            ))?;
            let states = select
                .query_map(params![space.0, PERSON_FIELD], |row| {
                    row.get::<_, Vec<u8>>(0)
                })?
                .collect::<Result<Vec<_>, _>>()?;
            for bytes in states {
                let named = PersonField
                    .read(&state::basic(&bytes)?)
                    .value
                    .filter(|entry| entry.known)
                    .and_then(|entry| Person::from_value(&entry.value));
                if let Some(named) = named.filter(|named| named.person == person) {
                    found.insert(named.person_key);
                }
            }
        }
        Ok(found)
    }
}
