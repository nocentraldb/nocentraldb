//! The plain-text read (RFC 0013, "Reading"): the text of each block, one
//! block per line, for previews.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::rich_text::read::text;

use crate::doc::{N, el, i, m, ma, p, s, state, t};

fn plain(nodes: Vec<N>) -> String {
    text(&state(nodes))
}

#[test]
fn an_empty_field_reads_as_no_text() {
    assert_eq!(text(&Default::default()), "");
}

#[test]
fn each_block_is_one_line_without_markup() {
    assert_eq!(
        plain(vec![
            el("heading", &[("level", i(1))], vec![t("Plan")]),
            p(vec![t("We "), m("ship", &["bold"]), t(" *now*.")]),
            p(vec![]),
            p(vec![ma("link", "link", &[("href", s("https://x"))])]),
        ]),
        "Plan\nWe ship *now*.\n\nlink"
    );
}

#[test]
fn nested_blocks_give_one_line_each_and_a_rule_gives_none() {
    let item = |text: &str| el("listItem", &[], vec![p(vec![t(text)])]);
    assert_eq!(
        plain(vec![
            el("blockquote", &[], vec![p(vec![t("Quoted")])]),
            el("bulletList", &[], vec![item("One"), item("Two")]),
            el("horizontalRule", &[], vec![]),
            el(
                "table",
                &[],
                vec![el(
                    "tableRow",
                    &[],
                    vec![
                        el("tableCell", &[], vec![p(vec![t("A")])]),
                        el("tableCell", &[], vec![p(vec![t("B")])]),
                    ]
                )]
            ),
            el("callout", &[], vec![t("Careful.")]),
        ]),
        "Quoted\nOne\nTwo\nA\nB\nCareful."
    );
}

#[test]
fn a_hard_break_and_a_code_block_keep_their_lines() {
    assert_eq!(
        plain(vec![
            p(vec![t("One"), el("hardBreak", &[], vec![]), t("Two")]),
            el("codeBlock", &[], vec![t("a\nb")]),
        ]),
        "One\nTwo\na\nb"
    );
}

#[test]
fn a_mention_reads_as_an_at_sign_and_another_inline_node_as_its_text() {
    let space = "0123456789abcdef0123456789abcdef";
    let id = "fedcba9876543210fedcba9876543210";
    assert_eq!(
        plain(vec![p(vec![
            t("Ask "),
            el("mention", &[("space", s(space)), ("id", s(id))], vec![]),
            t(" about "),
            el("chip", &[], vec![t("x")]),
        ])]),
        "Ask @ about x"
    );
}
