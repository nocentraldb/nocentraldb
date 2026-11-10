//! The view's model read from the member's model lock (RFC 0005, "the core
//! and both SDKs read only the lock"): the lock's collections with their
//! stored fields, and each collection's declared indexes, in the lock's
//! order of names.
//!
//! A plugin's field type has no field type in the view yet, so the view's
//! model leaves the field out: the fold parks its operations, as it parks
//! any field its model does not know, and keeps them (RFC 0004).
//! Computed fields and views have no place in the view yet either.

use std::collections::BTreeMap;

use super::model::{Direction, Field, Index, Invalid, Model};
use crate::basic::{BasicValue, Kind};
use crate::link::{Link, Links};
use crate::model_lock::{self, Element, ModelLock, check};
use crate::reference::Reference;
use crate::set::Set;

impl Model {
    /// The view's model of `lock`. A lock that breaks the rules RFC 0005
    /// gives a lock on its own is refused with every rule it breaks.
    pub fn from_lock(lock: &ModelLock) -> Result<Model, Invalid> {
        let refusals = check::validate(lock);
        if !refusals.is_empty() {
            return Err(Invalid::Lock(refusals));
        }
        let mut model = Model::default();
        for (name, collection) in &lock.collections {
            let fields: BTreeMap<String, Field> = collection
                .fields
                .iter()
                .filter_map(|(field, f)| Some((field.clone(), field_type(&f.kind)?)))
                .collect();
            model.collections.insert(name.clone(), fields);
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
        model.validate()?;
        Ok(model)
    }
}

/// The view's field type of a lock's field type; none for a plugin's.
fn field_type(kind: &model_lock::Kind) -> Option<Field> {
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
        model_lock::Kind::Plugin { .. } => None,
    }
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
