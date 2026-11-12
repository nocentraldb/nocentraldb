//! A transaction (RFC 0009, "One write, one change"): the writes an app
//! records in one [`crate::Space::write`] call, and the operations of RFC
//! 0004 they become in the change. An operation cannot name another of its
//! own change, so the change carries the net effect: a field written twice
//! keeps its last value, and an element added and removed keeps its last
//! action.

use std::collections::BTreeMap;

use ncdb_core::basic::{BasicValue, Kind};
use ncdb_core::cbor;
use ncdb_core::change::Operation;
use ncdb_core::field::FieldType;
use ncdb_core::field_op;
use ncdb_core::ids::{MemberId, ObjectId as CoreObjectId, SpaceId as CoreSpaceId};
use ncdb_core::model_lock::Person as Whom;
use ncdb_core::personal::{PERSON_FIELD, PersonField};
use ncdb_core::view::{Field, FieldState, Model, View};

use crate::encode::Site;
use crate::error::{Error, ErrorCode, storage};
use crate::id::ObjectId;
use crate::object::Object;
use crate::personal::Persons;
use crate::value::{Repr, Value};

/// The options of [`Transaction::create_with`] (RFC 0009, "Personal
/// fields").
#[derive(Clone, Debug, Default)]
pub struct Create {
    /// An object of the same person, read from another space: the new
    /// object keeps its person id, so an erasure of the person reaches
    /// both (RFC 0005, "Identifying the person").
    pub same_person_as: Option<Object>,
}

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
    /// A create, with the person id of the person it is the same as.
    Create(String, CoreObjectId, Vec<(String, Value)>, Option<[u8; 16]>),
    Update(String, CoreObjectId, Vec<(String, Value)>),
    Delete(String, CoreObjectId),
}

/// What one field of one object becomes in the change.
pub(crate) enum Intent {
    /// A basic value or a link: the last value written.
    Set(Value),
    /// A set or many links: each element with whether it is added, by its
    /// last action.
    Elements(Vec<(Value, bool)>),
}

impl Intent {
    /// The first write of the field.
    fn new(value: Value) -> Intent {
        let mut intent = Intent::Elements(Vec::new());
        intent.write(value);
        intent
    }

