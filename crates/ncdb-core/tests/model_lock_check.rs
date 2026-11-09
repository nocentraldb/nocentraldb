//! The additive-change rule (RFC 0005, "The additive-change rule"): each
//! accepted change passes, and each refused change is refused with its
//! message.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::model_lock::{ModelId, ModelLock, NameKind, Refusal, check};

/// A lock at `version` with these collections, written as JSON.
fn lock(version: u64, collections: &str) -> ModelLock {
    let text = format!(
        r#"{{ "collections": {collections}, "format": 1, "plugins": {{ "acme": {{ "npm": "@acme/x" }} }},
            "model": {{ "hash": "sha256:00", "id": "6f1c0e2a9b7d4c3e8a5f0b1d2c3e4f5a", "version": {version} }} }}"#
    );
    ModelLock::from_json(&text).unwrap()
}

/// Version 1 of the task board in RFC 0005.
const V1: &str = r#"{
  "tasks": {
    "since": 1,
    "fields": {
      "title": { "rule": 1, "since": 1, "type": "ncdb/string" },
      "priority": { "rule": 1, "since": 1, "type": "ncdb/int" },
      "status": { "options": ["open", "in_progress", "done"], "rule": 1, "since": 1, "type": "ncdb/choice" },
      "dependsOn": { "rule": 1, "since": 1, "targets": ["tasks"], "type": "ncdb/links" },
      "owner": { "rule": 1, "since": 1, "targets": ["people"], "type": "ncdb/link" },
      "email": { "personal": { "person": "owner" }, "rule": 1, "since": 1, "type": "ncdb/string" },
      "old": { "deprecated": 1, "deprecatedReason": "Unused.", "rule": 1, "since": 1, "type": "ncdb/boolean" },
      "kinds": { "of": { "options": ["bug"], "type": "ncdb/choice" }, "rule": 1, "since": 1, "type": "ncdb/set" },
      "votes": { "rule": 1, "since": 1, "type": "acme/counter" }
    },
    "computed": { "blocked": { "since": 1 } },
    "views": { "ready": { "since": 1 } },
    "indexes": {
      "byStatusPriority": { "equal": 1, "fields": [{ "direction": "asc", "field": "status" }, { "direction": "asc", "field": "priority" }], "since": 1 }
    }
  },
  "people": { "since": 1, "fields": { "name": { "personal": { "person": "self" }, "rule": 1, "since": 1, "type": "ncdb/string" } } },
  "teams": { "since": 1, "fields": {} }
}"#;

/// V1 with `edit` applied to its text, at version 2.
fn v2(edit: impl Fn(&str) -> String) -> ModelLock {
    lock(2, &edit(V1))
}

fn refusals(next: &ModelLock) -> Vec<Refusal> {
    check::successor(&lock(1, V1), next)
}

fn s(text: &str) -> String {
    text.to_owned()
}

#[test]
fn version_1_of_the_task_board_is_a_sound_model() {
    assert_eq!(check::validate(&lock(1, V1)), vec![]);
    assert_eq!(refusals(&lock(1, V1)), vec![]);
}

