//! The additive-change rule (RFC 0005, "The additive-change rule").
//!
//! [`validate`] checks what a lock must hold on its own: names, field types
//! and rule versions, link targets, options, personal fields and indexes.
//! [`successor`] checks a lock against the previous one, as `ncdb model
//! build` does against the lock in the repository and the SDK does on the
//! device against the lock it last opened the database with. Both list
//! every refusal they find, in the order of the names, so a developer sees
//! all of them at once.

use std::collections::BTreeMap;

use super::{
    BUILT_IN_RULE, Collection, Field, Kind, ModelLock, Person, Refusal, refusal::NameKind,
};

/// The longest name: a letter and 62 more letters or digits.
const MAX_NAME: usize = 63;

/// Whether a collection or field name matches `^[a-z][A-Za-z0-9]{0,62}$`.
pub fn is_valid_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    matches!(bytes.next(), Some(b'a'..=b'z'))
        && name.len() <= MAX_NAME
        && bytes.all(|b| b.is_ascii_alphanumeric())
}

/// What the lock breaks on its own.
pub fn validate(lock: &ModelLock) -> Vec<Refusal> {
    let mut found = Vec::new();
    let collections = lock
        .collections
        .keys()
        .map(|name| (NameKind::Collection, name));
    names(&mut found, None, collections);
    for (name, collection) in &lock.collections {
        validate_collection(&mut found, lock, name, collection);
    }
    found
}

/// What `next` breaks as a successor of `previous`: a lock of another
/// lineage, a lower model version, what `next` breaks on its own
/// ([`validate`]), and every change from `previous` that old clients could
/// not keep or new clients could not read.
pub fn successor(previous: &ModelLock, next: &ModelLock) -> Vec<Refusal> {
    if previous.model.id != next.model.id {
        return vec![Refusal::OtherModel {
            previous: previous.model.id,
            next: next.model.id,
        }];
    }
    let mut found = Vec::new();
    if next.model.version < previous.model.version {
        found.push(Refusal::LowerVersion {
            previous: previous.model.version,
            next: next.model.version,
        });
    }
    found.extend(validate(next));
    for (name, old) in &previous.collections {
        match next.collections.get(name) {
            None => found.push(Refusal::CollectionRemoved {
                collection: name.clone(),
            }),
            Some(new) => compare_collection(&mut found, name, old, new),
        }
    }
    found
}

/// Refuses names that break the naming rules, and names that collide, in
/// case, with an earlier name of the same scope.
fn names<'a>(
    found: &mut Vec<Refusal>,
    collection: Option<&String>,
    scope: impl Iterator<Item = (NameKind, &'a String)>,
) {
    let mut seen: Vec<(NameKind, &String)> = Vec::new();
    for (kind, name) in scope {
        if !is_valid_name(name) {
            found.push(Refusal::BadName {
                kind,
                collection: collection.cloned(),
                name: name.clone(),
            });
            continue;
        }
        if let Some((first_kind, first)) = seen.iter().find(|(_, s)| s.eq_ignore_ascii_case(name)) {
            found.push(Refusal::NameClash {
                collection: collection.cloned(),
                first: (*first_kind, (*first).clone()),
                second: (kind, name.clone()),
            });
        }
        seen.push((kind, name));
    }
}

fn validate_collection(
    found: &mut Vec<Refusal>,
    lock: &ModelLock,
    name: &String,
    collection: &Collection,
) {
    let scope = collection
        .fields
        .keys()
        .map(|n| (NameKind::Field, n))
        .chain(collection.computed.keys().map(|n| (NameKind::Computed, n)))
        .chain(collection.views.keys().map(|n| (NameKind::View, n)))
        .chain(collection.indexes.keys().map(|n| (NameKind::Index, n)));
    names(found, Some(name), scope);
    for (field_name, field) in &collection.fields {
        validate_field(found, lock, name, &collection.fields, field_name, field);
    }
    for (index_name, index) in &collection.indexes {
        let refuse = |field: &String, type_id: Option<&str>| match type_id {
            None => Refusal::IndexUnknownField {
                collection: name.clone(),
                index: index_name.clone(),
                field: field.clone(),
            },
            Some(type_id) => Refusal::IndexNotIndexable {
                collection: name.clone(),
                index: index_name.clone(),
                field: field.clone(),
                type_id: type_id.to_owned(),
            },
        };
        for entry in &index.fields {
            match collection.fields.get(&entry.field) {
                None => found.push(refuse(&entry.field, None)),
                Some(field) if !field.kind.is_indexable() => {
                    found.push(refuse(&entry.field, Some(field.kind.type_id())))
                }
                Some(_) => {}
            }
        }
        if index.equal > index.fields.len() as u64 {
            found.push(Refusal::IndexEqualTooLarge {
                collection: name.clone(),
                index: index_name.clone(),
                equal: index.equal,
                fields: index.fields.len(),
            });
        }
    }
}

