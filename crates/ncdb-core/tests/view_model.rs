//! The view's model read from `model.lock.json` (NOC-77; RFC 0005, "the
//! core and both SDKs read only the lock"): each collection with its
//! fields and their field types, and each collection's declared indexes,
//! whose names are scoped per collection as the lock scopes them.
//!
//! Native only: the view is not built on wasm32 yet.
#![cfg(not(target_arch = "wasm32"))]

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::BTreeMap;

use ncdb_core::basic::{BasicValue, Kind};
use ncdb_core::link::Link;
use ncdb_core::model_lock::{self, ModelLock, Refusal};
use ncdb_core::set::Set;
use ncdb_core::view::{Direction, Field, Index, Invalid, Model, Scope};

/// The lock of the view's tests, as `ncdb model build` wrote it from
/// `tests/view/model.ts`.
const LOCK: &str = include_str!("view/model.lock.json");

fn lock() -> ModelLock {
    ModelLock::from_json(LOCK).unwrap()
}

fn index(collection: &str, name: &str, keys: &[(&str, Direction)], equality: usize) -> Index {
    Index {
        name: name.to_owned(),
        collection: collection.to_owned(),
        keys: keys.iter().map(|(k, d)| ((*k).to_owned(), *d)).collect(),
        scope: Scope::Member,
        equality,
    }
}

#[test]
fn the_view_model_holds_the_collections_fields_and_indexes_of_the_lock() {
    let text = || Field::Basic(BasicValue { kind: Kind::Text });
    let task: BTreeMap<String, Field> = [
        ("title", text()),
        (
            "rank",
            Field::Basic(BasicValue {
                kind: Kind::Integer,
            }),
        ),
        ("tags", Field::Set(Set { kind: Kind::Text })),
        (
            "owner",
            Field::Link(Link {
                collections: ["project".to_owned()].into(),
            }),
        ),
    ]
    .into_iter()
    .map(|(name, field)| (name.to_owned(), field))
    .collect();
    let project = [("name".to_owned(), text())].into();
    use Direction::{Ascending, Descending};
    let expected = Model {
        collections: [("task".to_owned(), task), ("project".to_owned(), project)].into(),
        indexes: vec![
            index("project", "byName", &[("name", Ascending)], 0),
            index("task", "byName", &[("title", Ascending)], 0),
            index("task", "byRank", &[("rank", Ascending)], 0),
            index(
                "task",
                "byTitle",
                &[("title", Ascending), ("rank", Descending)],
                1,
            ),
        ],
    };
    assert_eq!(Model::from_lock(&lock()), Ok(expected));
}

#[test]
fn every_built_in_field_type_has_its_field_type_in_the_view() {
    let mut lock = lock();
    let fields = &mut lock.collections.get_mut("task").unwrap().fields;
    let template = fields["rank"].clone();
    let kinds = [
        (
            "f",
            model_lock::Kind::Float,
            Field::Basic(BasicValue { kind: Kind::Float }),
        ),
        (
            "b",
            model_lock::Kind::Boolean,
            Field::Basic(BasicValue {
                kind: Kind::Boolean,
            }),
        ),
        (
            "c",
            model_lock::Kind::Choice(vec!["open".into(), "done".into()]),
            Field::Basic(BasicValue {
                kind: Kind::Choice(["done".to_owned(), "open".to_owned()].into()),
            }),
        ),
        (
            "s",
            model_lock::Kind::Set(model_lock::Element::Choice(vec!["x".into()])),
            Field::Set(Set {
                kind: Kind::Choice(["x".to_owned()].into()),
            }),
        ),
        (
            "l",
            model_lock::Kind::Links(vec!["task".into(), "project".into()]),
            Field::Links(ncdb_core::link::Links {
                collections: ["project".to_owned(), "task".to_owned()].into(),
            }),
        ),
    ];
    for (name, kind, _) in &kinds {
        let mut field = template.clone();
        field.kind = kind.clone();
        fields.insert((*name).to_owned(), field);
    }
    let model = Model::from_lock(&lock).unwrap();
    for (name, _, expected) in kinds {
        assert_eq!(model.field("task", name), Some(&expected), "{name}");
    }
}

#[test]
fn two_collections_each_have_their_own_index_of_the_same_name() {
    let model = Model::from_lock(&lock()).unwrap();
    let task = model.index("task", "byName").unwrap();
    let project = model.index("project", "byName").unwrap();
    assert_eq!(task.keys, [("title".to_owned(), Direction::Ascending)]);
    assert_eq!(project.keys, [("name".to_owned(), Direction::Ascending)]);
    assert_eq!(model.index("task", "byRank").unwrap().collection, "task");
    assert_eq!(model.index("project", "byRank"), None);
    assert_eq!(model.validate(), Ok(()));
}

#[test]
fn two_indexes_of_one_collection_may_not_share_a_name() {
    let mut model = Model::from_lock(&lock()).unwrap();
    let mut twin = model.index("task", "byRank").unwrap().clone();
    twin.keys.reverse();
    model.indexes.push(twin);
    assert_eq!(
        model.validate(),
        Err(Invalid::DuplicateIndex {
            collection: "task".into(),
            index: "byRank".into(),
        })
    );
}

#[test]
fn a_plugin_field_has_no_field_type_in_the_view_so_its_writes_park() {
    let mut lock = lock();
    let mut field = lock.field("task", "rank").unwrap().clone();
    field.kind = model_lock::Kind::Plugin {
        type_id: "acme/money".into(),
        config: None,
    };
    lock.plugins
        .insert("acme".into(), model_lock::Plugin::default());
    let task = lock.collections.get_mut("task").unwrap();
    task.fields.insert("price".into(), field);
    let model = Model::from_lock(&lock).unwrap();
    assert_eq!(model.field("task", "price"), None);
    assert!(model.field("task", "rank").is_some());
}

#[test]
fn a_lock_that_breaks_the_rules_of_rfc_0005_is_refused_with_its_refusals() {
    let mut lock = lock();
    let task = lock.collections.get_mut("task").unwrap();
    task.indexes.get_mut("byRank").unwrap().fields[0].field = "nope".into();
    let refusals: Vec<Refusal> = model_lock::check::validate(&lock);
    assert!(!refusals.is_empty());
    assert_eq!(Model::from_lock(&lock), Err(Invalid::Lock(refusals)));
}
