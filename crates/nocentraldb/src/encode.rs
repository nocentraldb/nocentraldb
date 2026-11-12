//! How a field's net write in a transaction becomes operations of RFC 0004
//! (RFC 0009, "Field operations per field type"): a basic value or a link
//! one set, which replaces the winner and every conflict the copy holds; a
//! set or many links one add or remove per element, an add replacing the
//! element's removals the copy holds and a remove naming the additions it
//! observed.

use ncdb_core::change::Operation;
use ncdb_core::clock::Hlc;
use ncdb_core::field_op::{self, Encoding};
use ncdb_core::ids::{
    ChangeId as CoreChangeId, MemberId, ObjectId as CoreObjectId, OperationId,
    SpaceId as CoreSpaceId,
};
use ncdb_core::keys::PersonKey;
use ncdb_core::link::Target;
use ncdb_core::personal::{self, Place, Sealed};
use ncdb_core::view::{Field, FieldState};

use crate::error::{Error, ErrorCode};
use crate::tx::Intent;
use crate::value::{Repr, Value};

/// Where a field operation goes, and who writes it.
pub(crate) struct Site<'a> {
    pub(crate) object: CoreObjectId,
    pub(crate) collection: &'a str,
    pub(crate) name: &'a str,
    pub(crate) space: CoreSpaceId,
    pub(crate) author: MemberId,
}

impl Site<'_> {
    /// The id an operation is made with before its change exists. An
    /// operation's id and stamp come from its change; its bytes hold
    /// neither.
    pub(crate) const ID: OperationId = OperationId {
        change: CoreChangeId([0; 32]),
        position: 0,
    };
    /// The stamp an operation is made with, as [`Site::ID`].
    pub(crate) const STAMP: Hlc = Hlc(0);

    /// The operations that write `intent` to the field, against `state`,
    /// the copy's state of the field; a personal value sealed under `seal`,
    /// a person key and its id (RFC 0009, "Personal fields").
    pub(crate) fn encode(
        &self,
        field: &Field,
        state: &FieldState,
        intent: &Intent,
        seal: Option<(&PersonKey, [u8; 16])>,
    ) -> Result<Vec<Operation>, Error> {
        let (id, author, stamp) = (Site::ID, self.author, Site::STAMP);
        let invalid = || self.invalid();
        match (field, state, intent) {
            (Field::Basic(basic), FieldState::Basic(state), Intent::Set(value)) => {
                let value = value.to_cbor().ok_or_else(invalid)?;
                let mut write = basic
                    .set(state, id, author, stamp, value)
                    .map_err(|_| invalid())?;
                if let Some((key, person_key)) = seal {
                    let place = Place {
                        space: self.space,
                        object: self.object,
                        person_key,
                        collection: self.collection,
                        field: self.name,
                    };
                    let bytes = personal::seal(key, &place, &write.value);
                    write.value = Sealed { person_key, bytes }.to_value();
                }
                Ok(vec![self.operation(basic, &write)])
            }
            (Field::Link(link), FieldState::Basic(state), Intent::Set(value)) => {
                let target = Some(self.target(value)?);
                let write = link
                    .set(state, id, author, stamp, self.space, target)
                    .map_err(|_| invalid())?;
                Ok(vec![self.operation(link, &write)])
            }
            (Field::Set(set), FieldState::Set(state), Intent::Elements(actions)) => actions
                .iter()
                .map(|(element, add)| {
                    let element = element.to_cbor().ok_or_else(invalid)?;
                    let edit = match add {
                        true => set.add(state, id, author, stamp, element),
                        false => set.remove(state, id, author, stamp, element),
                    };
                    Ok(self.operation(set, &edit.map_err(|_| invalid())?))
                })
                .collect(),
            (Field::Links(links), FieldState::Set(state), Intent::Elements(actions)) => actions
                .iter()
                .map(|(element, add)| {
                    let target = self.target(element)?;
                    let edit = match add {
                        true => links
                            .add(state, id, author, stamp, self.space, target)
                            .map_err(|_| invalid())?,
                        false => links.remove(state, id, author, stamp, target.object),
                    };
                    Ok(self.operation(links, &edit))
                })
                .collect(),
            _ => Err(invalid()),
        }
    }

    /// The object in this space that `value`, an object id, names.
    fn target(&self, value: &Value) -> Result<Target, Error> {
        match &value.0 {
            Repr::Id(target) => Ok(Target {
                space: self.space,
                object: target.0,
            }),
            _ => Err(self.invalid()),
        }
    }

    fn invalid(&self) -> Error {
        Error::new(
            ErrorCode::InvalidValue,
            format!(
                "the value does not fit the field {} of {}",
                self.name, self.collection
            ),
        )
    }

    fn operation<T: Encoding>(&self, field: &T, op: &T::Operation) -> Operation {
        field_op::encode(field, op, self.object, self.collection, self.name)
    }
}
