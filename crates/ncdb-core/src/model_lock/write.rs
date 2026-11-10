//! Writing `model.lock.json` in its one canonical form, and the model hash
//! (RFC 0005, "Canonical form" and "The keys").
//!
//! The canonical form sorts keys by their UTF-16 code units, as RFC 8785
//! does, indents by two spaces, ends lines with LF and the file with a
//! newline. "Short objects on one line as a formatter would put them" is
//! made exact here, so two builds always give the same bytes:
//!
//! - an array goes on one line when that line fits in 80 columns, its
//!   trailing comma included;
//! - an object goes on one line under the same condition, and only when
//!   none of its values is a non-empty object.
//!
//! This reproduces RFC 0005's example lock byte for byte.

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use super::{Collection, Element, FORMAT, Field, Kind, ModelLock, Scope};

/// The widest line on which an object or array is put on one line.
const WIDTH: usize = 80;

impl ModelLock {
    /// The lock as a JSON value.
    pub fn to_json(&self) -> Value {
        let mut model = Map::new();
        model.insert("hash".into(), self.model.hash.clone().into());
        model.insert("id".into(), self.model.id.to_string().into());
        model.insert("version".into(), self.model.version.into());
        let mut top = self.content(true);
        top.insert("model".into(), Value::Object(model));
        Value::Object(top)
    }

    /// The lock in its canonical form, as `ncdb model build` writes it.
    pub fn to_canonical_json(&self) -> String {
        let mut out = String::new();
        pretty(&mut out, &self.to_json(), 0, "", "");
        out.push('\n');
        out
    }

    /// The model hash of this lock's content: `sha256:` and the hex SHA-256
    /// of the canonical JSON (RFC 8785) of `format`, `plugins` and
    /// `collections`, with every field's `description` removed. The `model`
    /// entry is not part of it.
    pub fn compute_hash(&self) -> String {
        let digest = Sha256::digest(canonical(&Value::Object(self.content(false))).as_bytes());
        let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
        format!("sha256:{hex}")
    }

    /// `format`, `plugins` and `collections`, with or without descriptions.
    fn content(&self, descriptions: bool) -> Map<String, Value> {
        let mut plugins = Map::new();
        for (namespace, plugin) in &self.plugins {
            let mut entry = Map::new();
            if let Some(name) = &plugin.crate_name {
                entry.insert("crate".into(), name.clone().into());
            }
            if let Some(name) = &plugin.npm {
                entry.insert("npm".into(), name.clone().into());
            }
            plugins.insert(namespace.clone(), Value::Object(entry));
        }
        let collections = self
            .collections
            .iter()
            .map(|(name, c)| (name.clone(), collection(c, descriptions)))
            .collect();
        let mut top = Map::new();
        top.insert("collections".into(), Value::Object(collections));
        top.insert("format".into(), FORMAT.into());
        top.insert("plugins".into(), Value::Object(plugins));
        top
    }
}

fn collection(collection: &Collection, descriptions: bool) -> Value {
    let mut entry = Map::new();
    entry.insert("since".into(), collection.since.into());
    if let Some(version) = collection.deprecated {
        entry.insert("deprecated".into(), version.into());
    }
    let fields = collection
        .fields
        .iter()
        .map(|(name, f)| (name.clone(), field(f, descriptions)))
        .collect();
    entry.insert("fields".into(), Value::Object(fields));
    for (key, named) in [
        ("computed", &collection.computed),
        ("views", &collection.views),
    ] {
        if !named.is_empty() {
            let names = named
                .iter()
                .map(|(name, n)| (name.clone(), since(n.since)))
                .collect();
            entry.insert(key.into(), Value::Object(names));
        }
    }
    if !collection.indexes.is_empty() {
        let indexes = collection
            .indexes
            .iter()
            .map(|(name, index)| {
                let fields = index
                    .fields
                    .iter()
                    .map(|f| {
                        let mut entry = Map::new();
                        entry.insert("direction".into(), f.direction.as_str().into());
                        entry.insert("field".into(), f.field.clone().into());
                        Value::Object(entry)
                    })
                    .collect();
                let mut entry = Map::new();
                entry.insert("equal".into(), index.equal.into());
                entry.insert("fields".into(), Value::Array(fields));
                if index.scope == Scope::Space {
                    entry.insert("scope".into(), "space".into());
                }
                entry.insert("since".into(), index.since.into());
                (name.clone(), Value::Object(entry))
            })
            .collect();
        entry.insert("indexes".into(), Value::Object(indexes));
    }
    Value::Object(entry)
}

