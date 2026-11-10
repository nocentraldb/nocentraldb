//! From the description of a schema and the previous lock to the next lock,
//! as `ncdb model build` does (RFC 0005, "The structure of model.lock.json").
//!
//! A [`Draft`] is what the TypeScript schema declares, without history: the
//! collections, their fields, computed fields, views and indexes, and the
//! deprecation reasons. It is the JSON the SDK's schema functions describe a
//! model as; nothing stores it. [`build`] adds the history from the
//! previous lock: `since` of everything the previous lock already has, the
//! model version, which grows by one only when the model hash changes, and
//! the `model.id`, which only the first build draws. The result must pass
//! the additive-change rule ([`super::check`]), or nothing is built.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value};

use super::read::{
    direction, entries, error, field_core, join, object, parse, plugins, required, scope, string,
    whole,
};
use super::{
    Collection, Deprecation, Field, Index, IndexField, LockError, ModelId, ModelInfo, ModelLock,
    Named, Plugin, Refusal, check,
};

/// The description of a schema: what it declares, without history.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Draft {
    /// The plugins whose field types the schema uses.
    pub plugins: BTreeMap<String, Plugin>,
    /// The collections, by name.
    pub collections: BTreeMap<String, DraftCollection>,
}

/// A collection as the schema declares it.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct DraftCollection {
    /// The text of the collection's `.deprecated()`, when it is deprecated.
    pub deprecated: Option<String>,
    /// The stored fields; their `since` is unused, and `deprecated`, when
    /// set, holds only the reason.
    pub fields: BTreeMap<String, Field>,
    /// The names of the computed fields.
    pub computed: BTreeSet<String>,
    /// The names of the views.
    pub views: BTreeSet<String>,
    /// The indexes; their `since` is unused.
    pub indexes: BTreeMap<String, Index>,
}

impl Draft {
    /// Reads a schema's description from its JSON text:
    ///
    /// ```json
    /// { "collections": { "tasks": {
    ///     "deprecated": "Use issues.",
    ///     "fields": { "title": { "rule": 1, "type": "ncdb/string", "deprecated": "Use summary." } },
    ///     "computed": ["blocked"], "views": ["ready"],
    ///     "indexes": { "byTitle": { "equal": 0, "fields": [{ "direction": "asc", "field": "title" }] } } } },
    ///   "plugins": {} }
    /// ```
    ///
    /// Collections and fields take the keys of the lock without `since`, and
    /// `deprecated` is the reason.
    pub fn from_json(text: &str) -> Result<Draft, LockError> {
        let root = parse(text)?;
        let top = object(&root, "")?;
        Ok(Draft {
            plugins: plugins(required(top, "", "plugins")?, "plugins")?,
            collections: entries(
                required(top, "", "collections")?,
                "collections",
                draft_collection,
            )?,
        })
    }
}

fn draft_collection(map: &Map<String, Value>, path: &str) -> Result<DraftCollection, LockError> {
    let names = |key: &str| -> Result<BTreeSet<String>, LockError> {
        let at = join(path, key);
        let Some(value) = map.get(key) else {
            return Ok(BTreeSet::new());
        };
        let items = value
            .as_array()
            .ok_or_else(|| error(&at, "expected an array of names"))?;
        items
            .iter()
            .enumerate()
            .map(|(i, item)| string(item, &join(&at, &i.to_string())))
            .collect()
    };
    let reason = |map: &Map<String, Value>, at: &str| {
        map.get("deprecated")
            .map(|v| string(v, &join(at, "deprecated")))
            .transpose()
    };
    let fields = match map.get("fields") {
        None => BTreeMap::new(),
        Some(value) => entries(value, &join(path, "fields"), |field, at| {
            Ok(Field {
                deprecated: reason(field, at)?.map(|reason| Deprecation { since: 0, reason }),
                ..field_core(field, at)?
            })
        })?,
    };
    let indexes = match map.get("indexes") {
        None => BTreeMap::new(),
        Some(value) => entries(value, &join(path, "indexes"), draft_index)?,
    };
    Ok(DraftCollection {
        deprecated: reason(map, path)?,
        fields,
        computed: names("computed")?,
        views: names("views")?,
        indexes,
    })
}

