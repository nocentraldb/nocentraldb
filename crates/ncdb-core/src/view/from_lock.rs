//! The view's model read from the member's model lock (RFC 0005, "the core
//! and both SDKs read only the lock"): the lock's collections with their
//! stored fields, and each collection's declared indexes, in the lock's
//! order of names.
//!
//! A plugin field gets the code the registry holds for its type id and
//! rule, configured with its `config` (RFC 0011). One whose type and rule
//! the registry does not hold, or whose `config` the plugin refuses, is
//! left out of the view's fields and named as unregistered: the fold parks
//! its operations as "unknown type" and keeps them (RFC 0011, "A type that
//! is not registered").
//! Computed fields and views have no place in the view yet either.

use std::collections::BTreeMap;

use ncdb_plugin::Data;

use super::model::{Direction, Field, Index, Invalid, Model, PluginField};
use crate::basic::{BasicValue, Kind};
use crate::link::{Link, Links};
use crate::model_lock::{self, Element, ModelLock, Person, check};
use crate::personal::{PERSON_FIELD, PersonField};
use crate::plugins::Registry;
use crate::reference::Reference;
use crate::set::Set;

impl Model {
    /// The view's model of `lock`, with no plugin registered. A lock that
    /// breaks the rules RFC 0005 gives a lock on its own is refused with
    /// every rule it breaks.
    pub fn from_lock(lock: &ModelLock) -> Result<Model, Invalid> {
        Model::from_lock_with(lock, &Registry::default())
    }

    /// The view's model of `lock`, its plugin fields with the code
    /// `registry` holds for them.
    pub fn from_lock_with(lock: &ModelLock, registry: &Registry) -> Result<Model, Invalid> {
        let refusals = check::validate(lock);
        if !refusals.is_empty() {
            return Err(Invalid::Lock(refusals));
        }
        let mut model = Model {
            id: lock.model.id.0,
            version: lock.model.version,
            ..Model::default()
        };
        for (name, collection) in &lock.collections {
            let fields: BTreeMap<String, Field> = collection
                .fields
                .iter()
                .filter_map(|(field, f)| Some((field.clone(), field_type(f, registry)?)))
                .collect();
            let unregistered: BTreeMap<String, (String, u64)> = collection
                .fields
                .iter()
                .filter(|(field, _)| !fields.contains_key(*field))
                .filter_map(|(field, f)| match &f.kind {
                    model_lock::Kind::Plugin { type_id, .. } => {
                        Some((field.clone(), (type_id.clone(), f.rule)))
                    }
                    _ => None,
                })
                .collect();
            if !unregistered.is_empty() {
                model.unregistered.insert(name.clone(), unregistered);
            }
            model.collections.insert(name.clone(), fields);
            let personal: BTreeMap<String, Person> = collection
                .fields
                .iter()
                .filter_map(|(field, f)| Some((field.clone(), f.personal.clone()?)))
                .collect();
            if !personal.is_empty() {
                model.personal.insert(name.clone(), personal);
            }
            for (index, i) in &collection.indexes {
                let keys = i
                    .fields
                    .iter()
                    .map(|f| (f.field.clone(), direction(f.direction)))
                    .collect();
                model.indexes.push(Index {
                    name: index.clone(),
                    collection: name.clone(),
                    keys,
                    // validate holds `equal` to at most the index's fields.
                    equality: i.equal as usize,
                    scope: i.scope,
                });
            }
        }
        // Each collection whose objects can be a person carries `$person`
        // (RFC 0005, "Where the person id is kept").
        let persons: Vec<String> = model
            .personal
            .iter()
            .flat_map(|(collection, fields)| {
                fields.values().flat_map(|person| match person {
                    Person::Itself => vec![collection.clone()],
                    Person::Link(link) => match model.field(collection, link) {
                        Some(Field::Link(link)) => link.collections.iter().cloned().collect(),
                        _ => Vec::new(),
                    },
                })
            })
            .collect();
        for collection in persons {
            if let Some(fields) = model.collections.get_mut(&collection) {
                fields.insert(PERSON_FIELD.to_owned(), Field::Person(PersonField));
            }
        }
        model.validate()?;
        Ok(model)
    }
}

