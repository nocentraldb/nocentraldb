//! Reading `model.lock.json`, and the description of a schema that
//! `ncdb model build` gets from the TypeScript module.
//!
//! The reader checks the shape of the document and the kind of every
//! value, and names the JSON path of the first one that is wrong. Whether
//! the model itself is sound (its names, indexes and personal fields) is
//! [`super::check`]'s job. Keys a reader does not know are ignored: a
//! later lock may add optional keys without a new format (RFC 0005, D8).

use std::collections::BTreeMap;
use std::fmt;

use serde_json::{Map, Value};

use super::{
    Collection, Deprecation, Direction, Element, FORMAT, Field, Index, IndexField, Kind, ModelId,
    ModelInfo, ModelLock, Named, Person, Plugin,
};

/// Why a lock (or a schema's description) could not be read: the JSON
/// path of the offending value, and what is wrong with it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LockError {
    /// Where, such as `collections.tasks.fields.title.since`; empty for the
    /// whole document.
    pub path: String,
    /// What is wrong.
    pub message: String,
}

impl fmt::Display for LockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.path.is_empty() {
            f.write_str(&self.message)
        } else {
            write!(f, "{}: {}", self.path, self.message)
        }
    }
}

impl std::error::Error for LockError {}

impl ModelLock {
    /// Reads a lock from its JSON text.
    pub fn from_json(text: &str) -> Result<ModelLock, LockError> {
        let root = parse(text)?;
        let top = object(&root, "")?;
        let format = whole(required(top, "", "format")?, "format")?;
        if format != FORMAT {
            return Err(error(
                "format",
                format!(
                    "format {format} is not known to this version; it reads format {FORMAT}. \
                     Build the lock with the matching version of ncdb, or upgrade."
                ),
            ));
        }
        let model = object(required(top, "", "model")?, "model")?;
        let id = model_id(required(model, "model", "id")?, "model.id")?;
        let version = whole(required(model, "model", "version")?, "model.version")?;
        let hash = string(required(model, "model", "hash")?, "model.hash")?;
        let plugins = plugins(required(top, "", "plugins")?, "plugins")?;
        let collections = entries(required(top, "", "collections")?, "collections", collection)?;
        Ok(ModelLock {
            model: ModelInfo { id, version, hash },
            plugins,
            collections,
        })
    }
}

/// Parses JSON text into a value; the error names no path.
pub(super) fn parse(text: &str) -> Result<Value, LockError> {
    serde_json::from_str(text).map_err(|e| error("", format!("not valid JSON: {e}")))
}

pub(super) fn error(path: &str, message: impl Into<String>) -> LockError {
    LockError {
        path: path.to_owned(),
        message: message.into(),
    }
}

pub(super) fn join(path: &str, key: &str) -> String {
    if path.is_empty() {
        key.to_owned()
    } else {
        format!("{path}.{key}")
    }
}

pub(super) fn object<'a>(
    value: &'a Value,
    path: &str,
) -> Result<&'a Map<String, Value>, LockError> {
    value
        .as_object()
        .ok_or_else(|| error(path, "expected an object"))
}

pub(super) fn required<'a>(
    map: &'a Map<String, Value>,
    path: &str,
    key: &str,
) -> Result<&'a Value, LockError> {
    map.get(key)
        .ok_or_else(|| error(&join(path, key), "missing"))
}

pub(super) fn whole(value: &Value, path: &str) -> Result<u64, LockError> {
    value
        .as_u64()
        .ok_or_else(|| error(path, "expected a whole number"))
}

pub(super) fn string(value: &Value, path: &str) -> Result<String, LockError> {
    value
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| error(path, "expected a string"))
}

pub(super) fn strings(value: &Value, path: &str) -> Result<Vec<String>, LockError> {
    let items = value
        .as_array()
        .ok_or_else(|| error(path, "expected an array of strings"))?;
    items
        .iter()
        .enumerate()
        .map(|(i, item)| string(item, &join(path, &i.to_string())))
        .collect()
}

/// An object whose every value is read by `read`, keyed by name.
pub(super) fn entries<T>(
    value: &Value,
    path: &str,
    read: impl Fn(&Map<String, Value>, &str) -> Result<T, LockError>,
) -> Result<BTreeMap<String, T>, LockError> {
    object(value, path)?
        .iter()
        .map(|(key, entry)| {
            let at = join(path, key);
            Ok((key.clone(), read(object(entry, &at)?, &at)?))
        })
        .collect()
}

fn optional<'a>(map: &'a Map<String, Value>, key: &str) -> Option<&'a Value> {
    map.get(key)
}

fn model_id(value: &Value, path: &str) -> Result<ModelId, LockError> {
    let text = string(value, path)?;
    let bad = || error(path, "expected 32 lower-case hex digits");
    if text.len() != 32
        || !text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(bad());
    }
    let mut bytes = [0; 16];
    for (i, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[2 * i..2 * i + 2], 16).map_err(|_| bad())?;
    }
    Ok(ModelId(bytes))
}

pub(super) fn plugins(value: &Value, path: &str) -> Result<BTreeMap<String, Plugin>, LockError> {
    entries(value, path, |map, at| {
        let text = |key| {
            optional(map, key)
                .map(|v| string(v, &join(at, key)))
                .transpose()
        };
        Ok(Plugin {
            npm: text("npm")?,
            crate_name: text("crate")?,
        })
    })
}

