//! What `ncdb model build` says about a lock it wrote or refused (RFC 0005,
//! "An example of each").
//!
//! A built lock is listed change by change: `+` for what was added, `~` for
//! what changed in place (a deprecation, a description, an index), `-` for
//! a removed index, each under the lock's name. A refused change is listed
//! with `x` and its message from the core.

use std::fmt::Write;

use ncdb_core::model_lock::{Collection, Field, Index, Kind, ModelLock, Person, Refusal};

/// The width of the name column in a field's line.
const NAME: usize = 17;

/// What a build that wrote `next` on `previous` changed, after the lock's
/// name: a summary line, a line per change, and for a new model version
/// the line that says it is additive.
pub fn changes(previous: Option<&ModelLock>, next: &ModelLock) -> String {
    let Some(previous) = previous else {
        let mut out = format!("model version 1, a new model ({})\n", next.model.id);
        for (name, collection) in &next.collections {
            let _ = writeln!(out, "  + collection {name} ({})", summary(collection));
        }
        return out;
    };
    let (from, to) = (previous.model.version, next.model.version);
    let mut out = if from == to {
        format!("descriptions changed (model version stays {to})\n")
    } else {
        format!("model version {from} -> {to}\n")
    };
    for (name, collection) in &next.collections {
        match previous.collections.get(name) {
            None => {
                let _ = writeln!(out, "  + collection {name} ({})", summary(collection));
            }
            Some(old) => {
                for line in collection_lines(name, old, collection) {
                    let _ = writeln!(out, "  {line}");
                }
            }
        }
    }
    if from != to {
        let _ = writeln!(
            out,
            "additive: clients on model version {from} keep every change written under version {to}."
        );
    }
    out
}

/// What a build said when it refused: a summary line under the lock's name,
/// and each refusal with its message.
pub fn refused(lock: &str, previous: Option<&ModelLock>, refusals: &[Refusal]) -> String {
    let mut out = match previous {
        Some(previous) => format!(
            "{lock}: refused, the change is not additive (model version stays {})\n",
            previous.model.version
        ),
        None => format!("{lock}: refused, the model breaks the rules (no lock is written)\n"),
    };
    for refusal in refusals {
        for (i, line) in refusal.lines().iter().enumerate() {
            let _ = writeln!(out, "{}{line}", if i == 0 { "  x " } else { "    " });
        }
    }
    out
}

/// `2 personal fields`, `4 fields`, `3 fields, 1 personal`, `no fields`.
fn summary(collection: &Collection) -> String {
    let count = collection.fields.len();
    let personal = collection
        .fields
        .values()
        .filter(|f| f.personal.is_some())
        .count();
    let fields = |n: usize| {
        if n == 1 {
            "1 field".to_owned()
        } else {
            format!("{n} fields")
        }
    };
    match (count, personal) {
        (0, _) => "no fields".into(),
        (n, 0) => fields(n),
        (n, p) if n == p => format!("{} personal {}", n, if n == 1 { "field" } else { "fields" }),
        (n, p) => format!("{}, {p} personal", fields(n)),
    }
}

/// The lines of an existing collection, in order: added fields, options and
/// targets; fields changed in place; computed fields; views; indexes.
fn collection_lines(name: &str, old: &Collection, new: &Collection) -> Vec<String> {
    let mut lines: Vec<(u8, String, String)> = Vec::new();
    let column = |field: &str| format!("{:<NAME$}", format!("{name}.{field}"));
    for (field_name, field) in &new.fields {
        let at = column(field_name);
        let mut add = |group: u8, line: String| lines.push((group, field_name.clone(), line));
        let Some(before) = old.fields.get(field_name) else {
            add(0, format!("+ {at} {}", describe(name, field)));
            continue;
        };
        if let (Some(was), Some(is)) = (before.kind.options(), field.kind.options())
            && is.len() > was.len()
        {
            add(0, format!("+ {at} options {}", is[was.len()..].join(", ")));
        }
        if let (Some(was), Some(is)) = (before.kind.targets(), field.kind.targets()) {
            let added: Vec<_> = is.iter().filter(|t| !was.contains(t)).cloned().collect();
            if !added.is_empty() {
                add(0, format!("+ {at} targets {}", added.join(", ")));
            }
        }
        if let (None, Some(deprecation)) = (&before.deprecated, &field.deprecated) {
            add(1, format!("~ {at} deprecated: {}", deprecation.reason));
        }
        if before.description != field.description {
            add(1, format!("~ {at} description"));
        }
    }
    for (group, kind, was, is) in [
        (2, "computed", &old.computed, &new.computed),
        (3, "view", &old.views, &new.views),
    ] {
        for added in is.keys().filter(|n| !was.contains_key(*n)) {
            lines.push((group, added.clone(), format!("+ {kind} {name}.{added}")));
        }
    }
    for (index_name, index) in &new.indexes {
        let line = |sign: char| {
            format!(
                "{sign} index {name}.{index_name} ({})",
                index_summary(index)
            )
        };
        match old.indexes.get(index_name) {
            None => lines.push((4, index_name.clone(), line('+'))),
            Some(was) if was.fields != index.fields || was.equal != index.equal => {
                lines.push((4, index_name.clone(), line('~')))
            }
            Some(_) => {}
        }
    }
    for removed in old.indexes.keys().filter(|n| !new.indexes.contains_key(*n)) {
        lines.push((4, removed.clone(), format!("- index {name}.{removed}")));
    }
    lines.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)));
    lines.into_iter().map(|(_, _, line)| line).collect()
}

/// A new field: its type, the collections it links to, whom it is about.
fn describe(collection: &str, field: &Field) -> String {
    let mut text = field.kind.describe();
    if let Kind::Link(targets) | Kind::Links(targets) = &field.kind {
        let _ = write!(text, " -> {}", targets.join(", "));
    }
    match &field.personal {
        Some(Person::Itself) => text.push_str(", personal (self)"),
        Some(Person::Link(link)) => {
            let _ = write!(text, ", personal to {collection}.{link}");
        }
        None => {}
    }
    text
}

/// `status, priority; equal 1` or `priority desc`.
fn index_summary(index: &Index) -> String {
    let fields: Vec<String> = index
        .fields
        .iter()
        .map(|f| match f.direction.as_str() {
            "asc" => f.field.clone(),
            direction => format!("{} {direction}", f.field),
        })
        .collect();
    let mut text = fields.join(", ");
    if index.equal > 0 {
        let _ = write!(text, "; equal {}", index.equal);
    }
    text
}