fn since(version: u64) -> Value {
    let mut entry = Map::new();
    entry.insert("since".into(), version.into());
    Value::Object(entry)
}

fn strings(items: &[String]) -> Value {
    Value::Array(items.iter().cloned().map(Value::from).collect())
}

fn field(field: &Field, descriptions: bool) -> Value {
    let mut entry = Map::new();
    entry.insert("type".into(), field.kind.type_id().into());
    entry.insert("rule".into(), field.rule.into());
    entry.insert("since".into(), field.since.into());
    match &field.kind {
        Kind::Choice(options) => {
            entry.insert("options".into(), strings(options));
        }
        Kind::Set(element) => {
            let mut of = Map::new();
            of.insert("type".into(), element.type_id().into());
            if let Element::Choice(options) = element {
                of.insert("options".into(), strings(options));
            }
            entry.insert("of".into(), Value::Object(of));
        }
        Kind::Link(targets) | Kind::Links(targets) => {
            entry.insert("targets".into(), strings(targets));
        }
        Kind::Plugin {
            config: Some(config),
            ..
        } => {
            entry.insert("config".into(), config.clone());
        }
        _ => {}
    }
    if let Some(person) = &field.personal {
        let mut personal = Map::new();
        personal.insert("person".into(), person.as_str().into());
        entry.insert("personal".into(), Value::Object(personal));
    }
    if let (Some(text), true) = (&field.description, descriptions) {
        entry.insert("description".into(), text.clone().into());
    }
    if let Some(deprecation) = &field.deprecated {
        entry.insert("deprecated".into(), deprecation.since.into());
        entry.insert("deprecatedReason".into(), deprecation.reason.clone().into());
    }
    Value::Object(entry)
}

/// The canonical JSON of RFC 8785: no whitespace, keys sorted by their
/// UTF-16 code units, strings escaped as ECMAScript's `JSON.stringify`
/// does, numbers written as ECMAScript writes an IEEE 754 double.
pub fn canonical(value: &Value) -> String {
    let mut out = String::new();
    write_canonical(&mut out, value);
    out
}

fn write_canonical(out: &mut String, value: &Value) {
    match value {
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_canonical(out, item);
            }
            out.push(']');
        }
        Value::Object(map) => {
            out.push('{');
            for (i, (key, item)) in sorted(map).into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_string(out, key);
                out.push(':');
                write_canonical(out, item);
            }
            out.push('}');
        }
        scalar => write_scalar(out, scalar),
    }
}

/// The entries of an object, sorted by the UTF-16 code units of the keys.
fn sorted(map: &Map<String, Value>) -> Vec<(&String, &Value)> {
    let mut entries: Vec<_> = map.iter().collect();
    entries.sort_by(|(a, _), (b, _)| a.encode_utf16().cmp(b.encode_utf16()));
    entries
}

fn write_scalar(out: &mut String, value: &Value) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => out.push_str(&number(n.as_f64().unwrap_or(f64::NAN))),
        Value::String(s) => write_string(out, s),
        Value::Array(_) | Value::Object(_) => unreachable!("not a scalar"),
    }
}

