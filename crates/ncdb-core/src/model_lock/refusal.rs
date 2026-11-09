//! A change the additive-change rule refuses, with its message (RFC 0005,
//! "Refused" and "An example of each").
//!
//! Each message says what changed, why it is refused, and what to do
//! instead, one line each. `ncdb model build` prints them under the lock's
//! name; the device check names them in its error.

use std::fmt;

use super::ModelId;

/// What kind of name a name in a collection's scope is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NameKind {
    /// A collection, in the model's scope.
    Collection,
    /// A stored field.
    Field,
    /// A computed field.
    Computed,
    /// A view.
    View,
    /// An index.
    Index,
}

impl NameKind {
    fn as_str(self) -> &'static str {
        match self {
            NameKind::Collection => "collection",
            NameKind::Field => "field",
            NameKind::Computed => "computed field",
            NameKind::View => "view",
            NameKind::Index => "index",
        }
    }
}

/// A refused change, or a model that breaks a rule on its own. Every
/// `collection`, `field` and `index` is a name; `type_id`s, `from` and `to`
/// are what the lock writes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// A lock of another lineage.
    OtherModel { previous: ModelId, next: ModelId },
    /// A lock with a lower model version than the previous one.
    LowerVersion { previous: u64, next: u64 },
    /// A collection that the previous lock has and the next one does not.
    CollectionRemoved { collection: String },
    /// A deprecated collection that is no longer deprecated.
    CollectionUndeprecated { collection: String },
    /// A field that the previous lock has and the next one does not.
    /// `rename` is the one new field of the same type in the collection,
    /// when there is exactly one.
    FieldRemoved {
        collection: String,
        field: String,
        rename: Option<String>,
    },
    /// A field whose type changes (for a set: its element's type).
    TypeChanged {
        collection: String,
        field: String,
        from: String,
        to: String,
    },
    /// A field that moves to another rule version of its type.
    RuleChanged {
        collection: String,
        field: String,
        type_id: String,
        from: u64,
        to: u64,
    },
    /// Options of a choice, or a set of choices, that are not the previous
    /// options with new ones appended.
    OptionsChanged {
        collection: String,
        field: String,
        from: Vec<String>,
        to: Vec<String>,
    },
    /// A collection removed from a link's targets.
    TargetRemoved {
        collection: String,
        field: String,
        target: String,
    },
    /// An existing field marked personal.
    MarkedPersonal { collection: String, field: String },
    /// A personal field that is no longer personal.
    UnmarkedPersonal {
        collection: String,
        field: String,
        person: String,
    },
    /// A personal field whose person changes.
    PersonChanged {
        collection: String,
        field: String,
        from: String,
        to: String,
    },
    /// A deprecated field that is no longer deprecated.
    Undeprecated { collection: String, field: String },
    /// A name that breaks the naming rules. `collection` is `None` for a
    /// collection's own name.
    BadName {
        kind: NameKind,
        collection: Option<String>,
        name: String,
    },
    /// Two names in one scope that are equal, or differ only in case.
    NameClash {
        collection: Option<String>,
        first: (NameKind, String),
        second: (NameKind, String),
    },
    /// A built-in field type with a rule version this core does not know.
    UnknownRule {
        collection: String,
        field: String,
        type_id: String,
        rule: u64,
    },
    /// A plugin field type whose namespace is not in `plugins`.
    UnknownPlugin {
        collection: String,
        field: String,
        type_id: String,
    },
    /// A link target that is no collection of the model.
    UnknownTarget {
        collection: String,
        field: String,
        target: String,
    },
    /// An option listed twice.
    DuplicateOption {
        collection: String,
        field: String,
        option: String,
    },
    /// A personal field of a type that cannot be personal.
    PersonalNotBasic {
        collection: String,
        field: String,
        type_id: String,
    },
    /// A personal field whose person is no link field of its collection.
    PersonNotLink {
        collection: String,
        field: String,
        person: String,
    },
    /// A personal field whose person's link field is itself personal.
    PersonLinkPersonal {
        collection: String,
        field: String,
        person: String,
    },
    /// An index on a field it names but the collection does not have.
    IndexUnknownField {
        collection: String,
        index: String,
        field: String,
    },
    /// An index on a field of a type that cannot be indexed.
    IndexNotIndexable {
        collection: String,
        index: String,
        field: String,
        type_id: String,
    },
    /// An index whose `equal` is larger than its number of fields.
    IndexEqualTooLarge {
        collection: String,
        index: String,
        equal: u64,
        fields: usize,
    },
}

