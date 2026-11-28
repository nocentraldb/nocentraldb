//! An `ncdb:` link in a replacement that is no mention's form
//! `[@](ncdb:<space id>/<object id>)` is refused with `InvalidValue`, as a
//! first document from Markdown refuses it (NOC-536), so that a malformed
//! mention is never written as a plain link. The scheme is read in any
//! case, and a mention's link with a name as its text is refused too.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::rich_text::anchor::Error;

use crate::doc::{ma, p, s, t};
use crate::support::{Field, after, append, edited, replace};

const SPACE: &str = "0123456789abcdef0123456789abcdef";
const ID: &str = "fedcba9876543210fedcba9876543210";

fn malformed() -> Vec<String> {
    vec![
        format!("[@](ncdb:{SPACE})"),
        format!("[@](ncdb:/{ID})"),
        format!("[@](ncdb:{SPACE}/{})", ID.to_uppercase()),
        format!("[@](ncdb:{SPACE}/{ID}/more)"),
        format!("[@](NCDB:{SPACE}/{ID})"),
        format!("[Ada](ncdb:{SPACE}/{ID})"),
    ]
}

#[test]
fn a_replacement_with_a_malformed_ncdb_link_is_refused() {
    for link in malformed() {
        let mut field = Field::new(vec![p(vec![t("Ask about it.")])]);
        assert_eq!(
            edited(&mut field, &[replace("about it.", &format!("about it. {link}"))]),
            Err(Error::InvalidValue { edit: 0 }),
            "{link}"
        );
        assert_eq!(field.markdown(), "Ask about it.");
    }
}

#[test]
fn added_blocks_with_a_malformed_ncdb_link_are_refused_and_name_their_edit() {
    let link = format!("[Ada](ncdb:{SPACE}/{ID})");
    let mut field = Field::new(vec![p(vec![t("Intro.")])]);
    assert_eq!(
        edited(
            &mut field,
            &[
                replace("Intro.", "Start."),
                after("Intro.", &format!("Ask {link}.")),
            ]
        ),
        Err(Error::InvalidValue { edit: 1 })
    );
    assert_eq!(
        edited(&mut field, &[append(&format!("Ask {link}."))]),
        Err(Error::InvalidValue { edit: 0 })
    );
    assert_eq!(field.markdown(), "Intro.");
}

#[test]
fn an_ncdb_link_the_document_holds_already_does_not_stop_an_edit() {
    let href = format!("ncdb:{SPACE}");
    let mut field = Field::new(vec![p(vec![
        t("See "),
        ma("the space", "link", &[("href", s(&href))]),
        t(" soon."),
    ])]);
    assert_eq!(
        edited(&mut field, &[replace("soon.", "today.")]),
        Ok(format!("See [the space]({href}) today."))
    );
}
