//! A change the additive-change rule refuses, with its message (RFC 0005,
//! "Refused" and "An example of each").
//!
//! Each message says what changed, why it is refused, and what to do
//! instead, one line each. `ncdb model build` prints them under the lock's
//! name; the device check names them in its error.

use std::fmt;

use super::ModelId;
use crate::texts;

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
            NameKind::Collection => texts::NAME_KIND_COLLECTION,
            NameKind::Field => texts::NAME_KIND_FIELD,
            NameKind::Computed => texts::NAME_KIND_COMPUTED,
            NameKind::View => texts::NAME_KIND_VIEW,
            NameKind::Index => texts::NAME_KIND_INDEX,
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
    /// A plugin field whose `config` changes, appears or disappears: the
    /// config is part of the type's merge rule (RFC 0011, "A type that is
    /// not registered"; RFC 0005, "Refused"; NOC-251).
    ConfigChanged {
        collection: String,
        field: String,
        type_id: String,
    },
    /// Options of a choice, or a set of choices, that are not the previous
    /// options with new ones appended.
    OptionsChanged {
        collection: String,
        field: String,
        from: Vec<String>,
        to: Vec<String>,
    },
    /// A collection removed from the targets of a link or a reference.
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
    /// (for most) what to do instead. Its text is the refusal's entry of
    /// texts/texts.json (NOC-329).
    pub fn lines(&self) -> Vec<String> {
        use Refusal::*;
        let text = match self {
            OtherModel { previous, next } => texts::refusal_other_model(next, previous),
            LowerVersion { previous, next } => texts::refusal_lower_version(next, previous),
            CollectionRemoved { collection } => texts::refusal_collection_removed(collection),
            CollectionUndeprecated { collection } => {
                texts::refusal_collection_undeprecated(collection)
            }
            FieldRemoved {
                collection: c,
                field,
                rename,
            } => texts::refusal_field_removed(
                c,
                field,
                match rename {
                    Some(new) => format!("{c}.{new}"),
                    None => texts::REFUSAL_NO_RENAME.into(),
                },
            ),
            TypeChanged {
                collection,
                field,
                from,
                to,
            } => texts::refusal_type_changed(collection, field, from, to),
            RuleChanged {
                collection,
                field,
                type_id,
                from,
                to,
            } => texts::refusal_rule_changed(collection, field, type_id, from, to),
            ConfigChanged {
                collection,
                field,
                type_id,
            } => texts::refusal_config_changed(collection, field, type_id),
            OptionsChanged {
                collection,
                field,
                from,
                to,
            } => texts::refusal_options_changed(collection, field, from.join(", "), to.join(", ")),
            TargetRemoved {
                collection,
                field,
                target,
            } => texts::refusal_target_removed(collection, field, target),
            MarkedPersonal { collection, field } => {
                texts::refusal_marked_personal(collection, field)
            }
            UnmarkedPersonal {
                collection,
                field,
                person,
            } => texts::refusal_unmarked_personal(collection, field, person),
            PersonChanged {
                collection,
                field,
                from,
                to,
            } => texts::refusal_person_changed(collection, field, from, to),
            Undeprecated { collection, field } => texts::refusal_undeprecated(collection, field),
            BadName {
                kind,
                collection,
                name,
            } => texts::refusal_bad_name(kind.as_str(), scoped(collection, name)),
            NameClash {
                collection,
                first,
                second,
            } => texts::refusal_name_clash(
                second.0.as_str(),
                scoped(collection, &second.1),
                first.0.as_str(),
                scoped(collection, &first.1),
            ),
            UnknownRule {
                collection,
                field,
                type_id,
                rule,
            } => {
                texts::refusal_unknown_rule(collection, field, type_id, rule, super::BUILT_IN_RULE)
            }
            UnknownPlugin {
                collection,
                field,
                type_id,
            } => texts::refusal_unknown_plugin(collection, field, type_id),
            UnknownTarget {
                collection,
                field,
                target,
            } => texts::refusal_unknown_target(collection, field, target),
            DuplicateOption {
                collection,
                field,
                option,
            } => texts::refusal_duplicate_option(collection, field, option),
            PersonalNotBasic {
                collection,
                field,
                type_id,
            } => texts::refusal_personal_not_basic(collection, field, type_id),
            PersonNotLink {
                collection,
                field,
                person,
            } => texts::refusal_person_not_link(collection, field, person),
            PersonLinkPersonal {
                collection,
                field,
                person,
            } => texts::refusal_person_link_personal(collection, field, person),
            IndexUnknownField {
                collection,
                index,
                field,
            } => texts::refusal_index_unknown_field(collection, index, field),
            IndexNotIndexable {
                collection,
                index,
                field,
                type_id,
            } => texts::refusal_index_not_indexable(collection, index, field, type_id),
            IndexEqualTooLarge {
                collection,
                index,
                equal,
                fields,
            } => texts::refusal_index_equal_too_large(collection, index, equal, fields),
        };
        text.lines().map(String::from).collect()
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
