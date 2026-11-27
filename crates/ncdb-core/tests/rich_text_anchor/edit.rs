//! The smallest change (RFC 0013, "An agent: anchored edits", step 3):
//! text by characters, marks by ranges, written directly on the Yrs
//! document, so that what the Markdown does not show stays untouched.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::rich_text::anchor::Error;
use ncdb_core::rich_text::update;
use ncdb_plugin::Data;

use crate::doc::{el, i, m, ma, p, s, t};
use crate::support::{Agent, Field, after, append, edited, replace};

#[test]
fn a_word_is_replaced_by_its_characters_and_the_rest_is_kept() {
    let mut field = Field::new(vec![
        p(vec![m("We", &["underline"]), t(" ship on Friday.")]),
        p(vec![t("Outro.")]),
    ]);
    let mut agent = Agent::new();
    let reading = agent.read(&field);
    let written = agent
        .edit(&reading, &[replace("on Friday.", "on Monday.")])
        .unwrap();
    // "Fri" goes and "Mon" comes; nothing else is written.
    let shape = update::read(&written.update).unwrap();
    let added: u64 = shape.runs.iter().map(|run| run.range.len).sum();
    let deleted: u64 = shape.deletes.iter().map(|range| range.len).sum();
    assert_eq!((added, deleted), (3, 3));
    field.agent(&written);
    assert_eq!(field.markdown(), "We ship on Monday.\n\nOutro.");
    // The underline the Markdown does not show stays.
    let tree = format!("{:?}", field.tree());
    assert!(tree.contains("underline"), "{tree}");
}

#[test]
fn a_mark_changes_by_its_range_without_rewriting_the_text() {
    let mut field = Field::new(vec![p(vec![t("We ship on Friday.")])]);
    let mut agent = Agent::new();
    let reading = agent.read(&field);
    let written = agent
        .edit(&reading, &[replace("Friday", "**Friday**")])
        .unwrap();
    assert!(update::read(&written.update).unwrap().deletes.is_empty());
    field.agent(&written);
    assert_eq!(field.markdown(), "We ship on **Friday**.");
    let reading = agent.read(&field);
    let written = agent
        .edit(&reading, &[replace("**Friday**", "*Friday*")])
        .unwrap();
    field.agent(&written);
    assert_eq!(field.markdown(), "We ship on *Friday*.");
}

#[test]
fn a_link_and_code_are_written_as_marks() {
    let mut field = Field::new(vec![p(vec![t("See the docs and run it.")])]);
    assert_eq!(
        edited(
            &mut field,
            &[replace(
                "See the docs and run it.",
                "See [the docs](https://example.com/a) and run `it`."
            )]
        ),
        Ok("See [the docs](https://example.com/a) and run `it`.".to_owned())
    );
}

#[test]
fn a_link_keeps_the_attributes_the_markdown_does_not_show() {
    let mut field = Field::new(vec![p(vec![
        ma(
            "docs",
            "link",
            &[
                ("href", s("https://example.com/a")),
                ("target", s("_blank")),
            ],
        ),
        t(" here."),
    ])]);
    assert_eq!(
        edited(&mut field, &[replace("here.", "there.")]),
        Ok("[docs](https://example.com/a) there.".to_owned())
    );
    let tree = format!("{:?}", field.tree());
    assert!(tree.contains("_blank"), "{tree}");
}

#[test]
fn a_replacement_adds_blocks() {
    let mut field = Field::new(vec![p(vec![t("Intro.")]), p(vec![t("Outro.")])]);
    assert_eq!(
        edited(
            &mut field,
            &[replace("Outro.", "Outro.\n\n- one\n- two\n\nThe end.")]
        ),
        Ok("Intro.\n\nOutro.\n\n- one\n- two\n\nThe end.".to_owned())
    );
}

#[test]
fn a_quote_across_blocks_can_remove_a_block() {
    let mut field = Field::new(vec![
        p(vec![t("One.")]),
        p(vec![t("Two.")]),
        p(vec![t("Three.")]),
    ]);
    assert_eq!(
        edited(&mut field, &[replace("One.\n\nTwo.", "One.")]),
        Ok("One.\n\nThree.".to_owned())
    );
}

#[test]
fn insert_after_and_append_add_blocks() {
    let mut field = Field::new(vec![p(vec![t("Intro.")]), p(vec![t("Outro.")])]);
    assert_eq!(
        edited(
            &mut field,
            &[
                after("Intro.", "## Plan\n\nShip it."),
                append("A closing line.")
            ]
        ),
        Ok("Intro.\n\n## Plan\n\nShip it.\n\nOutro.\n\nA closing line.".to_owned())
    );
}

#[test]
fn an_edit_inside_a_list_item_and_a_task_are_made_in_place() {
    let item = |text: &str| el("listItem", &[], vec![p(vec![t(text)])]);
    let task = |text: &str| {
        el(
            "taskItem",
            &[("checked", yrs::Any::Bool(false))],
            vec![p(vec![t(text)])],
        )
    };
    let mut field = Field::new(vec![
        el("bulletList", &[], vec![item("Milk"), item("Bread")]),
        el("taskList", &[], vec![task("Call")]),
    ]);
    assert_eq!(field.markdown(), "- Milk\n- Bread\n\n* [ ] Call");
    assert_eq!(
        edited(
            &mut field,
            &[
                replace("Bread", "Butter"),
                replace("* [ ] Call", "* [x] Call")
            ]
        ),
        Ok("- Milk\n- Butter\n\n* [x] Call".to_owned())
    );
}

#[test]
fn a_mention_is_written_from_its_markdown_form() {
    let space = "0123456789abcdef0123456789abcdef";
    let id = "fedcba9876543210fedcba9876543210";
    let mut field = Field::new(vec![p(vec![t("Ask about it.")])]);
    let mention = format!("[@](ncdb:{space}/{id})");
    assert_eq!(
        edited(
            &mut field,
            &[replace("Ask about", &format!("Ask {mention} about"))]
        ),
        Ok(format!("Ask {mention} about it."))
    );
    let tree = field.tree();
    let printed = format!("{tree:?}");
    assert!(printed.contains("mention"), "{printed}");
    let _ = Data::Null;
}

#[test]
fn a_document_part_the_core_does_not_read_back_is_refused_in_the_quote_only() {
    let mut field = Field::new(vec![
        p(vec![t("Before.")]),
        el("callout", &[("tone", s("warn"))], vec![t("Careful.")]),
        p(vec![t("After.")]),
    ]);
    assert_eq!(
        edited(&mut field, &[replace("Careful.", "Be careful.")]),
        Err(Error::Unsupported { edit: 0 })
    );
    // A quote outside the tag is edited.
    assert_eq!(
        edited(&mut field, &[replace("After.", "Later.")]),
        Ok("Before.\n\n<callout tone=\"warn\">Careful.</callout>\n\nLater.".to_owned())
    );
}

#[test]
fn a_replacement_with_html_is_refused() {
    let mut field = Field::new(vec![p(vec![t("Plain.")])]);
    assert_eq!(
        edited(&mut field, &[replace("Plain.", "<b>Bold.</b>")]),
        Err(Error::Unsupported { edit: 0 })
    );
    assert_eq!(field.markdown(), "Plain.");
}

#[test]
fn a_heading_level_changes_in_place() {
    let mut field = Field::new(vec![el("heading", &[("level", i(1))], vec![t("Plan")])]);
    assert_eq!(
        edited(&mut field, &[replace("# Plan", "## Plan")]),
        Ok("## Plan".to_owned())
    );
}