fn write_string(out: &mut String, text: &str) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// A double as ECMAScript's `Number.prototype.toString` writes it (RFC
/// 8785, section 3.2.2.3). JSON has no NaN or infinity, so serde_json never
/// holds one.
fn number(value: f64) -> String {
    if value == 0.0 {
        return "0".into();
    }
    if value < 0.0 {
        return format!("-{}", number(-value));
    }
    // Rust writes the shortest digits that round-trip, as ECMAScript does.
    let scientific = format!("{value:e}");
    let (mantissa, exponent) = scientific.split_once('e').expect("{:e} writes an exponent");
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let k = digits.len() as i64;
    let n = exponent.parse::<i64>().expect("the exponent is a number") + 1;
    if k <= n && n <= 21 {
        format!("{digits}{}", "0".repeat((n - k) as usize))
    } else if 0 < n && n <= 21 {
        format!("{}.{}", &digits[..n as usize], &digits[n as usize..])
    } else if -6 < n && n <= 0 {
        format!("0.{}{digits}", "0".repeat((-n) as usize))
    } else {
        let e = n - 1;
        let sign = if e >= 0 { "+" } else { "-" };
        let (first, rest) = digits.split_at(1);
        let fraction = if rest.is_empty() {
            String::new()
        } else {
            format!(".{rest}")
        };
        format!("{first}{fraction}e{sign}{}", e.abs())
    }
}

/// The value on one line, with a space inside an object's braces and after
/// each comma and colon.
fn inline(value: &Value) -> String {
    match value {
        Value::Array(items) => {
            let items: Vec<_> = items.iter().map(inline).collect();
            format!("[{}]", items.join(", "))
        }
        Value::Object(map) if map.is_empty() => "{}".into(),
        Value::Object(map) => {
            let entries: Vec<_> = sorted(map)
                .into_iter()
                .map(|(key, item)| {
                    let mut entry = String::new();
                    write_string(&mut entry, key);
                    format!("{entry}: {}", inline(item))
                })
                .collect();
            format!("{{ {} }}", entries.join(", "))
        }
        scalar => {
            let mut out = String::new();
            write_scalar(&mut out, scalar);
            out
        }
    }
}

/// Whether the value may go on one line, width aside: no object in it
/// holds a non-empty object.
fn may_inline(value: &Value) -> bool {
    match value {
        Value::Array(items) => items.iter().all(may_inline),
        Value::Object(map) => map.values().all(|item| match item {
            Value::Object(inner) => inner.is_empty(),
            other => may_inline(other),
        }),
        _ => true,
    }
}

/// Writes `value` at `indent`, after `prefix` (a key) and before `suffix`
/// (a comma), without a final newline.
fn pretty(out: &mut String, value: &Value, indent: usize, prefix: &str, suffix: &str) {
    let pad = " ".repeat(indent);
    let line = format!("{pad}{prefix}{}{suffix}", inline(value));
    let expand = match value {
        Value::Array(items) => !items.is_empty(),
        Value::Object(map) => !map.is_empty(),
        _ => false,
    };
    if !expand || (may_inline(value) && line.chars().count() <= WIDTH) {
        out.push_str(&line);
        return;
    }
    let (open, close, items): (char, char, Vec<(String, &Value)>) = match value {
        Value::Array(items) => ('[', ']', items.iter().map(|v| (String::new(), v)).collect()),
        Value::Object(map) => (
            '{',
            '}',
            sorted(map)
                .into_iter()
                .map(|(key, v)| {
                    let mut name = String::new();
                    write_string(&mut name, key);
                    (format!("{name}: "), v)
                })
                .collect(),
        ),
        _ => unreachable!("only arrays and objects expand"),
    };
    out.push_str(&format!("{pad}{prefix}{open}\n"));
    let last = items.len() - 1;
    for (i, (key, item)) in items.into_iter().enumerate() {
        pretty(out, item, indent + 2, &key, if i < last { "," } else { "" });
        out.push('\n');
    }
    out.push_str(&format!("{pad}{close}{suffix}"));
}
