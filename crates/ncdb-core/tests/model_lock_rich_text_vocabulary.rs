//! A rich-text field's vocabulary in `model.lock.json` (RFC 0013, "The
//! field in the schema and the lock"; RFC 0005, "The additive-change
//! rule"; NOC-553): the names of the nodes and marks its editor uses, kept
//! sorted and without duplicates, left out when empty, and covered by the
//! model hash. A later lock may add names; removing one is refused, by
//! `ncdb model build` and by the check on the device alike.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::BTreeSet;

use ncdb_core::model_lock::{Draft, ModelId, ModelLock, Refusal, Vocabulary, build, check};

/// Notes with a rich-text body, whose entry is `{BODY}`.
const LOCK: &str = r#"{
  "collections": {
    "notes": {
      "fields": {
        "body": {BODY},
        "title": { "rule": 1, "since": 1, "type": "ncdb/string" }
      },
      "since": 1
    }
  },
  "format": 1,
  "model": { "hash": "sha256:00", "id": "000102030405060708090a0b0c0d0e0f", "version": 1 },
  "plugins": {}
}"#;

/// The body's entry with this vocabulary, or without one.
fn entry(vocabulary: Option<&str>) -> String {
    match vocabulary {
        None => r#"{ "rule": 1, "since": 1, "type": "ncdb/richText" }"#.to_owned(),
        Some(v) => {
            format!(r#"{{ "rule": 1, "since": 1, "type": "ncdb/richText", "vocabulary": {v} }}"#)
        }
    }
}

/// The lock with the body's vocabulary, at `version`, with its hash.
fn lock(version: u64, vocabulary: Option<&str>) -> ModelLock {
    let mut lock =
        ModelLock::from_json(&LOCK.replace("{BODY}", &entry(vocabulary))).expect("the lock reads");
    lock.model.version = version;
    lock.model.hash = lock.compute_hash();
    lock
}

fn names(items: &[&str]) -> BTreeSet<String> {
    items.iter().map(|s| (*s).to_owned()).collect()
}

fn vocabulary(lock: &ModelLock) -> &Vocabulary {
    &lock.field("notes", "body").unwrap().vocabulary
}

const CALLOUT: &str = r#"{ "marks": ["link", "bold"], "nodes": ["paragraph", "callout", "heading"] }"#;

#[test]
fn reads_the_nodes_and_marks_as_sorted_sets() {
    let lock = lock(1, Some(CALLOUT));
    assert_eq!(
        vocabulary(&lock).nodes,
        names(&["callout", "heading", "paragraph"])
    );
    assert_eq!(vocabulary(&lock).marks, names(&["bold", "link"]));
}

#[test]
fn a_field_without_the_key_has_an_empty_vocabulary() {
    let lock = lock(1, None);
    assert!(vocabulary(&lock).is_empty());
    assert_eq!(vocabulary(&lock), &Vocabulary::default());
}

#[test]
fn a_missing_list_is_empty() {
    let lock = lock(1, Some(r#"{ "nodes": ["paragraph"] }"#));
    assert_eq!(vocabulary(&lock).nodes, names(&["paragraph"]));
    assert!(vocabulary(&lock).marks.is_empty());
}

#[test]
fn writes_each_list_sorted_without_duplicates() {
    let lock = lock(
        1,
        Some(r#"{ "marks": ["link", "bold", "link"], "nodes": ["paragraph", "callout"] }"#),
    );
    assert_eq!(
        lock.to_json()["collections"]["notes"]["fields"]["body"]["vocabulary"],
        serde_json::json!({ "marks": ["bold", "link"], "nodes": ["callout", "paragraph"] })
    );
}

#[test]
fn writes_no_key_for_an_empty_vocabulary() {
    for empty in [None, Some(r#"{ "marks": [], "nodes": [] }"#)] {
        let lock = lock(1, empty);
        assert_eq!(
            lock.to_json()["collections"]["notes"]["fields"]["body"],
            serde_json::json!({ "rule": 1, "since": 1, "type": "ncdb/richText" })
        );
    }
}

#[test]
fn the_model_hash_covers_the_vocabulary() {
    let without = lock(1, None);
    let with = lock(1, Some(CALLOUT));
    assert_ne!(without.model.hash, with.model.hash);
    let reordered = lock(
        1,
        Some(r#"{ "marks": ["bold", "link"], "nodes": ["heading", "callout", "paragraph"] }"#),
    );
    assert_eq!(with.model.hash, reordered.model.hash);
}

#[test]
fn a_vocabulary_of_the_wrong_shape_names_its_path() {
    for (vocabulary, path) in [
        (r#"["paragraph"]"#, "collections.notes.fields.body.vocabulary"),
        (
            r#"{ "nodes": "paragraph" }"#,
            "collections.notes.fields.body.vocabulary.nodes",
        ),
        (
            r#"{ "marks": [1] }"#,
            "collections.notes.fields.body.vocabulary.marks.0",
        ),
    ] {
        let error = ModelLock::from_json(&LOCK.replace("{BODY}", &entry(Some(vocabulary))))
            .expect_err(vocabulary);
        assert_eq!(error.path, path, "{vocabulary}");
    }
}

#[test]
fn the_key_on_a_field_of_another_type_is_ignored() {
    let text = LOCK.replace(
        "{BODY}",
        r#"{ "rule": 1, "since": 1, "type": "ncdb/string", "vocabulary": { "nodes": ["paragraph"] } }"#,
    );
    let lock = ModelLock::from_json(&text).expect("an unknown key is ignored");
    assert!(vocabulary(&lock).is_empty());
}

#[test]
fn adding_nodes_and_marks_is_accepted() {
    let before = lock(1, Some(r#"{ "marks": ["bold"], "nodes": ["paragraph"] }"#));
    let after = lock(2, Some(CALLOUT));
    assert_eq!(check::successor(&before, &after), vec![]);
}

#[test]
fn declaring_a_vocabulary_where_there_was_none_is_accepted() {
    assert_eq!(check::successor(&lock(1, None), &lock(2, Some(CALLOUT))), vec![]);
}

#[test]
fn removing_a_node_is_refused() {
    let before = lock(1, Some(CALLOUT));
    let after = lock(
        2,
        Some(r#"{ "marks": ["bold", "link"], "nodes": ["heading", "paragraph"] }"#),
    );
    assert_eq!(
        check::successor(&before, &after),
        vec![Refusal::NodeRemoved {
            collection: "notes".into(),
            field: "body".into(),
            node: "callout".into(),
        }]
    );
}

#[test]
fn removing_a_mark_is_refused() {
    let before = lock(1, Some(CALLOUT));
    let after = lock(
        2,
        Some(r#"{ "marks": ["bold"], "nodes": ["callout", "heading", "paragraph"] }"#),
    );
    assert_eq!(
        check::successor(&before, &after),
        vec![Refusal::MarkRemoved {
            collection: "notes".into(),
            field: "body".into(),
            mark: "link".into(),
        }]
    );
}

#[test]
fn dropping_the_vocabulary_refuses_each_name() {
    let refusals = check::successor(&lock(1, Some(CALLOUT)), &lock(2, None));
    assert_eq!(refusals.len(), 5, "{refusals:?}");
    assert!(refusals.iter().all(|r| matches!(
        r,
        Refusal::NodeRemoved { .. } | Refusal::MarkRemoved { .. }
    )));
}

#[test]
fn a_removal_says_what_changed_why_and_what_to_do() {
    let node = Refusal::NodeRemoved {
        collection: "notes".into(),
        field: "body".into(),
        node: "callout".into(),
    };
    let lines = node.lines();
    assert_eq!(lines.len(), 3, "{lines:?}");
    assert!(lines[0].starts_with("notes.body: "), "{lines:?}");
    assert!(lines[0].contains("callout"), "{lines:?}");
    let mark = Refusal::MarkRemoved {
        collection: "notes".into(),
        field: "body".into(),
        mark: "link".into(),
    };
    assert!(mark.lines()[0].contains("link"), "{:?}", mark.lines());
    assert_ne!(node.lines()[0], mark.lines()[0].replace("link", "callout"));
}

/// The schema's description of the notes, with the body's `vocabulary`.
fn draft(vocabulary: Option<&str>) -> Draft {
    let body = match vocabulary {
        None => r#"{ "rule": 1, "type": "ncdb/richText" }"#.to_owned(),
        Some(v) => format!(r#"{{ "rule": 1, "type": "ncdb/richText", "vocabulary": {v} }}"#),
    };
    Draft::from_json(&format!(
        r#"{{ "collections": {{ "notes": {{ "fields": {{ "body": {body} }}, "computed": [], "views": [], "indexes": {{}} }} }}, "plugins": {{}} }}"#
    ))
    .expect("the description reads")
}

const ID: ModelId = ModelId([7; 16]);

#[test]
fn model_build_writes_the_vocabulary_and_a_new_version_when_it_grows() {
    let first = build(
        &draft(Some(r#"{ "marks": ["bold"], "nodes": ["paragraph"] }"#)),
        None,
        || ID,
    )
    .unwrap()
    .lock;
    assert_eq!(
        first.to_json()["collections"]["notes"]["fields"]["body"]["vocabulary"],
        serde_json::json!({ "marks": ["bold"], "nodes": ["paragraph"] })
    );
    let second = build(&draft(Some(CALLOUT)), Some(&first), || unreachable!())
        .expect("adding names is additive")
        .lock;
    assert_eq!(second.model.version, 2);
    assert_eq!(second.field("notes", "body").unwrap().since, 1);
    assert_eq!(vocabulary(&second).nodes.len(), 3);
}

#[test]
fn model_build_keeps_the_version_when_only_the_order_changes() {
    let first = build(&draft(Some(CALLOUT)), None, || ID).unwrap().lock;
    let again = build(
        &draft(Some(
            r#"{ "marks": ["bold", "link", "bold"], "nodes": ["heading", "paragraph", "callout"] }"#,
        )),
        Some(&first),
        || unreachable!(),
    )
    .unwrap()
    .lock;
    assert_eq!(again, first);
}

#[test]
fn model_build_refuses_a_removed_name_and_builds_nothing() {
    let first = build(&draft(Some(CALLOUT)), None, || ID).unwrap().lock;
    let refusals = build(
        &draft(Some(r#"{ "marks": ["bold", "link"], "nodes": ["paragraph"] }"#)),
        Some(&first),
        || unreachable!(),
    )
    .expect_err("a removed node is refused");
    assert_eq!(
        refusals,
        vec![
            Refusal::NodeRemoved {
                collection: "notes".into(),
                field: "body".into(),
                node: "callout".into(),
            },
            Refusal::NodeRemoved {
                collection: "notes".into(),
                field: "body".into(),
                node: "heading".into(),
            },
        ]
    );
}
