//! The Markdown read (RFC 0013, "Reading"): CommonMark with GitHub's
//! tables, strikethrough and task lists, in TipTap's standard vocabulary,
//! and an HTML tag for any other part.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::rich_text::read::markdown;
use yrs::Any;

use crate::doc::{N, el, i, m, ma, p, s, state, t};

fn md(nodes: Vec<N>) -> String {
    markdown(&state(nodes))
}

#[test]
fn an_empty_field_reads_as_no_markdown() {
    assert_eq!(markdown(&Default::default()), "");
    assert_eq!(md(vec![]), "");
}

#[test]
fn paragraphs_are_separated_by_a_blank_line() {
    assert_eq!(
        md(vec![p(vec![t("First.")]), p(vec![t("Second.")])]),
        "First.\n\nSecond."
    );
}

#[test]
fn an_empty_paragraph_keeps_its_place() {
    assert_eq!(
        md(vec![p(vec![t("A")]), p(vec![]), p(vec![t("B")])]),
        "A\n\n\n\nB"
    );
}

#[test]
fn headings_take_their_level_and_default_to_one() {
    assert_eq!(
        md(vec![
            el("heading", &[("level", i(2))], vec![t("Plan")]),
            el("heading", &[], vec![t("Top")]),
            el("heading", &[("level", Any::from(3.0))], vec![t("Three")]),
        ]),
        "## Plan\n\n# Top\n\n### Three"
    );
}

#[test]
fn a_heading_with_a_level_markdown_has_no_form_for_reads_as_a_tag() {
    assert_eq!(
        md(vec![el("heading", &[("level", i(7))], vec![t("Deep")])]),
        "<heading level=\"7\">Deep</heading>"
    );
}

#[test]
fn marks_render_in_their_markdown_forms() {
    assert_eq!(
        md(vec![p(vec![
            m("bold", &["bold"]),
            t(" "),
            m("italic", &["italic"]),
            t(" "),
            m("gone", &["strike"]),
            t(" "),
            m("x = 1", &["code"]),
        ])]),
        "**bold** *italic* ~~gone~~ `x = 1`"
    );
}

#[test]
fn nested_marks_open_and_close_in_order() {
    assert_eq!(
        md(vec![p(vec![
            m("a", &["bold"]),
            m("b", &["bold", "italic"]),
            m("c", &["bold"]),
            t(" "),
            m("d", &["italic"]),
            m("e", &["bold", "italic"]),
        ])]),
        "**a*b*c** *d**e***"
    );
}

#[test]
fn whitespace_at_a_marks_edge_moves_outside_it() {
    assert_eq!(
        md(vec![p(vec![t("We"), m(" ship ", &["bold"]), t("Friday")])]),
        "We **ship** Friday"
    );
}

#[test]
fn underline_and_highlight_read_as_their_text() {
    assert_eq!(
        md(vec![p(vec![
            m("under", &["underline"]),
            t(" "),
            ma("lit", "highlight", &[("color", s("#ff0"))]),
        ])]),
        "under lit"
    );
}

#[test]
fn a_link_reads_with_its_href() {
    assert_eq!(
        md(vec![p(vec![
            t("See "),
            ma("the docs", "link", &[("href", s("https://example.com/a(b)")), ("target", s("_blank"))]),
            t("."),
        ])]),
        "See [the docs](https://example.com/a\\(b\\))."
    );
}

#[test]
fn a_link_whose_href_holds_a_space_is_written_in_angle_brackets() {
    assert_eq!(
        md(vec![p(vec![ma("x", "link", &[("href", s("a b"))])])]),
        "[x](<a b>)"
    );
}

#[test]
fn code_holding_backticks_takes_a_longer_fence() {
    assert_eq!(
        md(vec![p(vec![m("a `b` c", &["code"]), t(" "), m("`", &["code"])])]),
        "``a `b` c`` `` ` ``"
    );
}

#[test]
fn text_that_looks_like_markup_is_escaped() {
    assert_eq!(
        md(vec![p(vec![t("2 * 3 _x_ [a] <b> `c` ~d~ \\ AT&T &amp;")])]),
        "2 \\* 3 \\_x\\_ \\[a\\] \\<b> \\`c\\` \\~d\\~ \\\\ AT&T \\&amp;"
    );
}

#[test]
fn a_line_that_would_start_a_block_is_escaped() {
    assert_eq!(
        md(vec![
            p(vec![t("# not a heading")]),
            p(vec![t("- not a list")]),
            p(vec![t("+ nor this")]),
            p(vec![t("> no quote")]),
            p(vec![t("1. no number")]),
            p(vec![t("2) nor this")]),
            p(vec![t("=")]),
        ]),
        "\\# not a heading\n\n\\- not a list\n\n\\+ nor this\n\n\\> no quote\n\n1\\. no number\n\n2\\) nor this\n\n\\="
    );
}

#[test]
fn a_hard_break_ends_a_line_with_a_backslash() {
    assert_eq!(
        md(vec![p(vec![t("One"), el("hardBreak", &[], vec![]), t("# two")])]),
        "One\\\n\\# two"
    );
}

#[test]
fn a_blockquote_prefixes_each_line() {
    assert_eq!(
        md(vec![el(
            "blockquote",
            &[],
            vec![p(vec![t("One")]), p(vec![t("Two")])]
        )]),
        "> One\n>\n> Two"
    );
}

fn item(children: Vec<N>) -> N {
    el("listItem", &[], children)
}