#[test]
fn accepts_each_additive_change() {
    let accepted = [
        // Add a collection, a field, a personal field, a plugin type field.
        v2(|t| {
            t.replace(
                r#""teams": { "since": 1, "fields": {} }"#,
                r#""teams": { "since": 1, "fields": {} }, "goals": { "since": 2, "fields": {} }"#,
            )
        }),
        v2(|t| {
            t.replace(
                r#""title": {"#,
                r#""archived": { "rule": 1, "since": 2, "type": "ncdb/boolean" }, "title": {"#,
            )
        }),
        v2(|t| {
            t.replace(r#""title": {"#, r#""notes": { "personal": { "person": "owner" }, "rule": 1, "since": 2, "type": "ncdb/string" }, "title": {"#)
        }),
        v2(|t| {
            t.replace(
                r#""title": {"#,
                r#""score": { "rule": 3, "since": 2, "type": "acme/score" }, "title": {"#,
            )
        }),
        // Append an option to a choice, and to a set of choices.
        v2(|t| {
            t.replace(
                r#"["open", "in_progress", "done"]"#,
                r#"["open", "in_progress", "done", "blocked"]"#,
            )
        }),
        v2(|t| t.replace(r#"["bug"]"#, r#"["bug", "feature"]"#)),
        // Add a collection to a link's targets.
        v2(|t| {
            t.replace(
                r#""targets": ["people"]"#,
                r#""targets": ["people", "teams"]"#,
            )
        }),
        // Add, change and remove a computed field, a view and an index.
        v2(|t| {
            t.replace(
                r#""blocked": { "since": 1 }"#,
                r#""blocked": { "since": 1 }, "late": { "since": 2 }"#,
            )
        }),
        v2(|t| {
            t.replace(
                r#""ready": { "since": 1 }"#,
                r#""ready": { "since": 1 }, "mine": { "since": 2 }"#,
            )
        }),
        v2(|t| t.replace(r#""equal": 1, "fields""#, r#""equal": 0, "fields""#)),
        v2(|t| {
            t.replace(r#""byStatusPriority""#, r#""byPriority": { "equal": 0, "fields": [{ "direction": "desc", "field": "priority" }], "since": 2 }, "byStatusPriority""#)
        }),
        v2(|t| {
            t.replace(r#""byStatusPriority""#, r#""byOwner": { "equal": 1, "fields": [{ "direction": "asc", "field": "owner" }], "since": 2 }, "byStatusPriority""#)
        }),
        // Add a description.
        v2(|t| {
            t.replace(
                r#""title": { "rule""#,
                r#""title": { "description": "One line.", "rule""#,
            )
        }),
        // Deprecate a field and a collection.
        v2(|t| {
            t.replace(
                r#""priority": { "rule""#,
                r#""priority": { "deprecated": 2, "deprecatedReason": "Use rank.", "rule""#,
            )
        }),
        v2(|t| {
            t.replace(
                r#""teams": { "since": 1"#,
                r#""teams": { "deprecated": 2, "since": 1"#,
            )
        }),
    ];
    for next in accepted {
        assert_eq!(refusals(&next), vec![], "{next:?}");
    }
}

#[test]
fn refuses_the_rfc_example_with_its_messages() {
    // priority from nc.int() to nc.oneOf(["low", "high"]), title renamed to
    // summary (RFC 0005, "An example of each").
    let next = v2(|t| {
        t.replace(r#""priority": { "rule": 1, "since": 1, "type": "ncdb/int" }"#, r#""priority": { "options": ["low", "high"], "rule": 1, "since": 1, "type": "ncdb/choice" }"#)
            .replace(r#""title": { "rule": 1, "since": 1, "type": "ncdb/string" }"#, r#""summary": { "rule": 1, "since": 2, "type": "ncdb/string" }"#)
    });
    let found = refusals(&next);
    assert_eq!(
        found.iter().map(|r| r.lines()).collect::<Vec<_>>(),
        vec![
            vec![
                s("tasks.priority: type changes from ncdb/int to ncdb/choice."),
                s("Clients would merge and read existing values differently."),
                s("Add a new field instead, and deprecate tasks.priority."),
            ],
            vec![
                s(
                    "tasks.title: removed. Changes name fields; data in tasks.title would belong to nothing."
                ),
                s(
                    "To stop using it, keep it in the schema with .deprecated(\"...\"). To rename it, add tasks.summary."
                ),
            ],
        ]
    );
}

#[test]
fn refuses_removing_a_collection_or_a_field() {
    let next = v2(|t| t.replace(r#", "teams": { "since": 1, "fields": {} }"#, ""));
    assert_eq!(
        refusals(&next),
        vec![Refusal::CollectionRemoved {
            collection: s("teams")
        }]
    );
    let next = v2(|t| {
        t.replace(
            r#""title": { "rule": 1, "since": 1, "type": "ncdb/string" },"#,
            "",
        )
    });
    assert_eq!(
        refusals(&next),
        vec![Refusal::FieldRemoved {
            collection: s("tasks"),
            field: s("title"),
            rename: None
        }]
    );
    assert!(
        refusals(&next)[0].lines()[1].ends_with("To rename it, add a field under the new name.")
    );
    // Removing a deprecated field is refused too: deprecation is forever (D7).
    let next = v2(|t| {
        t.replace(r#""old": { "deprecated": 1, "deprecatedReason": "Unused.", "rule": 1, "since": 1, "type": "ncdb/boolean" },"#, "")
    });
    assert_eq!(
        refusals(&next),
        vec![Refusal::FieldRemoved {
            collection: s("tasks"),
            field: s("old"),
            rename: None
        }]
    );
}

#[test]
fn refuses_changing_a_type_or_a_set_element_type() {
    let next = v2(|t| {
        t.replace(
            r#""kinds": { "of": { "options": ["bug"], "type": "ncdb/choice" }"#,
            r#""kinds": { "of": { "type": "ncdb/string" }"#,
        )
    });
    assert_eq!(
        refusals(&next),
        vec![Refusal::TypeChanged {
            collection: s("tasks"),
            field: s("kinds"),
            from: s("ncdb/set of ncdb/choice"),
            to: s("ncdb/set of ncdb/string")
        }]
    );
    let next = v2(|t| {
        t.replace(
            r#""targets": ["tasks"], "type": "ncdb/links""#,
            r#""targets": ["tasks"], "type": "ncdb/link""#,
        )
    });
    assert_eq!(
        refusals(&next),
        vec![Refusal::TypeChanged {
            collection: s("tasks"),
            field: s("dependsOn"),
            from: s("ncdb/links"),
            to: s("ncdb/link")
        }]
    );
}

#[test]
fn refuses_moving_a_field_to_a_new_rule_version() {
    let next = v2(|t| t.replace(r#""votes": { "rule": 1"#, r#""votes": { "rule": 2"#));
    let found = refusals(&next);
    assert_eq!(
        found,
        vec![Refusal::RuleChanged {
            collection: s("tasks"),
            field: s("votes"),
            type_id: s("acme/counter"),
            from: 1,
            to: 2
        }]
    );
    assert_eq!(
        found[0].lines()[0],
        "tasks.votes: the rule version of acme/counter changes from 1 to 2."
    );
}

#[test]
fn refuses_removing_renaming_or_reordering_an_option() {
    for options in [
        r#"["open", "done"]"#,
        r#"["open", "doing", "done"]"#,
        r#"["in_progress", "open", "done"]"#,
    ] {
        let next = v2(|t| t.replace(r#"["open", "in_progress", "done"]"#, options));
        assert!(
            matches!(&refusals(&next)[..], [Refusal::OptionsChanged { field, .. }] if field == "status"),
            "{options}"
        );
    }
    let next = v2(|t| t.replace(r#"["bug"]"#, r#"["feature", "bug"]"#));
    let found = refusals(&next);
    assert_eq!(
        found,
        vec![Refusal::OptionsChanged {
            collection: s("tasks"),
            field: s("kinds"),
            from: vec![s("bug")],
            to: vec![s("feature"), s("bug")]
        }]
    );
    assert_eq!(
        found[0].lines()[0],
        "tasks.kinds: options change from [bug] to [feature, bug]."
    );
}

#[test]
fn refuses_removing_a_link_target() {
    let next = v2(|t| t.replace(r#""targets": ["people"]"#, r#""targets": ["teams"]"#));
    assert_eq!(
        refusals(&next),
        vec![Refusal::TargetRemoved {
            collection: s("tasks"),
            field: s("owner"),
            target: s("people")
        }]
    );
}

#[test]
fn refuses_marking_an_existing_field_personal() {
    let next = v2(|t| {
        t.replace(
            r#""title": { "rule""#,
            r#""title": { "personal": { "person": "owner" }, "rule""#,
        )
    });
    assert_eq!(
        refusals(&next),
        vec![Refusal::MarkedPersonal {
            collection: s("tasks"),
            field: s("title")
        }]
    );
}

#[test]
fn refuses_unmarking_a_personal_field_or_changing_its_person() {
    let next = v2(|t| t.replace(r#""personal": { "person": "self" }, "#, ""));
    assert_eq!(
        refusals(&next),
        vec![Refusal::UnmarkedPersonal {
            collection: s("people"),
            field: s("name"),
            person: s("self")
        }]
    );
    let next = v2(|t| {
        t.replace(
            r#""personal": { "person": "owner" }"#,
            r#""personal": { "person": "self" }"#,
        )
    });
    assert_eq!(
        refusals(&next),
        vec![Refusal::PersonChanged {
            collection: s("tasks"),
            field: s("email"),
            from: s("owner"),
            to: s("self")
        }]
    );
}

#[test]
fn refuses_undeprecating_a_field_or_a_collection() {
    let next = v2(|t| t.replace(r#""deprecated": 1, "deprecatedReason": "Unused.", "#, ""));
    assert_eq!(
        refusals(&next),
        vec![Refusal::Undeprecated {
            collection: s("tasks"),
            field: s("old")
        }]
    );
    let previous = lock(
        1,
        &V1.replace(
            r#""teams": { "since": 1"#,
            r#""teams": { "deprecated": 1, "since": 1"#,
        ),
    );
    assert_eq!(
        check::successor(&previous, &lock(2, V1)),
        vec![Refusal::CollectionUndeprecated {
            collection: s("teams")
        }]
    );
}

#[test]
fn refuses_an_index_on_an_unindexable_or_unknown_field_or_with_a_large_equal() {
    let index = |fields: &str, equal: u64| {
        v2(|t| {
            t.replace(r#""byStatusPriority""#, &format!(r#""byX": {{ "equal": {equal}, "fields": [{fields}], "since": 2 }}, "byStatusPriority""#))
        })
    };
    let on = |field: &str| format!(r#"{{ "direction": "asc", "field": "{field}" }}"#);
    for (field, type_id) in [
        ("kinds", "ncdb/set"),
        ("dependsOn", "ncdb/links"),
        ("votes", "acme/counter"),
    ] {
        assert_eq!(
            refusals(&index(&on(field), 0)),
            vec![Refusal::IndexNotIndexable {
                collection: s("tasks"),
                index: s("byX"),
                field: s(field),
                type_id: s(type_id)
            }]
        );
    }
    // A computed field is not a stored field: computed fields in indexes
    // belong to the computed-fields RFC.
    for field in ["nothing", "blocked"] {
        assert_eq!(
            refusals(&index(&on(field), 0)),
            vec![Refusal::IndexUnknownField {
                collection: s("tasks"),
                index: s("byX"),
                field: s(field)
            }]
        );
    }
    assert_eq!(
        refusals(&index(&on("title"), 2)),
        vec![Refusal::IndexEqualTooLarge {
            collection: s("tasks"),
            index: s("byX"),
            equal: 2,
            fields: 1
        }]
    );
}

#[test]
fn refuses_names_that_break_the_rules_or_collide_in_case() {
    let long = "a".repeat(64);
    for name in [
        "Title",
        "_title",
        "$title",
        "ti-tle",
        "1title",
        "títle",
        long.as_str(),
    ] {
        let next = v2(|t| {
            t.replace(
                r#""title": {"#,
                &format!(
                    r#""{name}": {{ "rule": 1, "since": 2, "type": "ncdb/string" }}, "title": {{"#
                ),
            )
        });
        assert_eq!(
            refusals(&next),
            vec![Refusal::BadName {
                kind: NameKind::Field,
                collection: Some(s("tasks")),
                name: s(name)
            }],
            "{name}"
        );
    }
    let next = v2(|t| {
        t.replace(
            r#""teams": {"#,
            r#""Goals": { "since": 2, "fields": {} }, "teams": {"#,
        )
    });
    assert_eq!(
        refusals(&next),
        vec![Refusal::BadName {
            kind: NameKind::Collection,
            collection: None,
            name: s("Goals")
        }]
    );
    let next = v2(|t| {
        t.replace(
            r#""title": {"#,
            r#""tiTle": { "rule": 1, "since": 2, "type": "ncdb/string" }, "title": {"#,
        )
    });
    assert_eq!(
        refusals(&next),
        vec![Refusal::NameClash {
            collection: Some(s("tasks")),
            first: (NameKind::Field, s("tiTle")),
            second: (NameKind::Field, s("title"))
        }]
    );
    // Fields, computed fields, views and indexes share one scope.
    let next = v2(|t| {
        t.replace(
            r#""ready": { "since": 1 }"#,
            r#""ready": { "since": 1 }, "title": { "since": 2 }"#,
        )
    });
    let found = refusals(&next);
    assert_eq!(
        found,
        vec![Refusal::NameClash {
            collection: Some(s("tasks")),
            first: (NameKind::Field, s("title")),
            second: (NameKind::View, s("title"))
        }]
    );
    assert_eq!(
        found[0].lines()[0],
        "view tasks.title: the name collides with the field tasks.title."
    );
    let next = v2(|t| {
        t.replace(
            r#""teams": {"#,
            r#""peoplE": { "since": 2, "fields": {} }, "teams": {"#,
        )
    });
    assert_eq!(
        refusals(&next),
        vec![Refusal::NameClash {
            collection: None,
            first: (NameKind::Collection, s("peoplE")),
            second: (NameKind::Collection, s("people"))
        }]
    );
}

#[test]
fn refuses_personal_fields_that_break_the_rules() {
    let add = |field: &str| v2(|t| t.replace(r#""title": {"#, &format!(r#"{field}, "title": {{"#)));
    let next = add(
        r#""tags": { "of": { "type": "ncdb/string" }, "personal": { "person": "self" }, "rule": 1, "since": 2, "type": "ncdb/set" }"#,
    );
    assert_eq!(
        refusals(&next),
        vec![Refusal::PersonalNotBasic {
            collection: s("tasks"),
            field: s("tags"),
            type_id: s("ncdb/set")
        }]
    );
    let next = add(
        r#""boss": { "personal": { "person": "self" }, "rule": 1, "since": 2, "targets": ["people"], "type": "ncdb/link" }"#,
    );
    assert_eq!(
        refusals(&next),
        vec![Refusal::PersonalNotBasic {
            collection: s("tasks"),
            field: s("boss"),
            type_id: s("ncdb/link")
        }]
    );
    for person in ["dependsOn", "title", "nobody"] {
        let next = add(&format!(
            r#""about": {{ "personal": {{ "person": "{person}" }}, "rule": 1, "since": 2, "type": "ncdb/string" }}"#
        ));
        assert_eq!(
            refusals(&next),
            vec![Refusal::PersonNotLink {
                collection: s("tasks"),
                field: s("about"),
                person: s(person)
            }],
            "{person}"
        );
    }
    let next = lock(2, &V1.replace(r#""title": {"#, r#""boss": { "personal": { "person": "self" }, "rule": 1, "since": 1, "targets": ["people"], "type": "ncdb/link" }, "about": { "personal": { "person": "boss" }, "rule": 1, "since": 2, "type": "ncdb/string" }, "title": {"#));
    assert!(
        check::validate(&next).contains(&Refusal::PersonLinkPersonal {
            collection: s("tasks"),
            field: s("about"),
            person: s("boss")
        })
    );
}

#[test]
fn refuses_unknown_rule_versions_plugins_targets_and_duplicate_options() {
    let add = |field: &str| v2(|t| t.replace(r#""title": {"#, &format!(r#"{field}, "title": {{"#)));
    assert_eq!(
        refusals(&add(
            r#""a": { "rule": 2, "since": 2, "type": "ncdb/string" }"#
        )),
        vec![Refusal::UnknownRule {
            collection: s("tasks"),
            field: s("a"),
            type_id: s("ncdb/string"),
            rule: 2
        }]
    );
    assert_eq!(
        refusals(&add(
            r#""a": { "rule": 1, "since": 2, "type": "other/counter" }"#
        )),
        vec![Refusal::UnknownPlugin {
            collection: s("tasks"),
            field: s("a"),
            type_id: s("other/counter")
        }]
    );
    assert_eq!(
        refusals(&add(
            r#""a": { "rule": 1, "since": 2, "targets": ["goals"], "type": "ncdb/links" }"#
        )),
        vec![Refusal::UnknownTarget {
            collection: s("tasks"),
            field: s("a"),
            target: s("goals")
        }]
    );
    assert_eq!(
        refusals(&add(
            r#""a": { "options": ["x", "y", "x"], "rule": 1, "since": 2, "type": "ncdb/choice" }"#
        )),
        vec![Refusal::DuplicateOption {
            collection: s("tasks"),
            field: s("a"),
            option: s("x")
        }]
    );
}

#[test]
fn refuses_a_lock_of_another_model_or_a_lower_version() {
    let mut other = lock(2, V1);
    other.model.id = ModelId([7; 16]);
    let found = refusals(&other);
    assert_eq!(
        found,
        vec![Refusal::OtherModel {
            previous: lock(1, V1).model.id,
            next: ModelId([7; 16])
        }]
    );
    assert_eq!(
        found[0].lines()[0],
        "model.id: 07070707070707070707070707070707 is not 6f1c0e2a9b7d4c3e8a5f0b1d2c3e4f5a."
    );
    assert_eq!(
        check::successor(&lock(3, V1), &lock(2, V1)),
        vec![Refusal::LowerVersion {
            previous: 3,
            next: 2
        }]
    );
}