fn validate_field(
    found: &mut Vec<Refusal>,
    lock: &ModelLock,
    collection: &str,
    fields: &BTreeMap<String, Field>,
    name: &str,
    field: &Field,
) {
    let c = || collection.to_owned();
    let f = || name.to_owned();
    match &field.kind {
        Kind::Plugin { type_id, .. } => {
            let namespace = type_id.split_once('/').map_or("", |(n, _)| n);
            if !lock.plugins.contains_key(namespace) {
                found.push(Refusal::UnknownPlugin {
                    collection: c(),
                    field: f(),
                    type_id: type_id.clone(),
                });
            }
        }
        kind if field.rule != BUILT_IN_RULE => found.push(Refusal::UnknownRule {
            collection: c(),
            field: f(),
            type_id: kind.type_id().to_owned(),
            rule: field.rule,
        }),
        _ => {}
    }
    for target in field.kind.targets().unwrap_or_default() {
        if !lock.collections.contains_key(target) {
            found.push(Refusal::UnknownTarget {
                collection: c(),
                field: f(),
                target: target.clone(),
            });
        }
    }
    let options = field.kind.options().unwrap_or_default();
    for (i, option) in options.iter().enumerate() {
        // Each duplicated option once, at its second occurrence.
        if options[..i].iter().filter(|o| *o == option).count() == 1 {
            found.push(Refusal::DuplicateOption {
                collection: c(),
                field: f(),
                option: option.clone(),
            });
        }
    }
    let Some(person) = &field.personal else {
        return;
    };
    if !field.kind.is_basic() {
        found.push(Refusal::PersonalNotBasic {
            collection: c(),
            field: f(),
            type_id: field.kind.type_id().to_owned(),
        });
    } else if let Person::Link(person) = person {
        match fields.get(person) {
            Some(link) if matches!(link.kind, Kind::Link(_)) => {
                if link.personal.is_some() {
                    found.push(Refusal::PersonLinkPersonal {
                        collection: c(),
                        field: f(),
                        person: person.clone(),
                    });
                }
            }
            _ => found.push(Refusal::PersonNotLink {
                collection: c(),
                field: f(),
                person: person.clone(),
            }),
        }
    }
}

fn compare_collection(found: &mut Vec<Refusal>, name: &str, old: &Collection, new: &Collection) {
    if old.deprecated.is_some() && new.deprecated.is_none() {
        found.push(Refusal::CollectionUndeprecated {
            collection: name.to_owned(),
        });
    }
    for (field_name, before) in &old.fields {
        let c = || name.to_owned();
        let f = || field_name.clone();
        let Some(after) = new.fields.get(field_name) else {
            let described = before.kind.describe();
            let mut renames = new.fields.iter().filter(|(n, field)| {
                !old.fields.contains_key(*n) && field.kind.describe() == described
            });
            let rename = match (renames.next(), renames.next()) {
                (Some((only, _)), None) => Some(only.clone()),
                _ => None,
            };
            found.push(Refusal::FieldRemoved {
                collection: c(),
                field: f(),
                rename,
            });
            continue;
        };
        compare_field(found, name, field_name, before, after);
    }
}

fn compare_field(
    found: &mut Vec<Refusal>,
    collection: &str,
    name: &str,
    before: &Field,
    after: &Field,
) {
    let c = || collection.to_owned();
    let f = || name.to_owned();
    let (from, to) = (before.kind.describe(), after.kind.describe());
    if from != to {
        found.push(Refusal::TypeChanged {
            collection: c(),
            field: f(),
            from,
            to,
        });
    } else {
        if before.rule != after.rule {
            found.push(Refusal::RuleChanged {
                collection: c(),
                field: f(),
                type_id: after.kind.type_id().to_owned(),
                from: before.rule,
                to: after.rule,
            });
        }
        if let (Some(old), Some(new)) = (before.kind.options(), after.kind.options())
            && !new.starts_with(old)
        {
            found.push(Refusal::OptionsChanged {
                collection: c(),
                field: f(),
                from: old.to_vec(),
                to: new.to_vec(),
            });
        }
        if let (Some(old), Some(new)) = (before.kind.targets(), after.kind.targets()) {
            for target in old.iter().filter(|t| !new.contains(t)) {
                found.push(Refusal::TargetRemoved {
                    collection: c(),
                    field: f(),
                    target: target.clone(),
                });
            }
        }
    }
    match (&before.personal, &after.personal) {
        (None, Some(_)) => found.push(Refusal::MarkedPersonal {
            collection: c(),
            field: f(),
        }),
        (Some(person), None) => found.push(Refusal::UnmarkedPersonal {
            collection: c(),
            field: f(),
            person: person.as_str().to_owned(),
        }),
        (Some(old), Some(new)) if old != new => found.push(Refusal::PersonChanged {
            collection: c(),
            field: f(),
            from: old.as_str().to_owned(),
            to: new.as_str().to_owned(),
        }),
        _ => {}
    }
    if before.deprecated.is_some() && after.deprecated.is_none() {
        found.push(Refusal::Undeprecated {
            collection: c(),
            field: f(),
        });
    }
}