impl Refusal {
    /// The message, one line each: what changed, why it is refused, and
    /// (for most) what to do instead.
    pub fn lines(&self) -> Vec<String> {
        use Refusal::*;
        let lines: Vec<String> = match self {
            OtherModel { previous, next } => vec![
                format!("model.id: {next} is not {previous}."),
                "The lock belongs to another model; it is not a successor of the previous one.".into(),
            ],
            LowerVersion { previous, next } => vec![
                format!("model.version: {next} is lower than {previous}."),
                "The lock is older than the previous one; it is not its successor.".into(),
            ],
            CollectionRemoved { collection: c } => vec![
                format!("collection {c}: removed. Changes name collections; data in {c} would belong to nothing."),
                "To stop using it, keep it in the schema. To rename it, add the collection under its new name.".into(),
            ],
            CollectionUndeprecated { collection: c } => vec![
                format!("collection {c}: no longer deprecated."),
                "Clients that stopped writing it would not write it again.".into(),
                "Keep it deprecated, and add a new collection instead.".into(),
            ],
            FieldRemoved { collection: c, field: f, rename } => vec![
                format!("{c}.{f}: removed. Changes name fields; data in {c}.{f} would belong to nothing."),
                format!(
                    "To stop using it, keep it in the schema with .deprecated(\"...\"). To rename it, add {}.",
                    match rename {
                        Some(new) => format!("{c}.{new}"),
                        None => "a field under the new name".into(),
                    }
                ),
            ],
            TypeChanged { collection: c, field: f, from, to } => vec![
                format!("{c}.{f}: type changes from {from} to {to}."),
                "Clients would merge and read existing values differently.".into(),
                format!("Add a new field instead, and deprecate {c}.{f}."),
            ],
            RuleChanged { collection: c, field: f, type_id, from, to } => vec![
                format!("{c}.{f}: the rule version of {type_id} changes from {from} to {to}."),
                "A new rule version needs a migration that every member runs first, and there is none yet.".into(),
                format!("Add a new field with rule version {to} instead, and deprecate {c}.{f}."),
            ],
            OptionsChanged { collection: c, field: f, from, to } => vec![
                format!("{c}.{f}: options change from [{}] to [{}].", from.join(", "), to.join(", ")),
                "Stored values name options, and their order may become sort order.".into(),
                "Keep every option where it is, and append new options at the end.".into(),
            ],
            TargetRemoved { collection: c, field: f, target: t } => vec![
                format!("{c}.{f}: target {t} removed."),
                format!("Existing links into {t} would read as wrong collection on upgraded clients only."),
                format!("Keep {t} in the targets."),
            ],
            MarkedPersonal { collection: c, field: f } => vec![
                format!("{c}.{f}: marked personal."),
                "Its earlier values are in the log under the space key only; erasing a person would not reach them.".into(),
                format!("Add a new personal field instead, and deprecate {c}.{f}."),
            ],
            UnmarkedPersonal { collection: c, field: f, person } => vec![
                format!("{c}.{f}: no longer personal."),
                "Old values stay sealed under person keys and new ones would not be: the field would be half erasable.".into(),
                format!("Keep .personal({{ person: \"{person}\" }})."),
            ],
            PersonChanged { collection: c, field: f, from, to } => vec![
                format!("{c}.{f}: its person changes from {from} to {to}."),
                "Old values stay sealed under the keys of the old person: the field would be half erasable.".into(),
                format!("Keep .personal({{ person: \"{from}\" }}), or add a new personal field."),
            ],
            Undeprecated { collection: c, field: f } => vec![
                format!("{c}.{f}: no longer deprecated."),
                "Clients that stopped writing it would not write it again.".into(),
                "Keep it deprecated, and add a new field instead.".into(),
            ],
            BadName { kind, collection, name } => vec![
                format!("{} {}: the name breaks the naming rules.", kind.as_str(), scoped(collection, name)),
                "A name starts with a lower-case letter and has at most 63 ASCII letters and digits.".into(),
            ],
            NameClash { collection, first, second } => vec![
                format!(
                    "{} {}: the name collides with the {} {}.",
                    second.0.as_str(),
                    scoped(collection, &second.1),
                    first.0.as_str(),
                    scoped(collection, &first.1),
                ),
                "Names in one scope must differ in more than case: the local view is SQLite, whose column names ignore case.".into(),
            ],
            UnknownRule { collection: c, field: f, type_id, rule } => vec![
                format!("{c}.{f}: {type_id} has no rule version {rule}."),
                format!("This version knows rule version {} of every built-in type.", super::BUILT_IN_RULE),
            ],
            UnknownPlugin { collection: c, field: f, type_id } => vec![
                format!("{c}.{f}: {type_id} names no plugin of the model."),
                "A plugin's field type needs its namespace in plugins.".into(),
            ],
            UnknownTarget { collection: c, field: f, target: t } => vec![
                format!("{c}.{f}: target {t} is no collection of the model."),
            ],
            DuplicateOption { collection: c, field: f, option } => vec![
                format!("{c}.{f}: option {option} is listed twice."),
            ],
            PersonalNotBasic { collection: c, field: f, type_id } => vec![
                format!("{c}.{f}: {type_id} cannot be personal."),
                "Only basic values can be personal: nc.string, nc.int, nc.float, nc.boolean and nc.oneOf.".into(),
            ],
            PersonNotLink { collection: c, field: f, person } => vec![
                format!("{c}.{f}: personal to {person}, which is no link field of {c}."),
                format!("person is \"self\" or the name of an nc.link field of {c}."),
            ],
            PersonLinkPersonal { collection: c, field: f, person } => vec![
                format!("{c}.{f}: personal to {c}.{person}, which is itself personal."),
                "The link that names the person must stay readable, so the SDK can find the person's key.".into(),
            ],
            IndexUnknownField { collection: c, index, field: f } => vec![
                format!("index {c}.{index}: {c} has no field {f}."),
            ],
            IndexNotIndexable { collection: c, index, field: f, type_id } => vec![
                format!("index {c}.{index}: {c}.{f} is {type_id}, which cannot be indexed."),
                "An index can use basic values and single links (nc.link).".into(),
            ],
            IndexEqualTooLarge { collection: c, index, equal, fields } => vec![
                format!("index {c}.{index}: equal is {equal}, but the index has {fields} fields."),
                "No query could fix more fields than the index has.".into(),
            ],
        };
        lines
    }
}

fn scoped(collection: &Option<String>, name: &str) -> String {
    match collection {
        Some(c) => format!("{c}.{name}"),
        None => name.to_owned(),
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.lines().join("\n"))
    }
}
