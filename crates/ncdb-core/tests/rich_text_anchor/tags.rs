//! Parts outside the vocabulary (RFC 0013, "Parts outside the
//! vocabulary"): a tag in the blocks a quote touches, which the
//! replacement keeps as it is, stays untouched, and the text around it is
//! edited. An edit that drops, changes or adds a tag is refused.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::rich_text::anchor::Error;

use crate::doc::{N, el, ma, p, s, t};
use crate::support::{Field, edited, holds, replace};

const ROCKET: &str = "<emoji name=\"rocket\"></emoji>";

fn rocket() -> N {
    el("emoji", &[("name", s("rocket"))], vec![])
}

#[test]
fn a_tag_in_the_quoted_block_stays_and_the_text_beside_it_is_edited() {
    let mut field = Field::new(vec![p(vec![t("Ship on Friday "), rocket(), t(" soon.")])]);
    assert_eq!(
        edited(&mut field, &[replace("Friday", "Monday")]),
        Ok(format!("Ship on Monday {ROCKET} soon."))
    );
}

#[test]
fn a_quote_that_holds_a_tag_its_replacement_keeps_is_edited_on_both_sides() {
    let mut field = Field::new(vec![p(vec![t("Ship on Friday "), rocket(), t(" soon.")])]);
    assert_eq!(
        edited(
            &mut field,
            &[replace(
                &format!("Friday {ROCKET} soon."),
                &format!("Monday {ROCKET} after the review.")
            )]
        ),
        Ok(format!("Ship on Monday {ROCKET} after the review."))
    );
    assert!(holds(&field.tree(), "rocket"));
}

#[test]
fn a_mark_outside_the_vocabulary_stays_on_its_text() {
    let mut field = Field::new(vec![p(vec![
        t("Read "),
        ma("this part", "comment", &[("id", s("c1"))]),
        t(" first."),
    ])]);
    let tag = "<comment id=\"c1\">this part</comment>";
    assert_eq!(
        edited(
            &mut field,
            &[replace(
                &format!("Read {tag} first."),
                &format!("Please read {tag} today.")
            )]
        ),
        Ok(format!("Please read {tag} today."))
    );
    assert!(holds(&field.tree(), "c1"));
}

#[test]
fn a_block_tag_inside_the_quote_stays_between_edited_blocks() {
    let callout = "<callout tone=\"warn\">Careful.</callout>";
    let mut field = Field::new(vec![
        p(vec![t("Before.")]),
        el("callout", &[("tone", s("warn"))], vec![t("Careful.")]),
        p(vec![t("After.")]),
    ]);
    assert_eq!(
        edited(
            &mut field,
            &[replace(
                &format!("Before.\n\n{callout}\n\nAfter."),
                &format!("Earlier.\n\n{callout}\n\nLater.")
            )]
        ),
        Ok(format!("Earlier.\n\n{callout}\n\nLater."))
    );
}

#[test]
fn a_quote_may_end_where_a_tag_ends() {
    let mut field = Field::new(vec![p(vec![t("Go "), rocket()])]);
    assert_eq!(
        edited(
            &mut field,
            &[replace(&format!("Go {ROCKET}"), &format!("Now go {ROCKET}"))]
        ),
        Ok(format!("Now go {ROCKET}"))
    );
}

#[test]
fn a_replacement_that_drops_changes_or_adds_a_tag_is_refused() {
    let quote = format!("Friday {ROCKET} soon.");
    for replacement in [
        "Monday soon.".to_owned(),
        "Monday <emoji name=\"tada\"></emoji> soon.".to_owned(),
        format!("Monday {ROCKET} {ROCKET} soon."),
        format!("Monday {ROCKET} <b>soon</b>."),
    ] {
        let mut field =
            Field::new(vec![p(vec![t("Ship on Friday "), rocket(), t(" soon.")])]);
        assert_eq!(
            edited(&mut field, &[replace(&quote, &replacement)]),
            Err(Error::Unsupported { edit: 0 })
        );
        assert_eq!(field.markdown(), format!("Ship on Friday {ROCKET} soon."));
    }
}
