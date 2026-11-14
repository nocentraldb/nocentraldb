//! The durability of the copy and the view's report as the bridge sends
//! them (NOC-315, slice 3; RFC 0006, "What the SDK reports"; RFC 0009, "Sync
//! and status"; the shapes the owner decided on NOC-317, Carsten,
//! 2026-11-14).

use ncdb_core::cbor::Value;

use super::entries;
use crate::durability::{Durability, Storage};
use crate::view_report::ViewStatus;

/// `{"storage": "persistent" | "best-effort" | "temporary", "unconfirmed":
/// u64, "oldestUnconfirmedAt": milliseconds since the epoch | null,
/// "atRisk": bool}`.
pub(super) fn durability(durability: &Durability) -> Value {
    let storage = match durability.storage {
        Storage::Persistent => "persistent",
        Storage::BestEffort => "best-effort",
        Storage::Temporary => "temporary",
    };
    entries([
        ("storage", Value::Text(storage.into())),
        ("unconfirmed", Value::Unsigned(durability.unconfirmed)),
        (
            "oldestUnconfirmedAt",
            durability
                .oldest_unconfirmed_at
                .map_or(Value::Null, Value::Unsigned),
        ),
        ("atRisk", Value::Bool(durability.at_risk)),
    ])
}

/// `{"rebuild": {"folded": u64, "total": u64} | null}`.
pub(super) fn view(view: &ViewStatus) -> Value {
    entries([(
        "rebuild",
        view.rebuild.map_or(Value::Null, |progress| {
            entries([
                ("folded", Value::Unsigned(progress.folded)),
                ("total", Value::Unsigned(progress.total)),
            ])
        }),
    )])
}
