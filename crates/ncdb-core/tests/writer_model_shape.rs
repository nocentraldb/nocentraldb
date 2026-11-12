//! Which values of payload key 64 name a model (RFC 0002, "The writer's
//! model"): a map with key 1, 16 bytes, and key 2, a model version of 1
//! and up. Any other value has the wrong shape, and its change is parked
//! as "another model". Keys of the map other than 1 and 2 are ignored, so
//! a later field of the model can be a new key of it. The owner decides
//! both open points on NOC-202; this builds pm's recommendation.

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::BTreeMap;

use ncdb_core::cbor::{Key, Value};
use ncdb_core::change::{WriterModel, WrongShape};

fn map(entries: Vec<(u64, Value)>) -> Value {
    Value::Map(
        entries
            .into_iter()
            .map(|(key, value)| (Key::Unsigned(key), value))
            .collect::<BTreeMap<_, _>>(),
    )
}

fn id() -> Value {
    Value::Bytes(vec![7; 16])
}

#[test]
fn a_map_of_a_16_byte_id_and_a_version_names_a_model() {
    let value = map(vec![(1, id()), (2, Value::Unsigned(3))]);

    assert_eq!(
        WriterModel::from_value(&value),
        Ok(WriterModel {
            id: [7; 16],
            version: 3
        })
    );
}

#[test]
fn keys_other_than_1_and_2_are_ignored() {
    let value = map(vec![
        (1, id()),
        (2, Value::Unsigned(3)),
        (3, Value::Text("a later field".into())),
    ]);

    assert_eq!(
        WriterModel::from_value(&value),
        Ok(WriterModel {
            id: [7; 16],
            version: 3
        })
    );
}

#[test]
fn version_0_has_the_wrong_shape() {
    let value = map(vec![(1, id()), (2, Value::Unsigned(0))]);

    assert_eq!(WriterModel::from_value(&value), Err(WrongShape));
}

#[test]
fn other_values_have_the_wrong_shape() {
    let wrong = [
        Value::Unsigned(1),
        Value::Bytes(vec![7; 16]),
        map(vec![(1, id())]),
        map(vec![(2, Value::Unsigned(1))]),
        map(vec![
            (1, Value::Bytes(vec![7; 15])),
            (2, Value::Unsigned(1)),
        ]),
        map(vec![
            (1, Value::Bytes(vec![7; 17])),
            (2, Value::Unsigned(1)),
        ]),
        map(vec![(1, Value::Text("id".into())), (2, Value::Unsigned(1))]),
        map(vec![(1, id()), (2, Value::Negative(0))]),
    ];

    for value in &wrong {
        assert_eq!(WriterModel::from_value(value), Err(WrongShape), "{value:?}");
    }
}
