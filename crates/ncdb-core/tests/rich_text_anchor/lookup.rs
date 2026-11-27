//! The lookup (RFC 0013, "An agent: anchored edits", step 2): a quote must
//! occur exactly once in the Markdown of the version the agent read, with
//! both ends on text, never inside markup.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::rich_text::anchor::Error;

use crate::doc::{m, p, t};
use crate::support::{Agent, Field, edited, replace};

fn friday() -> Field {
    Field::new(vec![
        p(vec![t("Intro.")]),
        p(vec![t("We ship on "), m("Fri", &["bold"]), t("day.")]),
        p(vec![t("Outro.")]),
    ])
}

#[test]
fn a_quote_that_does_not_occur_is_refused_with_anchor_not_found() {
    let mut field = friday();
    assert_eq!(
        edited(&mut field, &[replace("We ship on Friday.", "Monday.")]),
        Err(Error::AnchorNotFound { edit: 0 })
    );
}

#[test]
fn a_quote_that_occurs_twice_is_refused_with_anchor_ambiguous_and_the_number_of_matches() {
    let mut field = Field::new(vec![
        p(vec![t("Ship it.")]),
        p(vec![t("Wait.")]),
        p(vec![t("Ship it.")]),
    ]);
    assert_eq!(
        edited(&mut field, &[replace("Ship it.", "Hold it.")]),
        Err(Error::AnchorAmbiguous {
            edit: 0,
            matches: 2
        })
    );
    // Quoting more finds it once.
    assert_eq!(
        edited(
            &mut field,
            &[replace("Wait.\n\nShip it.", "Wait.\n\nHold it.")]
        ),
        Ok("Ship it.\n\nWait.\n\nHold it.".to_owned())
    );
}

#[test]
fn an_error_names_the_edit_it_refuses() {
    let mut field = friday();
    assert_eq!(
        edited(
            &mut field,
            &[replace("Intro.", "Start."), replace("Nowhere", "x")]
        ),
        Err(Error::AnchorNotFound { edit: 1 })
    );
    // Nothing of a refused call is written.
    assert_eq!(
        field.markdown(),
        "Intro.\n\nWe ship on **Fri**day.\n\nOutro."
    );
}

#[test]
fn a_quote_whose_end_falls_inside_markup_is_not_found() {
    // The quote opens the bold "Fri" but does not close it.
    let mut field = friday();
    assert_eq!(
        edited(&mut field, &[replace("on **Fri", "on **Mon")]),
        Err(Error::AnchorNotFound { edit: 0 })
    );
    // It starts inside the bold run and ends outside it.
    assert_eq!(
        edited(&mut field, &[replace("Fri**day", "Mon**day")]),
        Err(Error::AnchorNotFound { edit: 0 })
    );
    // It ends between the two characters of the closing `**`.
    assert_eq!(
        edited(&mut field, &[replace("**Fri*", "**Mon*")]),
        Err(Error::AnchorNotFound { edit: 0 })
    );
}

#[test]
fn a_quote_that_starts_inside_an_escape_is_not_found() {
    // The Markdown is `5 \* 3`; `* 3` occurs in it, but starts between the
    // backslash and the character it escapes.
    let mut field = Field::new(vec![p(vec![t("Take 5 * 3.")])]);
    assert_eq!(field.markdown(), "Take 5 \\* 3.");
    assert_eq!(
        edited(&mut field, &[replace("* 3.", "* 4.")]),
        Err(Error::AnchorNotFound { edit: 0 })
    );
    assert_eq!(
        edited(&mut field, &[replace("5 \\* 3.", "5 \\* 4.")]),
        Ok("Take 5 \\* 4.".to_owned())
    );
}

#[test]
fn a_quote_whose_ends_fall_on_text_or_around_whole_markup_is_found() {
    let mut field = friday();
    assert_eq!(
        edited(&mut field, &[replace("Fri", "Mon")]),
        Ok("Intro.\n\nWe ship on **Mon**day.\n\nOutro.".to_owned())
    );
    assert_eq!(
        edited(&mut field, &[replace("**Mon**day", "Tuesday")]),
        Ok("Intro.\n\nWe ship on Tuesday.\n\nOutro.".to_owned())
    );
}

#[test]
fn an_empty_quote_is_not_found() {
    let mut field = friday();
    assert_eq!(
        edited(&mut field, &[replace("", "x")]),
        Err(Error::AnchorNotFound { edit: 0 })
    );
}

#[test]
fn the_edits_of_one_call_apply_in_order_each_seeing_the_ones_before() {
    let mut field = friday();
    assert_eq!(
        edited(
            &mut field,
            &[replace("Intro.", "Start."), replace("Start.", "Begin.")]
        ),
        Ok("Begin.\n\nWe ship on **Fri**day.\n\nOutro.".to_owned())
    );
}

#[test]
fn the_lookup_depends_only_on_the_version_read() {
    // After the read, the person types inside the quoted sentence (F12):
    // the quote is looked up in the version read, and both edits stay.
    let mut field = Field::new(vec![p(vec![t("We ship on Friday.")])]);
    let mut agent = Agent::new();
    let reading = agent.read(&field);
    let typed = crate::rich_text_support::type_text(&field.person, 0, 14, "-");
    field.person(typed);
    assert_eq!(field.markdown(), "We ship on Fri-day.");
    let written = agent
        .edit(
            &reading,
            &[replace("We ship on Friday.", "We ship on Monday.")],
        )
        .unwrap();
    field.agent(&written);
    assert_eq!(field.markdown(), "We ship on Mon-day.");
}

#[test]
fn errors_and_edits_print_no_text() {
    let edit = replace("secret quote", "secret replacement");
    let error = Error::AnchorAmbiguous {
        edit: 3,
        matches: 2,
    };
    for printed in [format!("{edit:?}"), format!("{error:?}"), error.to_string()] {
        assert!(!printed.contains("secret"), "{printed}");
    }
    assert!(error.to_string().contains('2'));
}