#[test]
fn a_bullet_list_is_tight_and_nests_by_indent() {
    assert_eq!(
        md(vec![el(
            "bulletList",
            &[],
            vec![
                item(vec![
                    p(vec![t("One")]),
                    el("bulletList", &[], vec![item(vec![p(vec![t("Inner")])])]),
                ]),
                item(vec![p(vec![t("Two")]), p(vec![t("More")])]),
            ]
        )]),
        "- One\n  - Inner\n- Two\n\n  More"
    );
}

#[test]
fn an_ordered_list_counts_from_its_start() {
    assert_eq!(
        md(vec![el(
            "orderedList",
            &[("start", i(9))],
            vec![item(vec![p(vec![t("Nine")])]), item(vec![p(vec![t("Ten")])])]
        )]),
        "9. Nine\n10. Ten"
    );
}

#[test]
fn two_lists_in_a_row_stay_two_lists() {
    let list = || el("bulletList", &[], vec![item(vec![p(vec![t("A")])])]);
    let ordered = || el("orderedList", &[], vec![item(vec![p(vec![t("B")])])]);
    assert_eq!(
        md(vec![list(), list(), ordered(), ordered()]),
        "- A\n\n* A\n\n1. B\n\n1) B"
    );
}

#[test]
fn a_task_list_shows_each_items_box() {
    let task = |checked: Option<bool>, text: &str| {
        let attrs: Vec<(&'static str, Any)> =
            checked.map(|c| ("checked", Any::Bool(c))).into_iter().collect();
        el("taskItem", &attrs, vec![p(vec![t(text)])])
    };
    assert_eq!(
        md(vec![el(
            "taskList",
            &[],
            vec![task(Some(true), "Done"), task(Some(false), "Open"), task(None, "New")]
        )]),
        "- [x] Done\n- [ ] Open\n- [ ] New"
    );
}

#[test]
fn a_code_block_is_fenced_with_its_language() {
    assert_eq!(
        md(vec![
            el("codeBlock", &[("language", s("rust"))], vec![t("let a = 1;\n```\n*b*")]),
            el("codeBlock", &[], vec![t("plain")]),
        ]),
        "````rust\nlet a = 1;\n```\n*b*\n````\n\n```\nplain\n```"
    );
}

#[test]
fn a_horizontal_rule_is_three_dashes() {
    assert_eq!(
        md(vec![p(vec![t("A")]), el("horizontalRule", &[], vec![]), p(vec![t("B")])]),
        "A\n\n---\n\nB"
    );
}

#[test]
fn a_table_reads_as_a_github_table_with_its_first_row_as_header() {
    let cell = |kind: &'static str, children: Vec<N>| el(kind, &[], vec![p(children)]);
    assert_eq!(
        md(vec![el(
            "table",
            &[],
            vec![
                el(
                    "tableRow",
                    &[],
                    vec![cell("tableHeader", vec![t("Name")]), cell("tableHeader", vec![t("Note")])]
                ),
                el(
                    "tableRow",
                    &[],
                    vec![
                        cell("tableCell", vec![m("Ada", &["bold"])]),
                        cell("tableCell", vec![t("a|b"), el("hardBreak", &[], vec![]), t("c")]),
                    ]
                ),
                el("tableRow", &[], vec![cell("tableCell", vec![t("short")])]),
            ]
        )]),
        "| Name | Note |\n| --- | --- |\n| **Ada** | a\\|b<br>c |\n| short |  |"
    );
}

#[test]
fn a_node_outside_the_vocabulary_reads_as_a_tag_around_its_content() {
    assert_eq!(
        md(vec![
            el("callout", &[("tone", s("warn"))], vec![t("Careful.")]),
            el("panel", &[("open", Any::Bool(true)), ("a", s("say \"hi\" & go"))], vec![
                p(vec![t("Inside.")]),
                p(vec![t("Twice.")]),
            ]),
            el("divider", &[], vec![]),
            p(vec![t("A "), el("chip", &[("n", i(2))], vec![t("x")]), t(" b")]),
        ]),
        "<callout tone=\"warn\">Careful.</callout>\n\n\
         <panel a=\"say &quot;hi&quot; &amp; go\" open=\"true\">\n\nInside.\n\nTwice.\n\n</panel>\n\n\
         <divider></divider>\n\n\
         A <chip n=\"2\">x</chip> b"
    );
}

#[test]
fn a_mark_outside_the_vocabulary_reads_as_a_tag_around_its_text() {
    assert_eq!(
        md(vec![p(vec![
            t("A "),
            ma("noted", "comment", &[("id", s("c1"))]),
            t(" b"),
        ])]),
        "A <comment id=\"c1\">noted</comment> b"
    );
}

#[test]
fn an_attribute_that_is_no_text_reads_as_json() {
    let mut map = std::collections::HashMap::new();
    map.insert("b".to_owned(), Any::from(vec![Any::from(1), Any::Null]));
    map.insert("a".to_owned(), Any::from("x\"y"));
    assert_eq!(
        md(vec![el("widget", &[("data", Any::from(map)), ("size", Any::from(1.5))], vec![])]),
        "<widget data=\"{&quot;a&quot;:&quot;x\\&quot;y&quot;,&quot;b&quot;:[1,null]}\" size=\"1.5\"></widget>"
    );
}

#[test]
fn a_mark_y_prosemirror_names_with_a_hash_reads_under_its_name() {
    assert_eq!(
        md(vec![p(vec![m("b", &["bold--AbC1+/=9"])])]),
        "**b**"
    );
}
