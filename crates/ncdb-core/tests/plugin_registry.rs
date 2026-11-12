//! The registry the core builds from the plugins it is given (RFC 0011, "A
//! plugin and the registry"; Carsten, 2026-11-12, NOC-232): it maps a type
//! id and rule to the code that implements it, and refuses two plugins for
//! one namespace and a plugin for the namespace `ncdb`.

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::plugins::{Registry, RegistryError};
use ncdb_plugin::Plugin;
use ncdb_test_counter::Counter;

#[test]
fn the_registry_holds_each_type_id_and_rule_of_its_plugins() {
    let registry = Registry::new([
        ncdb_test_counter::plugin(),
        Plugin::new("acme")
            .field_type("counter", 1, Counter)
            .field_type("counter", 2, Counter),
    ])
    .unwrap();
    assert!(registry.holds("test/counter", 1));
    assert!(registry.holds("acme/counter", 1));
    assert!(registry.holds("acme/counter", 2));
    assert!(!registry.holds("test/counter", 2));
    assert!(!registry.holds("acme/vote", 1));
    assert!(!registry.holds("counter", 1));
}

#[test]
fn an_empty_registry_holds_nothing() {
    let registry = Registry::default();
    assert!(!registry.holds("test/counter", 1));
}

#[test]
fn two_plugins_for_one_namespace_are_refused() {
    let refused = Registry::new([
        ncdb_test_counter::plugin(),
        Plugin::new("test").field_type("other", 1, Counter),
    ]);
    assert_eq!(
        refused.err(),
        Some(RegistryError::DuplicateNamespace("test".into()))
    );
}

#[test]
fn a_plugin_for_the_built_in_namespace_is_refused() {
    let refused = Registry::new([Plugin::new("ncdb").field_type("string", 1, Counter)]);
    assert_eq!(
        refused.err(),
        Some(RegistryError::ReservedNamespace("ncdb".into()))
    );
}

#[test]
fn a_refusal_names_the_namespace_in_its_message() {
    let duplicate = RegistryError::DuplicateNamespace("acme".into()).to_string();
    let reserved = RegistryError::ReservedNamespace("ncdb".into()).to_string();
    assert!(duplicate.contains("acme"), "{duplicate}");
    assert!(reserved.contains("ncdb"), "{reserved}");
}