    /// Takes in one more write of the field.
    fn write(&mut self, value: Value) {
        let (elements, add) = match value.0 {
            Repr::Add(elements) => (elements, true),
            Repr::Remove(elements) => (elements, false),
            _ => return *self = Intent::Set(value),
        };
        if let Intent::Set(_) = self {
            *self = Intent::Elements(Vec::new());
        }
        let Intent::Elements(actions) = self else {
            unreachable!("made a list of elements above");
        };
        for element in elements {
            match actions.iter_mut().find(|(e, _)| *e == element) {
                Some(action) => action.1 = add,
                None => actions.push((element, add)),
            }
        }
    }
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
        self.create_with(collection, values, Create::default())
    }

    /// Records the creation of an object as [`Transaction::create`] does,
    /// with `options`: `same_person_as` names the same person read from
    /// another space (RFC 0009, "Personal fields").
    pub fn create_with<'v>(
        &mut self,
        collection: &str,
        values: impl IntoIterator<Item = (&'v str, Value)>,
        options: Create,
    ) -> Result<ObjectId, Error> {
        let values = self.fields(collection, values)?;
        let id = CoreObjectId::random();
        let same_as = options.same_person_as.and_then(|object| object.person_id());
        self.records
            .push(Record::Create(collection.to_owned(), id, values, same_as));
        Ok(ObjectId(id))
    }

    /// Records a write of `values` to the object `id` of `collection`. The
    /// write fails with `NotFound` when the copy holds no such object and
    /// this transaction does not create it.
    pub fn update<'v>(
        &mut self,
        collection: &str,
        id: ObjectId,
        values: impl IntoIterator<Item = (&'v str, Value)>,
    ) -> Result<(), Error> {
        let values = self.fields(collection, values)?;
        self.records
            .push(Record::Update(collection.to_owned(), id.0, values));
        Ok(())
    }

    /// Records the deletion of the object `id` of `collection`, which
    /// fails as an update does when the copy holds no such object.
    pub fn delete(&mut self, collection: &str, id: ObjectId) -> Result<(), Error> {
        self.fields(collection, [])?;
        self.records
            .push(Record::Delete(collection.to_owned(), id.0));
        Ok(())
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
            .map(|(name, value)| {
                if let Some((type_id, rule)) = self.model.unregistered_type(collection, name) {
                    return Err(Error::new(
                        ErrorCode::UnknownType,
                        format!(
                            "the field {name} of {collection} has the plugin type {type_id}, \
                             rule {rule}, which no plugin given to OpenOptions::plugin implements"
                        ),
                    ));
                }
                match fields.contains_key(name) && !name.starts_with('$') {
                    true => Ok((name.to_owned(), value)),
                    false => Err(Error::new(
                        ErrorCode::NotFound,
                        format!("the collection {collection} has no field {name}"),
                    )),
                }
            })
            .collect()
    }

    /// The operations of the change: the creates and deletes in the order
    /// the transaction recorded them, then each field written, in the order
    /// it was first written; and the ids of the objects it created. A value
    /// a field does not take is refused with `InvalidValue`.
    pub(crate) fn operations(
        self,
        view: &View,
        space: CoreSpaceId,
        author: MemberId,
        persons: &mut Persons<'_>,
    ) -> Result<(Vec<Operation>, Vec<ObjectId>), Error> {
        let mut operations = Vec::new();
        let mut created: Vec<(CoreObjectId, String)> = Vec::new();
        let mut same_as: BTreeMap<CoreObjectId, [u8; 16]> = BTreeMap::new();
        let mut intents: BTreeMap<(CoreObjectId, String), (String, Intent)> = BTreeMap::new();
        let mut order = Vec::new();
        for record in self.records {
            let (collection, id, values) = match record {
                Record::Create(collection, id, values, person) => {
                    operations.push(field_op::create(id, &collection));
                    created.push((id, collection.clone()));
                    if let Some(person) = person {
                        same_as.insert(id, person);
                    }
                    (collection, id, values)
                }
                Record::Update(collection, id, values) => {
                    held(view, space, &created, &collection, id)?;
                    (collection, id, values)
                }
                Record::Delete(collection, id) => {
                    let deletion = held(view, space, &created, &collection, id)?;
                    let write = BasicValue {
                        kind: Kind::Boolean,
                    }
                    .set(
                        &deletion,
                        Site::ID,
                        author,
                        Site::STAMP,
                        cbor::Value::Bool(true),
                    )
                    .expect("a boolean is a deletion's value");
                    operations.push(field_op::delete(id, &collection, &write));
                    continue;
                }
            };
            for (name, value) in values {
                let key = (id, name);
                match intents.get_mut(&key) {
                    Some((_, intent)) => intent.write(value),
                    None => {
                        order.push(key.clone());
                        intents.insert(key, (collection.clone(), Intent::new(value)));
                    }
                }
            }
        }
        // The person each personal value is sealed for.
        let mut sealed_for: BTreeMap<(CoreObjectId, String), [u8; 16]> = BTreeMap::new();
        for key in &order {
            let (collection, intent) = &intents[key];
            let (object, name) = key;
            let (Some(whom), Intent::Set(_)) = (self.model.person(collection, name), intent) else {
                continue;
            };
            let (person, of) = match whom {
                Whom::Itself => (*object, collection.clone()),
                Whom::Link(link) => person_link(
                    self.model, view, space, &created, &intents, *object, collection, link,
                )?,
            };
            let id = persons.person_key(view, space, person, &of, same_as.get(&person).copied())?;
            sealed_for.insert(key.clone(), id);
        }
        // `$person` of each person this write drew, in the same change as
        // its first value (RFC 0005, "Where the person id is kept").
        for drawn in &persons.drawn {
            let write = PersonField.set(
                &Default::default(),
                Site::ID,
                author,
                Site::STAMP,
                Some(drawn.person),
            );
            operations.push(field_op::encode(
                &PersonField,
                &write,
                drawn.object,
                &drawn.collection,
                PERSON_FIELD,
            ));
        }
        for key in order {
            let (collection, intent) = &intents[&key];
            let (object, name) = &key;
            let field = self
                .model
                .field(collection, name)
                .expect("a recorded field is in the model");
            let state = match view
                .get(space, collection, *object)
                .map_err(storage)?
                .and_then(|mut row| row.fields.remove(name))
            {
                Some(state) => state,
                None => field.empty(),
            };
            let site = Site {
                object: *object,
                collection,
                name,
                space,
                author,
            };
            let seal = sealed_for.get(&key).map(|id| (persons.key(*id), *id));
            operations.extend(site.encode(field, &state, intent, seal)?);
        }
        let ids = created.into_iter().map(|(id, _)| ObjectId(id)).collect();
        Ok((operations, ids))
    }
}

