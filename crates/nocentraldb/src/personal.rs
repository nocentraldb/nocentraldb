//! Whose key a personal field is sealed under (RFC 0009, "Personal
//! fields"; RFC 0005, "Identifying the person"; Carsten, 2026-11-12,
//! NOC-244). A write of a personal field reads `$person` on its person's
//! object: the object itself for `"self"`, or the target of its person
//! link, as the copy holds it after the transaction's own writes. A person
//! with no `$person` yet gets one: a person id, drawn or carried over with
//! `same_person_as`, and a new person key with a new id, which the write
//! announces in a person-key entry before its content.

use std::collections::BTreeMap;

use ncdb_core::field::FieldType;
use ncdb_core::ids::{ObjectId as CoreObjectId, SpaceId as CoreSpaceId};
use ncdb_core::keys::PersonKey;
use ncdb_core::personal::{PERSON_FIELD, Person, PersonField};
use ncdb_core::view::{FieldState, View};

use crate::db::SpaceState;
use crate::error::{Error, ErrorCode, storage};

/// The code of a write whose person has a key this device does not hold
/// yet, as after a reconnect before the person keys are fetched (Carsten,
/// 2026-11-12, NOC-255).
const KEY_NOT_HELD: ErrorCode = ErrorCode::PersonKeyNotHeld;

/// A person whose key this write draws.
pub(crate) struct Drawn {
    /// The person's object.
    pub(crate) object: CoreObjectId,
    /// Its collection.
    pub(crate) collection: String,
    /// The value of its new `$person`.
    pub(crate) person: Person,
    /// The new person key.
    pub(crate) key: PersonKey,
}

/// The persons of one write: the keys the member holds, and those the
/// write draws.
pub(crate) struct Persons<'a> {
    held: &'a BTreeMap<[u8; 16], PersonKey>,
    pub(crate) drawn: Vec<Drawn>,
}

impl<'a> Persons<'a> {
    pub(crate) fn new(held: &'a BTreeMap<[u8; 16], PersonKey>) -> Persons<'a> {
        Persons {
            held,
            drawn: Vec::new(),
        }
    }

    /// The person-key id of the person whose object is `object` of
    /// `collection`, drawing a person for it when it has none; with the
    /// person id `same_as` for a new one, when the app named the person.
    pub(crate) fn person_key(
        &mut self,
        view: &View,
        space: CoreSpaceId,
        object: CoreObjectId,
        collection: &str,
        same_as: Option<[u8; 16]>,
    ) -> Result<[u8; 16], Error> {
        if let Some(drawn) = self.drawn.iter().find(|d| d.object == object) {
            return Ok(drawn.person.person_key);
        }
        if let Some(person) = held_person(view, space, collection, object)? {
            if !self.held.contains_key(&person.person_key) {
                return Err(Error::new(
                    KEY_NOT_HELD,
                    "the person's key is not held on this device yet",
                ));
            }
            return Ok(person.person_key);
        }
        let person = Person {
            person: same_as.unwrap_or_else(|| CoreObjectId::random().0),
            person_key: CoreObjectId::random().0,
        };
        self.drawn.push(Drawn {
            object,
            collection: collection.to_owned(),
            person,
            key: PersonKey::generate(),
        });
        Ok(person.person_key)
    }

    /// The key of `person_key`, held or drawn by this write.
    pub(crate) fn key(&self, person_key: [u8; 16]) -> &PersonKey {
        self.drawn
            .iter()
            .find(|d| d.person.person_key == person_key)
            .map(|d| &d.key)
            .or_else(|| self.held.get(&person_key))
            .expect("a person key this write found or drew")
    }
}

/// The `$person` the copy holds on `object` of `collection`, if any.
pub(crate) fn held_person(
    view: &View,
    space: CoreSpaceId,
    collection: &str,
    object: CoreObjectId,
) -> Result<Option<Person>, Error> {
    let row = view.get(space, collection, object).map_err(storage)?;
    Ok(row
        .and_then(|mut row| row.fields.remove(PERSON_FIELD))
        .and_then(|state| match state {
            FieldState::Basic(state) => PersonField.read(&state).value,
            FieldState::Set(_) | FieldState::Plugin(_) => None,
        })
        .filter(|entry| entry.known)
        .and_then(|entry| Person::from_value(&entry.value)))
}

/// The person keys of `state` the view opens personal fields with: those
/// the member holds of persons that are not erased, provisionally or for
/// good (RFC 0009, "How a personal field reads").
pub(crate) fn open_keys(state: &SpaceState) -> BTreeMap<[u8; 16], PersonKey> {
    let resolved = state.checks.resolved();
    state
        .person_keys
        .iter()
        .filter(|(id, _)| resolved.person_key(**id).is_none_or(|key| !key.erased()))
        .map(|(id, key)| (*id, key.clone()))
        .collect()
}

/// Gives the view the person keys of `space` it opens personal fields
/// with, when they changed: after a fetch of the person keys, a shred, an
/// erasure decision or a write that drew a key (RFC 0006, "What the view
/// stores in plaintext": the fold erases an erased person's fields from
/// the view at once).
pub(crate) fn refresh(
    view: &mut View,
    space: CoreSpaceId,
    state: &mut SpaceState,
) -> Result<(), ncdb_core::view::Error> {
    let keys = open_keys(state);
    let ids: std::collections::BTreeSet<[u8; 16]> = keys.keys().copied().collect();
    if ids != state.opened {
        view.set_person_keys(space, keys)?;
        state.opened = ids;
    }
    Ok(())
}
