//! A transaction (RFC 0009, "One write, one change"): the writes an app
//! records in one [`crate::Space::write`] call, and the operations of RFC
//! 0004 they become in the change. An operation cannot name another of its
//! own change, so a field written twice in one transaction is written once,
//! with its last value: the net effect.

use std::collections::BTreeMap;

use ncdb_core::change::Operation;
use ncdb_core::clock::Hlc;
use ncdb_core::field_op;
use ncdb_core::ids::{
    ChangeId as CoreChangeId, MemberId, ObjectId as CoreObjectId, OperationId,
    SpaceId as CoreSpaceId,
};
use ncdb_core::link::Target;
use ncdb_core::view::{Field, FieldState, Model, View};

use crate::error::{Error, ErrorCode, storage};
use crate::id::ObjectId;
use crate::value::{Repr, Value};

/// The writes recorded in one [`crate::Space::write`] call. Its `Debug`
/// output counts them and shows no value.
pub struct Transaction<'a> {
    model: &'a Model,
    records: Vec<Record>,
}

impl std::fmt::Debug for Transaction<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Transaction")
            .field("records", &self.records.len())
            .finish()
    }
}

/// One recorded write.
enum Record {
    Create {
        collection: String,
        id: CoreObjectId,
        values: Vec<(String, Value)>,
    },
}

/// What a field of one object becomes in the change, after every write of
/// the transaction.
struct FieldWrite {
    collection: String,
    value: Value,
}

impl<'a> Transaction<'a> {
    pub(crate) fn new(model: &'a Model) -> Transaction<'a> {
        Transaction {
            model,
            records: Vec::new(),
        }
    }

    /// Records the creation of an object in `collection` with `values`,
    /// and returns its new id: 128 random bits drawn on the device.
    pub fn create<'v>(
        &mut self,
        collection: &str,
        values: impl IntoIterator<Item = (&'v str, Value)>,
    ) -> Result<ObjectId, Error> {
        let values = self.fields(collection, values)?;
        let id = CoreObjectId::random();
        self.records.push(Record::Create {
            collection: collection.to_owned(),
            id,
            values,
        });
        Ok(ObjectId(id))
    }

    /// The values, each for a field `collection` has.
    fn fields<'v>(
        &self,
        collection: &str,
        values: impl IntoIterator<Item = (&'v str, Value)>,
    ) -> Result<Vec<(String, Value)>, Error> {
        let fields = self.model.collections.get(collection).ok_or_else(|| {
            Error::new(
                ErrorCode::NotFound,
                format!("the model has no collection {collection}"),
            )
        })?;
        values
            .into_iter()
            .map(|(name, value)| match fields.contains_key(name) {
                true => Ok((name.to_owned(), value)),
                false => Err(Error::new(
                    ErrorCode::NotFound,
                    format!("the collection {collection} has no field {name}"),
                )),
            })
            .collect()
    }

    /// The operations of the change, in the order the transaction recorded
    /// them, and the ids of the objects it created. A value a field does
    /// not take is refused with `InvalidValue`.
    pub(crate) fn operations(
        self,
        view: &View,
        space: CoreSpaceId,
        author: MemberId,
    ) -> Result<(Vec<Operation>, Vec<ObjectId>), Error> {
        let mut objects = Vec::new();
        let mut created = Vec::new();
        let mut fields: BTreeMap<(CoreObjectId, String), FieldWrite> = BTreeMap::new();
        let mut order = Vec::new();
        for record in self.records {
            match record {
                Record::Create {
                    collection,
                    id,
                    values,
                } => {
                    objects.push(field_op::create(id, &collection));
                    created.push(ObjectId(id));
                    for (name, value) in values {
                        let key = (id, name);
                        if !fields.contains_key(&key) {
                            order.push(key.clone());
                        }
                        let collection = collection.clone();
                        fields.insert(key, FieldWrite { collection, value });
                    }
                }
            }
        }
        let mut operations = objects;
        for key in order {
            let write = &fields[&key];
            let (object, name) = &key;
            let field = self
                .model
                .field(&write.collection, name)
                .expect("a recorded field is in the model");
            let state = match view
                .get(space, &write.collection, *object)
                .map_err(storage)?
                .and_then(|mut row| row.fields.remove(name))
            {
                Some(state) => state,
                None => field.empty(),
            };
            let at = Site {
                object: *object,
                collection: &write.collection,
                name,
                space,
                author,
            };
            operations.push(at.encode(field, &state, &write.value)?);
        }
        Ok((operations, created))
    }
}

/// Where a field operation goes, and who writes it.
struct Site<'a> {
    object: CoreObjectId,
    collection: &'a str,
    name: &'a str,
    space: CoreSpaceId,
    author: MemberId,
}

impl Site<'_> {
    /// The operation that writes `value` to the field, replacing what
    /// `state`, the copy's state of the field, holds.
    fn encode(&self, field: &Field, state: &FieldState, value: &Value) -> Result<Operation, Error> {
        // The operation's id, author and stamp come from the change; they
        // are not part of the operation's bytes.
        let id = OperationId {
            change: CoreChangeId([0; 32]),
            position: 0,
        };
        let stamp = Hlc(0);
        let invalid = || {
            Error::new(
                ErrorCode::InvalidValue,
                format!(
                    "the value does not fit the field {} of {}",
                    self.name, self.collection
                ),
            )
        };
        match (field, state) {
            (Field::Basic(basic), FieldState::Basic(state)) => {
                let write = basic
                    .set(state, id, self.author, stamp, value.to_cbor())
                    .map_err(|_| invalid())?;
                Ok(self.operation(basic, &write))
            }
            (Field::Link(link), FieldState::Basic(state)) => {
                let Repr::Id(target) = &value.0 else {
                    return Err(invalid());
                };
                let target = Target {
                    space: self.space,
                    object: target.0,
                };
                let write = link
                    .set(state, id, self.author, stamp, self.space, Some(target))
                    .map_err(|_| invalid())?;
                Ok(self.operation(link, &write))
            }
            _ => Err(invalid()),
        }
    }

    fn operation<T: field_op::Encoding>(&self, field: &T, op: &T::Operation) -> Operation {
        field_op::encode(field, op, self.object, self.collection, self.name)
    }
}
