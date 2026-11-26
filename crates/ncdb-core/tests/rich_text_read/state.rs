//! The four reads are functions of the state (C7): copies that fold the
//! same updates in other orders hold equal state bytes and read alike, and
//! a state read back from its bytes reads as itself. Attributes and marks,
//! which Yrs keeps in hash maps, read in one order on every copy.
//!
//! The marks here hold at most one attribute each: a mark with more makes
//! the state's bytes depend on the order of a hash map (reported on
//! NOC-460), which the reads do not.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::field::FieldType;
use ncdb_core::rich_text::read::{markdown, mentions, text, tree};
use ncdb_core::rich_text::{RichText, State};
use yrs::Any;

use crate::doc::{el, fold, fold_in, m, ma, p, s, steps, t};

/// Three updates by one member, the later ones made on the earlier.
fn updates() -> Vec<Vec<u8>> {
    let attrs: Vec<(&'static str, Any)> = ["a", "b", "c", "d", "e", "f", "g", "h"]
        .into_iter()
        .map(|k| (k, s(k)))
        .collect();
    let first = vec![
        el("widget", &attrs, vec![]),
        p(vec![
            ma("x", "link", &[("href", s("h"))]),
            m("y", &["bold", "italic", "strike", "underline", "comment", "note"]),
        ]),
    ];
    let mention = el(
        "mention",
        &[
            ("space", s("0123456789abcdef0123456789abcdef")),
            ("id", s("fedcba9876543210fedcba9876543210")),
        ],
        vec![],
    );
    steps(vec![first, vec![p(vec![t("Then "), mention])], vec![p(vec![t("Last.")])]])
}

#[test]
fn every_read_is_the_same_on_copies_with_equal_state_bytes() {
    let updates = updates();
    let forward = fold(&updates);
    let backward = fold_in(&updates, &[2, 0, 1]);
    assert_eq!(forward.bytes(), backward.bytes());
    let again = State::from_bytes(&forward.bytes()).unwrap();
    for state in [&backward, &again] {
        assert_eq!(markdown(state), markdown(&forward));
        assert_eq!(text(state), text(&forward));
        assert_eq!(tree(state), tree(&forward));
        assert_eq!(mentions(state), mentions(&forward));
        let (read, expected) = (RichText.read(state), RichText.read(&forward));
        assert_eq!(
            (read.markdown, read.text, read.tree, read.mentions, read.lost),
            (expected.markdown, expected.text, expected.tree, expected.mentions, expected.lost)
        );
    }
}

#[test]
fn the_field_types_read_carries_the_four_reads() {
    let state = fold(&updates());
    let read = RichText.read(&state);
    assert_eq!(read.markdown, markdown(&state));
    assert_eq!(read.text, text(&state));
    assert_eq!(read.tree, tree(&state));
    assert_eq!(read.mentions, mentions(&state));
}

#[test]
fn a_read_prints_no_content() {
    let read = RichText.read(&fold(&updates()));
    let printed = format!("{read:?}");
    for word in ["widget", "Then", "0123456789abcdef"] {
        assert!(!printed.contains(word), "{printed}");
    }
}
