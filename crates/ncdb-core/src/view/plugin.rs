//! Folding one operation on a plugin field (RFC 0011, "What the core does
//! around it"): the core parks what it can judge without the plugin, an
//! operation number below 64 and arguments that are no map with integer
//! keys, drops argument keys from 64 up, and lets the plugin's code decode
//! the stored state, apply the operation and encode the state again. A
//! park reason of the plugin becomes the core's; bytes the plugin did not
//! write are corrupt.

use std::collections::BTreeMap;

use ncdb_plugin::{ApplyError, Data, ParkReason};

use super::Error;
use super::model::PluginField;
use crate::cbor::{Key, Value};
use crate::change::Operation;
use crate::field_op::Origin;
use crate::log::OperationParkReason;

/// The first operation number of plugin types (RFC 0004, "Operation
/// numbers").
pub(crate) const FIRST_PLUGIN_OPERATION: u64 = 64;

/// The first argument key a member may ignore (RFC 0004, "Operations a
/// member does not understand").
const FIRST_MAY_IGNORE: u64 = 64;

/// The state's bytes after `operation`, or why it parks; a stored state
/// the plugin did not write is an error of the view.
pub(super) fn apply(
    field: &PluginField,
    state: &[u8],
    operation: &Operation,
    origin: Origin,
) -> Result<Result<Vec<u8>, OperationParkReason>, Error> {
    if operation.op < FIRST_PLUGIN_OPERATION {
        return Ok(Err(OperationParkReason::UnknownOperation));
    }
    let Some(arguments) = arguments(operation.value.as_ref()) else {
        return Ok(Err(OperationParkReason::MalformedArguments));
    };
    match field
        .code
        .apply(Some(state), operation.op, &arguments, origin)
    {
        Ok(bytes) => Ok(Ok(bytes)),
        Err(ApplyError::Park(reason)) => Ok(Err(park(reason))),
        Err(ApplyError::Corrupt) => Err(Error::Corrupt("plugin state")),
    }
}

/// The arguments by key, those from 64 up dropped; `None` when they are
/// missing, no map, keyed by a text, or hold a value that is no `Data`.
pub(crate) fn arguments(value: Option<&Value>) -> Option<BTreeMap<u64, Data>> {
    let Some(Value::Map(entries)) = value else {
        return None;
    };
    entries
        .iter()
        .filter_map(|(key, value)| match key {
            Key::Unsigned(key) if *key >= FIRST_MAY_IGNORE => None,
            Key::Unsigned(key) => Some(Data::from_value(value.clone()).ok().map(|v| (*key, v))),
            Key::Text(_) => Some(None),
        })
        .collect()
}

fn park(reason: ParkReason) -> OperationParkReason {
    match reason {
        ParkReason::UnknownOperation => OperationParkReason::UnknownOperation,
        ParkReason::UnknownArgument => OperationParkReason::UnknownArgument,
        ParkReason::MalformedArguments => OperationParkReason::MalformedArguments,
    }
}
