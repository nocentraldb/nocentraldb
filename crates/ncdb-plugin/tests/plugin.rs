//! A plugin (RFC 0011, "A plugin and the registry"): one crate is one
//! namespace, and its `plugin()` lists its types by name with the rule
//! versions it implements, each with the code that implements it.

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::BTreeMap;

use ncdb_plugin::{
    ConfigError, Data, FieldType, InvalidIntent, OperationId, Origin, Outcome, ParkReason, Plugin,
};

/// A field type that keeps nothing: enough to be listed.
#[derive(Clone, Debug)]
struct Nothing;

impl FieldType for Nothing {
    type State = ();
    type Operation = ();

    fn configure(&self, _rule: u64, _config: Option<&Data>) -> Result<Nothing, ConfigError> {
        Ok(Nothing)
    }
    fn state_bytes(&self, _state: &()) -> Vec<u8> {
        Vec::new()
    }
    fn decode_state(&self, _bytes: &[u8]) -> Option<()> {
        Some(())
    }
    fn decode(
        &self,
        _number: u64,
        _arguments: &BTreeMap<u64, Data>,
        _origin: Origin,
    ) -> Result<(), ParkReason> {
        Err(ParkReason::UnknownOperation)
    }
    fn apply(&self, _state: &mut (), _operation: &()) {}
    fn read(&self, _state: &()) -> Data {
        Data::Null
    }
    fn outcome(&self, _state: &(), _id: OperationId) -> Option<Outcome> {
        None
    }
    fn write(
        &self,
        _state: &(),
        _intent: &Data,
    ) -> Result<Vec<(u64, BTreeMap<u64, Data>)>, InvalidIntent> {
        Err(InvalidIntent::new("writes nothing"))
    }
}

#[test]
fn a_plugin_lists_its_namespace_and_each_type_with_its_rules() {
    let plugin = Plugin::new("acme")
        .field_type("counter", 1, Nothing)
        .field_type("counter", 2, Nothing)
        .field_type("vote", 1, Nothing);
    assert_eq!(plugin.namespace(), "acme");
    assert_eq!(
        plugin.types().collect::<Vec<_>>(),
        [("counter", 1), ("counter", 2), ("vote", 1)]
    );
}

#[test]
fn the_same_type_and_rule_listed_twice_keeps_the_last() {
    let plugin = Plugin::new("acme")
        .field_type("counter", 1, Nothing)
        .field_type("counter", 1, Nothing);
    assert_eq!(plugin.types().collect::<Vec<_>>(), [("counter", 1)]);
}

#[test]
fn a_park_reason_a_plugin_returns_is_one_of_rfc_0004s_three() {
    let reasons = [
        ParkReason::UnknownOperation,
        ParkReason::UnknownArgument,
        ParkReason::MalformedArguments,
    ];
    assert_eq!(reasons.len(), 3);
}

#[test]
fn an_invalid_intent_and_a_config_error_say_why_without_content() {
    assert_eq!(
        InvalidIntent::new("an add takes an integer").to_string(),
        "an add takes an integer"
    );
    assert_eq!(
        ConfigError::new("the step must be a positive integer").to_string(),
        "the step must be a positive integer"
    );
}
