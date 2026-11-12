//! A plugin for the tests of NoCentralDB (RFC 0011): the namespace `test`
//! with one field type, `counter`, rule 1. Not published.
//!
//! An add, operation 64 with the argument `{1: n}`, adds the integer `n`.
//! The state is the set of adds received, so the sum is the same in any
//! order of arrival and an add received twice counts once (RFC 0004, C1
//! and C2). Every add is visible in the read value. The type takes no
//! `config`, or an empty map.

use std::collections::BTreeMap;

use ncdb_plugin::{
    ChangeId, ConfigError, Data, FieldType, InvalidIntent, OperationId, Origin, Outcome,
    ParkReason, Plugin,
};

/// The namespace of the plugin.
pub const NAMESPACE: &str = "test";
/// Operation 64: add.
pub const ADD: u64 = 64;

/// The plugin: `test/counter`, rule 1.
pub fn plugin() -> Plugin {
    Plugin::new(NAMESPACE).field_type("counter", 1, Counter)
}

/// The counter field type.
#[derive(Clone, Copy, Debug, Default)]
pub struct Counter;

impl FieldType for Counter {
    /// Each add received, by its operation id.
    type State = BTreeMap<OperationId, i128>;
    /// One add.
    type Operation = (OperationId, i128);

    fn configure(&self, rule: u64, config: Option<&Data>) -> Result<Counter, ConfigError> {
        match (rule, config) {
            (1, None) => Ok(Counter),
            (1, Some(Data::Map(map))) if map.is_empty() => Ok(Counter),
            (1, Some(_)) => Err(ConfigError::new("the counter takes no options")),
            _ => Err(ConfigError::new("the counter implements rule 1 only")),
        }
    }

    fn state_bytes(&self, state: &Self::State) -> Vec<u8> {
        let adds = state.iter().map(|(id, n)| {
            Data::Array(vec![
                Data::Bytes(id.change.0.to_vec()),
                Data::Integer(i128::from(id.position)),
                Data::Integer(*n),
            ])
        });
        Data::Array(adds.collect())
            .encode()
            .expect("every add was decoded from an integer that fits")
    }

    fn decode_state(&self, bytes: &[u8]) -> Option<Self::State> {
        let Data::Array(adds) = Data::decode(bytes).ok()? else {
            return None;
        };
        adds.into_iter()
            .map(|add| match add {
                Data::Array(fields) => match fields.as_slice() {
                    [Data::Bytes(change), Data::Integer(position), Data::Integer(n)] => Some((
                        OperationId {
                            change: ChangeId(change.as_slice().try_into().ok()?),
                            position: u64::try_from(*position).ok()?,
                        },
                        *n,
                    )),
                    _ => None,
                },
                _ => None,
            })
            .collect()
    }

    fn decode(
        &self,
        number: u64,
        arguments: &BTreeMap<u64, Data>,
        origin: Origin,
    ) -> Result<Self::Operation, ParkReason> {
        if number != ADD {
            return Err(ParkReason::UnknownOperation);
        }
        if arguments.keys().any(|key| *key != 1) {
            return Err(ParkReason::UnknownArgument);
        }
        match arguments.get(&1) {
            Some(Data::Integer(n)) => Ok((origin.id, *n)),
            _ => Err(ParkReason::MalformedArguments),
        }
    }

    fn apply(&self, state: &mut Self::State, operation: &Self::Operation) {
        state.insert(operation.0, operation.1);
    }

    fn read(&self, state: &Self::State) -> Data {
        // Saturating, so that no set of adds, however large, panics (RFC
        // 0004, C5).
        Data::Integer(state.values().fold(0, |sum, n| sum.saturating_add(*n)))
    }

    fn outcome(&self, state: &Self::State, id: OperationId) -> Option<Outcome> {
        state.contains_key(&id).then_some(Outcome::Visible)
    }

    fn write(
        &self,
        _state: &Self::State,
        intent: &Data,
    ) -> Result<Vec<(u64, BTreeMap<u64, Data>)>, InvalidIntent> {
        match intent {
            Data::Integer(n) => Ok(vec![(ADD, BTreeMap::from([(1, Data::Integer(*n))]))]),
            _ => Err(InvalidIntent::new("a counter adds an integer")),
        }
    }
}