/// The target of the person link `link` of `object`, and its
/// collection, as the copy holds it after this transaction's own
/// writes; `NoPerson` when the link is unset or names no object the
/// copy or the transaction holds.
#[allow(clippy::too_many_arguments)]
fn person_link(
    model: &Model,
    view: &View,
    space: CoreSpaceId,
    created: &[(CoreObjectId, String)],
    intents: &BTreeMap<(CoreObjectId, String), (String, Intent)>,
    object: CoreObjectId,
    collection: &str,
    link: &str,
) -> Result<(CoreObjectId, String), Error> {
    let no_person = || {
        Error::new(
            ErrorCode::NoPerson,
            format!("the person link {link} of {collection} is unset"),
        )
    };
    let target = match intents.get(&(object, link.to_owned())) {
        Some((_, Intent::Set(Value(Repr::Id(target))))) => target.0,
        Some(_) => return Err(no_person()),
        None => {
            let Some(Field::Link(field)) = model.field(collection, link) else {
                return Err(no_person());
            };
            let state = view
                .get(space, collection, object)
                .map_err(storage)?
                .and_then(|mut row| row.fields.remove(link));
            let Some(FieldState::Basic(state)) = state else {
                return Err(no_person());
            };
            match field
                .read(&state)
                .value
                .filter(|e| e.known)
                .map(|e| e.value)
            {
                Some(cbor::Value::Bytes(bytes)) => {
                    CoreObjectId(bytes.try_into().map_err(|_| no_person())?)
                }
                _ => return Err(no_person()),
            }
        }
    };
    let Some(Field::Link(field)) = model.field(collection, link) else {
        return Err(no_person());
    };
    if let Some((_, of)) = created.iter().find(|(id, _)| *id == target) {
        return Ok((target, of.clone()));
    }
    for of in &field.collections {
        if held(view, space, &[], of, target).is_ok() {
            return Ok((target, of.clone()));
        }
    }
    Err(no_person())
}

/// The state of the deletion of `object`, which the copy holds in
/// `collection` or this transaction created there; `NotFound` otherwise.
fn held(
    view: &View,
    space: CoreSpaceId,
    created: &[(CoreObjectId, String)],
    collection: &str,
    object: CoreObjectId,
) -> Result<ncdb_core::basic::State, Error> {
    let not_found = || {
        Error::new(
            ErrorCode::NotFound,
            format!(
                "the copy holds no object {} in {collection}",
                ObjectId(object)
            ),
        )
    };
    let row = view.get(space, collection, object).map_err(storage)?;
    match row {
        Some(row)
            if row.object.collection().value.map(|e| e.value)
                == Some(cbor::Value::Text(collection.to_owned())) =>
        {
            Ok(row.object.deletion().clone())
        }
        _ if created.contains(&(object, collection.to_owned())) => Ok(Default::default()),
        _ => Err(not_found()),
    }
}
