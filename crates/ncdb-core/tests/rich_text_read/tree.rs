//! The tree read (RFC 0013, "Reading"): the document as ProseMirror JSON,
//! `{ type, attrs, content, marks, text }`, as `Data`, read from Yrs and
//! never through a schema, so nothing outside the vocabulary is dropped.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::BTreeMap;

use ncdb_core::rich_text::read::tree;
use ncdb_plugin::Data;
use yrs::Any;

use crate::doc::{el, i, m, ma, p, s, state, t};

fn map(entries: Vec<(&str, Data)>) -> Data {
    Data::Map(
        entries
            .into_iter()
            .map(|(k, v)| (k.to_owned(), v))
            .collect::<BTreeMap<_, _>>(),
    )
}

fn text(value: &str) -> Data {
    Data::Text(value.to_owned())
}

fn node(kind: &str, rest: Vec<(&str, Data)>) -> Data {
    let mut entries = vec![("type", text(kind))];
    entries.extend(rest);
    map(entries)
}

#[test]
fn an_empty_field_reads_as_an_empty_doc() {
    assert_eq!(
        tree(&Default::default()),
        node("doc", vec![("content", Data::Array(vec![]))])
    );
}

#[test]
fn the_tree_holds_nodes_attributes_marks_and_text() {
    let state = state(vec![
        el("heading", &[("level", i(2))], vec![t("Plan")]),
        p(vec![
            t("We "),
            m("ship", &["bold", "italic"]),
            ma(" here", "link", &[("href", s("https://x"))]),
        ]),
        p(vec![]),
        el("horizontalRule", &[], vec![]),
    ]);
    let expected = node(
        "doc",
        vec![(
            "content",
            Data::Array(vec![
                node(
                    "heading",
                    vec![
                        ("attrs", map(vec![("level", Data::Integer(2))])),
                        (
                            "content",
                            Data::Array(vec![node("text", vec![("text", text("Plan"))])]),
                        ),
                    ],
                ),
                node(
                    "paragraph",
                    vec![(
                        "content",
                        Data::Array(vec![
                            node("text", vec![("text", text("We "))]),
                            node(
                                "text",
                                vec![
                                    ("text", text("ship")),
                                    (
                                        "marks",
                                        Data::Array(vec![
                                            node("bold", vec![]),
                                            node("italic", vec![]),
                                        ]),
                                    ),
                                ],
                            ),
                            node(
                                "text",
                                vec![
                                    ("text", text(" here")),
                                    (
                                        "marks",
                                        Data::Array(vec![node(
                                            "link",
                                            vec![("attrs", map(vec![("href", text("https://x"))]))],
                                        )]),
                                    ),
                                ],
                            ),
                        ]),
                    )],
                ),
                node("paragraph", vec![]),
                node("horizontalRule", vec![]),
            ]),
        )],
    );
    assert_eq!(tree(&state), expected);
}

#[test]
fn a_node_and_a_mark_outside_the_vocabulary_stay_whole() {
    let mut nested = std::collections::HashMap::new();
    nested.insert("k".to_owned(), Any::from(1.5));
    let state = state(vec![el(
        "callout",
        &[
            ("tone", s("warn")),
            ("meta", Any::from(nested)),
            ("on", Any::Bool(true)),
        ],
        vec![ma("x", "comment", &[("id", s("c1"))])],
    )]);
    let expected = node(
        "doc",
        vec![(
            "content",
            Data::Array(vec![node(
                "callout",
                vec![
                    (
                        "attrs",
                        map(vec![
                            ("meta", map(vec![("k", Data::Float(1.5))])),
                            ("on", Data::Bool(true)),
                            ("tone", text("warn")),
                        ]),
                    ),
                    (
                        "content",
                        Data::Array(vec![node(
                            "text",
                            vec![
                                ("text", text("x")),
                                (
                                    "marks",
                                    Data::Array(vec![node(
                                        "comment",
                                        vec![("attrs", map(vec![("id", text("c1"))]))],
                                    )]),
                                ),
                            ],
                        )]),
                    ),
                ],
            )]),
        )],
    );
    assert_eq!(tree(&state), expected);
}
