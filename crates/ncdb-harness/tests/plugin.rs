//! The harness checks a plugin field type (RFC 0011, "Checking a plugin:
//! ncdb-harness"; RFC 0004, "The plugin contract"): the test plugin
//! `test/counter` passes C1 to C7 and the round trip of its state; broken
//! variants of it, the fixtures RFC 0004 asks for (C6), are each caught.

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::BTreeMap;

use ncdb_harness::{Author, Config, Generator, Member, PluginOp, PluginState, Property, check};
use ncdb_plugin::{
    ConfigError, Data, FieldType, InvalidIntent, OperationId, Origin, Outcome, ParkReason, Written,
};
use ncdb_test_counter::{ADD, Counter};
use proptest::prelude::*;

/// Adds from -3 to 3, written through the plugin's own `write`, with an
/// arbitrary operation now and then (C5).
struct Adds;

impl<T: FieldType + Default> Generator<PluginState<T>> for Adds {
    type Raw = i8;

    fn raw(&self, _member: Member) -> BoxedStrategy<i8> {
        (-3i8..=3).boxed()
    }

    fn make(&self, author: &Author<'_, PluginState<T>>, raw: &i8) -> PluginOp {
        author
            .copy
            .write(&Data::Integer(i128::from(*raw)))
            .expect("an add of an integer")
            .remove(0)
    }

    fn malformed(&self) -> Option<BoxedStrategy<PluginOp>> {
        Some(
            (
                60u64..70,
                proptest::collection::btree_map(0u64..4, any::<i8>(), 0..3),
            )
                .prop_map(|(number, arguments)| PluginOp {
                    number,
                    arguments: arguments
                        .into_iter()
                        .map(|(key, n)| (key, Data::Integer(i128::from(n))))
                        .collect(),
                })
                .boxed(),
        )
    }
}

fn config() -> Config {
    Config {
        cases: 64,
        seed: 11,
        ..Config::default()
    }
}

#[test]
fn the_test_counter_meets_the_contract() {
    let result = check::<PluginState<Counter>, _>(&config(), &Adds);
    assert!(result.is_ok(), "{}", result.unwrap_err());
}

/// A broken counter whose state is a list in order of arrival, so two
/// copies with the same adds hold different bytes.
#[derive(Clone, Copy, Debug, Default)]
struct ArrivalOrder;

/// A broken counter whose `decode_state` loses the last add.
#[derive(Clone, Copy, Debug, Default)]
struct LosesTheLast;

macro_rules! counter_like {
    ($name:ident, $state:ty, $apply:expr, $decode:expr) => {
        impl FieldType for $name {
            type State = $state;
            type Operation = (OperationId, i128);

            fn configure(&self, _rule: u64, _config: Option<&Data>) -> Result<Self, ConfigError> {
                Ok($name)
            }
            fn state_bytes(&self, state: &Self::State) -> Vec<u8> {
                let adds = state.iter().map(|(id, n)| {
                    Data::Array(vec![Data::Bytes(id.change.0.to_vec()), Data::Integer(*n)])
                });
                Data::Array(adds.collect()).encode().unwrap()
            }
            fn decode_state(&self, bytes: &[u8]) -> Option<Self::State> {
                let Data::Array(adds) = Data::decode(bytes).ok()? else {
                    return None;
                };
                let all: Option<Vec<(OperationId, i128)>> = adds
                    .into_iter()
                    .map(|add| match add {
                        Data::Array(f) => match f.as_slice() {
                            [Data::Bytes(c), Data::Integer(n)] => Some((
                                OperationId {
                                    change: ncdb_plugin::ChangeId(c.as_slice().try_into().ok()?),
                                    position: 0,
                                },
                                *n,
                            )),
                            _ => None,
                        },
                        _ => None,
                    })
                    .collect();
                Some(($decode)(all?))
            }
            fn decode(
                &self,
                number: u64,
                arguments: &BTreeMap<u64, Data>,
                origin: Origin,
            ) -> Result<Self::Operation, ParkReason> {
                match (number, arguments.get(&1)) {
                    (ADD, Some(Data::Integer(n))) if arguments.len() == 1 => Ok((origin.id, *n)),
                    (ADD, _) => Err(ParkReason::MalformedArguments),
                    _ => Err(ParkReason::UnknownOperation),
                }
            }
            fn apply(&self, state: &mut Self::State, operation: &Self::Operation) {
                ($apply)(state, *operation)
            }
            fn read(&self, state: &Self::State) -> Data {
                Data::Integer(state.iter().map(|(_, n)| *n).sum())
            }
            fn outcome(&self, state: &Self::State, id: OperationId) -> Option<Outcome> {
                state
                    .iter()
                    .any(|(i, _)| *i == id)
                    .then_some(Outcome::Visible)
            }
            fn write(
                &self,
                _state: &Self::State,
                intent: &Data,
            ) -> Result<Vec<Written>, InvalidIntent> {
                Counter.write(&Default::default(), intent)
            }
        }
    };
}

counter_like!(
    ArrivalOrder,
    Vec<(OperationId, i128)>,
    |state: &mut Vec<(OperationId, i128)>, op: (OperationId, i128)| {
        if !state.contains(&op) {
            state.push(op);
        }
    },
    |all: Vec<(OperationId, i128)>| all
);

counter_like!(
    LosesTheLast,
    BTreeMap<OperationId, i128>,
    |state: &mut BTreeMap<OperationId, i128>, op: (OperationId, i128)| {
        state.insert(op.0, op.1);
    },
    |mut all: Vec<(OperationId, i128)>| {
        all.pop();
        all.into_iter().collect::<BTreeMap<_, _>>()
    }
);

#[test]
fn a_counter_whose_state_follows_arrival_breaks_c1() {
    let violation = check::<PluginState<ArrivalOrder>, _>(&config(), &Adds).unwrap_err();
    assert_eq!(violation.property, Property::C1, "{violation}");
}

#[test]
fn a_counter_whose_state_does_not_round_trip_is_caught() {
    let violation = check::<PluginState<LosesTheLast>, _>(&config(), &Adds).unwrap_err();
    assert_eq!(violation.property, Property::RoundTrip, "{violation}");
}

#[test]
fn an_operation_the_plugin_parks_reads_as_parked_on_every_copy() {
    let mut state = PluginState::<Counter>::default();
    let op = ncdb_harness::Op {
        id: 0,
        author: 0,
        body: PluginOp {
            number: 65,
            arguments: BTreeMap::new(),
        },
    };
    ncdb_harness::Mergeable::apply(&mut state, &op);
    assert_eq!(
        ncdb_harness::Mergeable::outcome(&state, 0),
        ncdb_harness::Outcome::Parked
    );
}
