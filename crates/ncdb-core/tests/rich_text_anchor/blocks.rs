//! A block's type changes only when the replacement changes it (RFC 0013,
//! "An agent: anchored edits"; F14): changing a type rewrites the block in
//! Yjs, so a person's concurrent typing in it is lost; an edit of the
//! text of a block, quoted whole or in part, keeps the block and with it
//! what the person types there meanwhile.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::field::FieldType;
use ncdb_core::rich_text::RichText;

use crate::doc::{el, i, p, t};
use crate::rich_text_support::type_text;
use crate::support::{Agent, Field, replace};

/// The agent reads, the person types `typed` at `at` in block `block`,
/// then the agent's edit is folded.
fn concurrently(
    field: &mut Field,
    block: u32,
    at: u32,
    typed: &str,
    edits: &[crate::support::Edit],
) {
    let mut agent = Agent::new();
    let reading = agent.read(field);
    let update = type_text(&field.person, block, at, typed);
    field.person(update);
    let written = agent.edit(&reading, edits).unwrap();
    field.agent(&written);
}

#[test]
fn editing_a_headings_text_keeps_the_heading_and_the_persons_typing() {
    let mut field = Field::new(vec![el("heading", &[("level", i(2))], vec![t("Plan")])]);
    concurrently(&mut field, 0, 0, "Our ", &[replace("Plan", "Plans")]);
    assert_eq!(field.markdown(), "## Our Plans");
}

#[test]
fn a_whole_block_quoted_keeps_its_type_and_the_persons_typing() {
    let mut field = Field::new(vec![p(vec![t("We ship on Friday.")])]);
    concurrently(
        &mut field,
        0,
        18,
        " Bob typed this.",
        &[replace("We ship on Friday.", "We ship on Monday.")],
    );
    assert_eq!(field.markdown(), "We ship on Monday. Bob typed this.");
}

#[test]
fn a_quote_across_blocks_changes_only_the_type_the_replacement_changes() {
    let mut field = Field::new(vec![p(vec![t("Intro")]), p(vec![t("We ship.")])]);
    concurrently(
        &mut field,
        1,
        8,
        " Soon.",
        &[replace("Intro\n\nWe ship.", "# Intro\n\nWe ship!")],
    );
    assert_eq!(field.markdown(), "# Intro\n\nWe ship! Soon.");
}

#[test]
fn a_replacement_that_changes_the_type_rewrites_the_block_and_the_persons_typing_is_lost() {
    let mut field = Field::new(vec![p(vec![t("Plan")]), p(vec![t("Rest.")])]);
    concurrently(&mut field, 0, 4, " ahead", &[replace("Plan", "# Plan")]);
    assert_eq!(field.markdown(), "# Plan\n\nRest.");
    // The person's typing is lost, and stays identifiable.
    assert!(!RichText.read(&field.state).lost.is_empty());
}