/// The view's field type of a lock's field; none for a plugin type
/// without code.
fn field_type(field: &model_lock::Field, registry: &Registry) -> Option<Field> {
    let kind = &field.kind;
    let basic = |kind| Some(Field::Basic(BasicValue { kind }));
    let targets = |t: &[String]| t.iter().cloned().collect();
    match kind {
        model_lock::Kind::String => basic(Kind::Text),
        model_lock::Kind::Int => basic(Kind::Integer),
        model_lock::Kind::Float => basic(Kind::Float),
        model_lock::Kind::Boolean => basic(Kind::Boolean),
        model_lock::Kind::Choice(options) => basic(choice(options)),
        model_lock::Kind::Set(element) => Some(Field::Set(Set {
            kind: element_kind(element),
        })),
        model_lock::Kind::Link(t) => Some(Field::Link(Link {
            collections: targets(t),
        })),
        model_lock::Kind::Links(t) => Some(Field::Links(Links {
            collections: targets(t),
        })),
        model_lock::Kind::Ref(t) => Some(Field::Ref(Reference {
            collections: targets(t),
        })),
        model_lock::Kind::Plugin { type_id, config } => {
            plugin(type_id, field.rule, config, registry)
        }
    }
}

/// A plugin field with the code `registry` holds for `type_id` at `rule`,
/// configured with `config`; none without code, or when the plugin refuses
/// the `config`.
fn plugin(
    type_id: &str,
    rule: u64,
    config: &Option<serde_json::Value>,
    registry: &Registry,
) -> Option<Field> {
    let code = registry.code(type_id, rule)?;
    let config = match config {
        Some(json) => Some(data(json)?),
        None => None,
    };
    let configured = code.configure(rule, config.as_ref()).ok()?;
    let bytes = match &config {
        Some(config) => config.encode().ok()?,
        None => Vec::new(),
    };
    Some(Field::Plugin(PluginField {
        type_id: type_id.to_owned(),
        rule,
        config: bytes,
        code: configured,
    }))
}

/// A `config` from the lock's JSON as `Data`: a number with no fraction
/// and in range as an integer, any other as a float.
fn data(json: &serde_json::Value) -> Option<Data> {
    use serde_json::Value as Json;
    Some(match json {
        Json::Null => Data::Null,
        Json::Bool(b) => Data::Bool(*b),
        Json::Number(n) => match (n.as_u64(), n.as_i64()) {
            (Some(u), _) => Data::Integer(i128::from(u)),
            (None, Some(i)) => Data::Integer(i128::from(i)),
            _ => Data::Float(n.as_f64()?),
        },
        Json::String(text) => Data::Text(text.clone()),
        Json::Array(items) => Data::Array(items.iter().map(data).collect::<Option<_>>()?),
        Json::Object(entries) => Data::Map(
            entries
                .iter()
                .map(|(key, value)| Some((key.clone(), data(value)?)))
                .collect::<Option<_>>()?,
        ),
    })
}

/// The kind of basic value a set's element holds.
fn element_kind(element: &Element) -> Kind {
    match element {
        Element::String => Kind::Text,
        Element::Int => Kind::Integer,
        Element::Float => Kind::Float,
        Element::Boolean => Kind::Boolean,
        Element::Choice(options) => choice(options),
    }
}

fn choice(options: &[String]) -> Kind {
    Kind::Choice(options.iter().cloned().collect())
}

fn direction(direction: model_lock::Direction) -> Direction {
    match direction {
        model_lock::Direction::Asc => Direction::Ascending,
        model_lock::Direction::Desc => Direction::Descending,
    }
}
