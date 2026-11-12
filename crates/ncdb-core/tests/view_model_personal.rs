//! The view's model knows which fields are personal, and whom each is
//! about (RFC 0005, "Personal fields": `{ "person": "self" }` or the link
//! field that names the person), so that the fold can keep a personal
//! field's plaintext only while its person key is held and the person is
//! not erased (RFC 0006, "What the view stores in plaintext"; NOC-244).

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::model_lock::{ModelLock, Person};
use ncdb_core::view::Model;

const EXAMPLE: &str = include_str!("model_lock/rfc-0005-v2.lock.json");

fn model() -> Model {
    Model::from_lock(&ModelLock::from_json(EXAMPLE).expect("the example reads"))
        .expect("the example is valid")
}

#[test]
fn a_field_about_the_object_itself_is_personal_to_self() {
    let model = model();
    assert_eq!(model.person("people", "name"), Some(&Person::Itself));
    assert_eq!(model.person("people", "email"), Some(&Person::Itself));
}

#[test]
fn a_field_about_a_linked_person_names_its_link() {
    assert_eq!(
        model().person("tasks", "notesAboutAssignee"),
        Some(&Person::Link("assignee".to_owned()))
    );
}

#[test]
fn every_other_field_is_not_personal() {
    let model = model();
    assert_eq!(model.person("tasks", "estimate"), None);
    assert_eq!(model.person("tasks", "assignee"), None);
    assert_eq!(model.person("nothing", "here"), None);
}