fn collection(map: &Map<String, Value>, path: &str) -> Result<Collection, LockError> {
    let named = |key: &str| -> Result<BTreeMap<String, Named>, LockError> {
        match optional(map, key) {
            None => Ok(BTreeMap::new()),
            Some(value) => entries(value, &join(path, key), |entry, at| {
                Ok(Named {
                    since: whole(required(entry, at, "since")?, &join(at, "since"))?,
                })
            }),
        }
    };
    Ok(Collection {
        since: whole(required(map, path, "since")?, &join(path, "since"))?,
        deprecated: deprecation(map, path)?,
        fields: match optional(map, "fields") {
            None => BTreeMap::new(),
            Some(value) => entries(value, &join(path, "fields"), field)?,
        },
        computed: named("computed")?,
        views: named("views")?,
        indexes: match optional(map, "indexes") {
            None => BTreeMap::new(),
            Some(value) => entries(value, &join(path, "indexes"), index)?,
        },
    })
}

fn field(map: &Map<String, Value>, path: &str) -> Result<Field, LockError> {
    Ok(Field {
        since: whole(required(map, path, "since")?, &join(path, "since"))?,
        deprecated: deprecation(map, path)?,
        ..field_core(map, path)?
    })
}

/// The keys `deprecated` and `deprecatedReason` of a field or a collection.
fn deprecation(map: &Map<String, Value>, path: &str) -> Result<Option<Deprecation>, LockError> {
    let Some(value) = optional(map, "deprecated") else {
        return Ok(None);
    };
    Ok(Some(Deprecation {
        since: whole(value, &join(path, "deprecated"))?,
        reason: optional(map, "deprecatedReason")
            .map(|v| string(v, &join(path, "deprecatedReason")))
            .transpose()?
            .unwrap_or_default(),
    }))
}

/// What a field is, apart from its history: its type, rule, person and
/// description. `since` and `deprecated` are left at their defaults.
pub(super) fn field_core(map: &Map<String, Value>, path: &str) -> Result<Field, LockError> {
    let personal = match optional(map, "personal") {
        None => None,
        Some(value) => {
            let at = join(path, "personal");
            let person = string(
                required(object(value, &at)?, &at, "person")?,
                &join(&at, "person"),
            )?;
            Some(if person == "self" {
                Person::Itself
            } else {
                Person::Link(person)
            })
        }
    };
    Ok(Field {
        kind: kind(map, path)?,
        rule: whole(required(map, path, "rule")?, &join(path, "rule"))?,
        since: 0,
        personal,
        description: optional(map, "description")
            .map(|v| string(v, &join(path, "description")))
            .transpose()?,
        deprecated: None,
    })
}

fn kind(map: &Map<String, Value>, path: &str) -> Result<Kind, LockError> {
    let type_path = join(path, "type");
    let type_id = string(required(map, path, "type")?, &type_path)?;
    let list = |key| strings(required(map, path, key)?, &join(path, key));
    Ok(match type_id.as_str() {
        "ncdb/string" => Kind::String,
        "ncdb/int" => Kind::Int,
        "ncdb/float" => Kind::Float,
        "ncdb/boolean" => Kind::Boolean,
        "ncdb/choice" => Kind::Choice(list("options")?),
        "ncdb/set" => {
            let at = join(path, "of");
            Kind::Set(element(object(required(map, path, "of")?, &at)?, &at)?)
        }
        "ncdb/link" => Kind::Link(list("targets")?),
        "ncdb/links" => Kind::Links(list("targets")?),
        other => match other.split_once('/') {
            Some((namespace, name))
                if !namespace.is_empty() && !name.is_empty() && namespace != super::BUILT_IN =>
            {
                Kind::Plugin {
                    type_id: type_id.clone(),
                    config: optional(map, "config").cloned(),
                }
            }
            _ => {
                return Err(error(
                    &type_path,
                    format!("{other} is no field type: expected ncdb/<type> or <plugin>/<type>"),
                ));
            }
        },
    })
}

fn element(map: &Map<String, Value>, path: &str) -> Result<Element, LockError> {
    let type_path = join(path, "type");
    let type_id = string(required(map, path, "type")?, &type_path)?;
    Ok(match type_id.as_str() {
        "ncdb/string" => Element::String,
        "ncdb/int" => Element::Int,
        "ncdb/float" => Element::Float,
        "ncdb/boolean" => Element::Boolean,
        "ncdb/choice" => Element::Choice(strings(
            required(map, path, "options")?,
            &join(path, "options"),
        )?),
        other => {
            return Err(error(
                &type_path,
                format!("a set's element is a basic value, not {other}"),
            ));
        }
    })
}

fn index(map: &Map<String, Value>, path: &str) -> Result<Index, LockError> {
    let fields_path = join(path, "fields");
    let items = required(map, path, "fields")?
        .as_array()
        .ok_or_else(|| error(&fields_path, "expected an array"))?;
    let fields = items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let at = join(&fields_path, &i.to_string());
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
        since: whole(required(map, path, "since")?, &join(path, "since"))?,
    })
}

pub(super) fn direction(value: &Value, path: &str) -> Result<Direction, LockError> {
    match value.as_str() {
        Some("asc") => Ok(Direction::Asc),
        Some("desc") => Ok(Direction::Desc),
        _ => Err(error(path, "expected \"asc\" or \"desc\"")),
    }
}
