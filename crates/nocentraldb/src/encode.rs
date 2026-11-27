//! How a field's net write in a transaction becomes operations of RFC 0004
//! (RFC 0009, "Field operations per field type"): a basic value, a link or
//! a reference one set, which replaces the winner and every conflict the
//! copy holds; a set or many links one add or remove per element, an add
//! replacing the element's removals the copy holds and a remove naming the
//! additions it observed.

use ncdb_core::cbor;
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
use ncdb_core::view::{Field, FieldState, PluginField};
use ncdb_plugin::{DataError, WriteError};

/// The first operation number of plugin types (RFC 0004, "Operation
/// numbers").
const FIRST_PLUGIN_OPERATION: u64 = 64;

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
                if !value.fits(&basic.kind) {
                    return Err(invalid());
                }
                let value = match value.0 {
                    Repr::Clear => cbor::Value::Null,
                    _ => value.to_cbor().ok_or_else(invalid)?,
                };
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
                // A link takes a reference too, and refuses one into another
                // space (RFC 0018, "Existing link fields read as references").
                let target = match &value.0 {
                    Repr::Clear => None,
                    Repr::Ref(reference) => Some(Target {
                        space: reference.space.0,
                        object: reference.id.0,
                    }),
                    _ => Some(self.target(value)?),
                };
                let write = link
                    .set(state, id, author, stamp, self.space, target)
                    .map_err(|_| invalid())?;
                Ok(vec![self.operation(link, &write)])
            }
            (Field::Ref(reference), FieldState::Basic(state), Intent::Set(value)) => {
                // A bare object id names the written object's own space
                // (RFC 0018, "One function, two cases").
                let target = match value.0 {
                    Repr::Clear => None,
                    Repr::Ref(reference) => Some(Target {
                        space: reference.space.0,
                        object: reference.id.0,
                    }),
                    Repr::Id(_) => Some(self.target(value)?),
                    _ => return Err(invalid()),
                };
                let write = reference.set(state, id, author, stamp, target);
                Ok(vec![self.operation(reference, &write)])
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
            (Field::Plugin(plugin), FieldState::Plugin(state), Intent::Set(value)) => {
                self.plugin(plugin, state, value)
            }
            #[cfg(all(ncdb_rich_text, any(test, feature = "bridge")))]
            (
                Field::RichText(rich_text),
                FieldState::RichText(state),
                Intent::Set(Value(Repr::RichText(update))),
            ) => self
                .rich_text(
                    rich_text,
                    state,
                    &update.update,
                    update.client,
                    update.nonce,
                )
                .map(|op| vec![op]),
            #[cfg(ncdb_rich_text)]
            (
                Field::RichText(rich_text),
                FieldState::RichText(state),
                Intent::Set(Value(Repr::Markdown(markdown))),
            ) => self
                .first_document(rich_text, state, markdown)
                .map(|op| vec![op]),
            _ => Err(invalid()),
        }
    }

    /// The operations a plugin's `write` makes of `value`, a `Data`
    /// intent, against `state` (RFC 0011): each a number of 64 or more and
    /// its arguments, keyed by number. An intent the plugin refuses is
    /// `InvalidValue` with the plugin's reason; a value that is no `Data`,
    /// or an operation below 64, is `InvalidValue` too.
    fn plugin(
        &self,
        plugin: &PluginField,
        state: &[u8],
        value: &Value,
    ) -> Result<Vec<Operation>, Error> {
        let Repr::Data(intent) = &value.0 else {
            return Err(self.invalid());
        };
        let written = plugin.write(state, intent).map_err(|error| match error {
            WriteError::Invalid(why) => Error::new(
                ErrorCode::InvalidValue,
                crate::texts::error::value_does_not_fit_because(self.name, self.collection, why),
            ),
            WriteError::Corrupt => Error::new(
                ErrorCode::StorageFailed,
                crate::texts::error::PLUGIN_STATE_UNREADABLE,
            ),
        })?;
        written
            .into_iter()
            .map(|(number, arguments)| {
                if number < FIRST_PLUGIN_OPERATION {
                    return Err(self.invalid());
                }
                let arguments = arguments
                    .into_iter()
                    .map(|(key, data)| Ok((cbor::Key::Unsigned(key), data.to_value()?)))
                    .collect::<Result<_, DataError>>()
                    .map_err(|_| self.invalid())?;
                Ok(Operation::new(
                    self.object,
                    self.collection.to_owned(),
                    Some(self.name.to_owned()),
                    number,
                    Some(cbor::Value::Map(arguments)),
                ))
            })
            .collect()
    }

    /// Operation 5 with `update` (RFC 0013, "How an edit travels in a
    /// change"), listing the leased client id when the update adds items.
    /// The change is checked before it is made, as every member folds it:
    /// an update Yjs cannot read, one that adds items under another client
    /// id, one Yrs refuses and one that gives an id the state holds other
    /// content are `InvalidValue`, so the log never holds a write of this
    /// member that parks.
    #[cfg(ncdb_rich_text)]
    fn rich_text(
        &self,
        field: &ncdb_core::rich_text::RichText,
        state: &ncdb_core::rich_text::State,
        update: &[u8],
        client: u64,
        nonce: [u8; 8],
    ) -> Result<Operation, Error> {
        use ncdb_core::field_op::Origin;
        use ncdb_core::rich_text::{Fit, UPDATE};
        let shape = ncdb_core::rich_text::update::read(update).map_err(|_| self.invalid())?;
        let clients = match shape.runs.is_empty() {
            true => Vec::new(),
            false => vec![(
                cbor::Key::Unsigned(client),
                cbor::Value::Bytes(nonce.to_vec()),
            )],
        };
        let arguments = cbor::Value::Map(
            [
                (cbor::Key::Unsigned(1), cbor::Value::Bytes(update.to_vec())),
                (
                    cbor::Key::Unsigned(2),
                    cbor::Value::Map(clients.into_iter().collect()),
                ),
            ]
            .into(),
        );
        let operation = Operation::new(
            self.object,
            self.collection.to_owned(),
            Some(self.name.to_owned()),
            UPDATE,
            Some(arguments),
        );
        let origin = Origin {
            id: Site::ID,
            author: self.author,
            stamp: Site::STAMP,
        };
        let edit = field
            .decode(&operation, origin)
            .map_err(|_| self.invalid())?;
        if field.fit(state, &edit) != Fit::Fits {
            return Err(self.invalid());
        }
        let mut folded = state.clone();
        field
            .try_apply(&mut folded, &edit)
            .map_err(|_| self.invalid())?;
        Ok(operation)
    }

    /// Operation 5 with a first document read from `markdown` (RFC 0013,
    /// "The first content"), written under a client id of the author drawn
    /// for this write, so no item of it can share an id with another. A
    /// field that holds content is `RichTextNotEmpty`; Markdown the core
    /// does not write a document from is `InvalidValue`.
    #[cfg(ncdb_rich_text)]
    fn first_document(
        &self,
        field: &ncdb_core::rich_text::RichText,
        state: &ncdb_core::rich_text::State,
        markdown: &str,
    ) -> Result<Operation, Error> {
        if state.holds_content() {
            return Err(Error::new(
                ErrorCode::RichTextNotEmpty,
                crate::texts::error::rich_text_not_empty(self.name, self.collection),
            ));
        }
        let mut nonce = [0u8; 8];
        getrandom::fill(&mut nonce).expect("the platform's secure random generator failed");
        let client = ncdb_core::rich_text::client_id(&self.author, &nonce);
        let update = ncdb_core::rich_text::anchor::from_markdown(markdown, client)
            .ok_or_else(|| self.invalid())?;
        self.rich_text(field, state, &update, client, nonce)
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
            crate::texts::error::value_does_not_fit(self.name, self.collection),
        )
    }

    fn operation<T: Encoding>(&self, field: &T, op: &T::Operation) -> Operation {
        field_op::encode(field, op, self.object, self.collection, self.name)
    }
}