fn draft_index(map: &Map<String, Value>, path: &str) -> Result<Index, LockError> {
    let at = join(path, "fields");
    let items = required(map, path, "fields")?
        .as_array()
        .ok_or_else(|| error(&at, "expected an array"))?;
    let fields = items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let at = join(&at, &i.to_string());
            let entry = object(item, &at)?;
            Ok(IndexField {
                field: string(required(entry, &at, "field")?, &join(&at, "field"))?,
                direction: direction(required(entry, &at, "direction")?, &join(&at, "direction"))?,
            })
        })
        .collect::<Result<_, LockError>>()?;
    Ok(Index {
        fields,
        equal: whole(required(map, path, "equal")?, &join(path, "equal"))?,
        scope: scope(map, path)?,
        since: 0,
    })
}

/// A lock that passed the additive-change rule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Built {
    /// The next lock. Its model version is the previous one when the model
    /// hash did not change (descriptions only, or nothing).
    pub lock: ModelLock,
}

/// Builds the next lock from `draft` and the `previous` lock, or returns
/// every refusal. `new_id` draws the `model.id` of a first build, when
/// there is no previous lock.
pub fn build(
    draft: &Draft,
    previous: Option<&ModelLock>,
    new_id: impl FnOnce() -> ModelId,
) -> Result<Built, Vec<Refusal>> {
    let version = previous.map_or(1, |p| p.model.version + 1);
    let empty = BTreeMap::new();
    let old_collections = previous.map_or(&empty, |p| &p.collections);
    let collections = draft
        .collections
        .iter()
        .map(|(name, c)| {
            (
                name.clone(),
                collection(c, old_collections.get(name), version),
            )
        })
        .collect();
    let mut lock = ModelLock {
        model: ModelInfo {
            id: ModelId([0; 16]),
            version,
            hash: String::new(),
        },
        plugins: draft.plugins.clone(),
        collections,
    };
    lock.model.hash = lock.compute_hash();
    let refusals = match previous {
        Some(previous) => {
            lock.model.id = previous.model.id;
            if lock.model.hash == previous.model.hash {
                lock.model.version = previous.model.version;
            }
            check::successor(previous, &lock)
        }
        None => {
            lock.model.id = new_id();
            check::validate(&lock)
        }
    };
    if refusals.is_empty() {
        Ok(Built { lock })
    } else {
        Err(refusals)
    }
}

/// The collection with its history: what `old` has keeps its `since`, and
/// what is new takes `version`.
fn collection(draft: &DraftCollection, old: Option<&Collection>, version: u64) -> Collection {
    let named = |names: &BTreeSet<String>, old: Option<&BTreeMap<String, Named>>| {
        names
            .iter()
            .map(|name| {
                let since = old.and_then(|o| o.get(name)).map_or(version, |n| n.since);
                (name.clone(), Named { since })
            })
            .collect()
    };
    let fields = draft
        .fields
        .iter()
        .map(|(name, field)| {
            let before = old.and_then(|o| o.fields.get(name));
            let deprecated = deprecation(
                field.deprecated.as_ref().map(|d| &d.reason),
                before.and_then(|b| b.deprecated.as_ref()),
                version,
            );
            let since = before.map_or(version, |b| b.since);
            (
                name.clone(),
                Field {
                    since,
                    deprecated,
                    ..field.clone()
                },
            )
        })
        .collect();
    let indexes = draft
        .indexes
        .iter()
        .map(|(name, index)| {
            let since = old
                .and_then(|o| o.indexes.get(name))
                .filter(|b| {
                    b.fields == index.fields && b.equal == index.equal && b.scope == index.scope
                })
                .map_or(version, |b| b.since);
            (
                name.clone(),
                Index {
                    since,
                    ..index.clone()
                },
            )
        })
        .collect();
    Collection {
        since: old.map_or(version, |o| o.since),
        deprecated: deprecation(
            draft.deprecated.as_ref(),
            old.and_then(|o| o.deprecated.as_ref()),
            version,
        ),
        fields,
        computed: named(&draft.computed, old.map(|o| &o.computed)),
        views: named(&draft.views, old.map(|o| &o.views)),
        indexes,
    }
}

/// The deprecation the schema declares with `reason`: since the version of
/// the deprecation `before` when there was one, else since `version`.
fn deprecation(
    reason: Option<&String>,
    before: Option<&Deprecation>,
    version: u64,
) -> Option<Deprecation> {
    reason.map(|reason| Deprecation {
        since: before.map_or(version, |b| b.since),
        reason: reason.clone(),
    })
}
