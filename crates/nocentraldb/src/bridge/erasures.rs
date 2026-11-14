//! The bridge's `erasures.list` (NOC-315, slice 2; RFC 0009, "Erasures").

use ncdb_core::cbor::Value;

use crate::erasures::Erasure;

/// An erasure as the bridge sends it.
pub(super) fn erasure(_erasure: &Erasure) -> Value {
    Value::Null
}
