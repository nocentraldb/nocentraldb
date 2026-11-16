//! Why a model cannot be folded into a view, as a developer reads it: a
//! lock that breaks a rule on its own prints its refusals' lines, as
//! `ncdb model build` does, and each finding of the view's own check has a
//! text of its own, never the core's Debug form (NOC-343).

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::model_lock::Refusal;
use ncdb_core::view::{Error, Invalid};

fn unknown_field(index: &str, field: &str) -> Refusal {
    Refusal::IndexUnknownField {
        collection: "tasks".into(),
        index: index.into(),
        field: field.into(),
    }
}

#[test]
fn a_lock_that_breaks_rules_prints_each_refusals_lines() {
    let invalid = Invalid::Lock(vec![
        unknown_field("byPriority", "nope"),
        Refusal::IndexEqualTooLarge {
            collection: "tasks".into(),
            index: "open".into(),
            equal: 3,
            fields: 2,
        },
    ]);

    assert_eq!(
        invalid.to_string(),
        "index tasks.byPriority: tasks has no field nope.\n\
         index tasks.open: equal is 3, but the index has 2 fields.\n\
         No query could fix more fields than the index has."
    );
}

#[test]
fn each_finding_of_the_views_check_has_its_text() {
    let cases = [
        (
            Invalid::DuplicateIndex {
                collection: "tasks".into(),
                index: "open".into(),
            },
            "index tasks.open is declared twice.",
        ),
        (
            Invalid::UnknownCollection("open".into()),
            "index open names a collection the model does not have.",
        ),
        (
            Invalid::UnsortableKey("open".into()),
            "index open orders by a field without a single sortable value.",
        ),
        (
            Invalid::Equality("open".into()),
            "index open fixes more fields with equality than it orders by.",
        ),
        (
            Invalid::ReservedId,
            "model.id is the id of the model the SDK ships; the app's model needs an id of its own.",
        ),
    ];

    for (invalid, text) in cases {
        assert_eq!(invalid.to_string(), text);
    }
}

#[test]
fn the_views_error_prints_the_finding_not_its_debug_form() {
    let error = Error::Invalid(Invalid::Lock(vec![unknown_field("byPriority", "nope")]));

    assert_eq!(
        error.to_string(),
        "the model cannot be folded: index tasks.byPriority: tasks has no field nope."
    );
}
