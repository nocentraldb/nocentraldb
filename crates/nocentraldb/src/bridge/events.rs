//! The events the worker sends unasked (NOC-176): the folds, the sync
//! status and the status of each space the SDK follows, as the module
//! docs of [`super`] describe them.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::task::Poll;

use ncdb_core::cbor::{self, Key, Value};

use super::{entries, lower_camel, role_name};
use crate::id::{ChangeId, SpaceId};
use crate::list::{SpaceInfo, SpaceModel};
use crate::status::SpaceStatus;
use crate::subscribe::FoldFeed;
use crate::sync::{Connection, SyncStatus, Watch};

/// The events of a database (NOC-176): after each fold, the space and
/// the collections it touched, so that the TypeScript SDK re-runs what its
/// subscriptions follow (RFC 0009, "Subscribe"), and the sync status
/// whenever it changed. Names, ids and counts, never values.
#[derive(Clone)]
pub struct Events {
    pub(super) feed: FoldFeed,
    pub(super) sync: Arc<Mutex<Watch<SyncStatus>>>,
    /// The spaces whose status the SDK follows.
    pub(super) spaces: Arc<Mutex<BTreeMap<SpaceId, Watch<SpaceStatus>>>>,
    /// The space list, once `spaces.follow` asked for it (NOC-315).
    pub(super) list: Arc<Mutex<Option<Watch<Vec<SpaceInfo>>>>>,
}

impl Events {
    fn sync(&self) -> std::sync::MutexGuard<'_, Watch<SyncStatus>> {
        self.sync
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub(super) fn spaces(
        &self,
    ) -> std::sync::MutexGuard<'_, BTreeMap<SpaceId, Watch<SpaceStatus>>> {
        self.spaces
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn list(&self) -> std::sync::MutexGuard<'_, Option<Watch<Vec<SpaceInfo>>>> {
        self.list
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The messages since the last take, in order: the folds as one
    /// `folded` message (see below), then the sync status as `{"event":
    /// "sync", "status": status}` when it changed. None once the database
    /// closed.
    pub fn take(&self) -> Vec<Vec<u8>> {
        if self.feed.is_closed() {
            return Vec::new();
        }
        let mut messages = Vec::new();
        messages.extend(self.folded());
        if let Some(status) = self.sync().take() {
            messages.push(cbor::encode(&entries([
                ("event", Value::Text("sync".into())),
                ("status", sync_status(&status)),
            ])));
        }
        if let Some(list) = self.list().as_mut().and_then(Watch::take) {
            messages.push(cbor::encode(&entries([
                ("event", Value::Text("spaces".into())),
                ("spaces", space_list(&list)),
            ])));
        }
        for (space, watch) in self.spaces().iter_mut() {
            if let Some(status) = watch.take() {
                messages.push(cbor::encode(&entries([
                    ("event", Value::Text("spaceStatus".into())),
                    ("space", Value::Text(space.to_string())),
                    ("status", space_status(&status)),
                ])));
            }
        }
        messages
    }

    /// The folds since the last take as one message, `{"event": "folded",
    /// "folds": [{"space": id, "collections": [text], "changes":
    /// [{"collection": text, "ids": [id], "origin": "local" | "remote"}]}]}`;
    /// `None` when nothing folded.
    fn folded(&self) -> Option<Vec<u8>> {
        let folds = self.feed.take();
        if folds.is_empty() {
            return None;
        }
        let folds = folds
            .into_iter()
            .map(|fold| {
                let changes = fold
                    .changes
                    .into_iter()
                    .map(|changed| {
                        entries([
                            ("collection", Value::Text(changed.collection)),
                            (
                                "ids",
                                Value::Array(
                                    changed
                                        .ids
                                        .iter()
                                        .map(|id| Value::Text(id.to_string()))
                                        .collect(),
                                ),
                            ),
                            ("origin", Value::Text(changed.origin.name().into())),
                        ])
                    })
                    .collect();
                entries([
                    ("space", Value::Text(fold.space.to_string())),
                    (
                        "collections",
                        Value::Array(fold.collections.into_iter().map(Value::Text).collect()),
                    ),
                    ("changes", Value::Array(changes)),
                ])
            })
            .collect();
        Some(cbor::encode(&entries([
            ("event", Value::Text("folded".into())),
            ("folds", Value::Array(folds)),
        ])))
    }

    /// Resolves when a message waits to be taken, or the database closed.
    pub fn wait(&self) -> impl std::future::Future<Output = ()> + use<> {
        let events = self.clone();
        std::future::poll_fn(move |cx| {
            if events.sync().changed(cx.waker()) {
                return Poll::Ready(());
            }
            if events
                .list()
                .as_ref()
                .is_some_and(|watch| watch.changed(cx.waker()))
            {
                return Poll::Ready(());
            }
            if events
                .spaces()
                .values()
                .any(|watch| watch.changed(cx.waker()))
            {
                return Poll::Ready(());
            }
            events.feed.poll_wait(cx)
        })
    }

    /// Whether the database these events follow is closed.
    pub fn is_closed(&self) -> bool {
        self.feed.is_closed()
    }

    /// Whether `other` follows the same database as these events.
    pub fn same(&self, other: &Events) -> bool {
        self.feed.same(&other.feed)
    }
}

/// The space list as `spaces.list` answers it and the `spaces` event
/// carries it: `[{"id": id, "role": text | null, "model": "ours" |
/// "another"}]` (RFC 0009, "Spaces").
pub(super) fn space_list(list: &[SpaceInfo]) -> Value {
    Value::Array(
        list.iter()
            .map(|info| {
                let role = info
                    .role
                    .map_or(Value::Null, |role| Value::Text(role_name(role).into()));
                let model = match info.model {
                    SpaceModel::Ours => "ours",
                    SpaceModel::Another => "another",
                };
                entries([
                    ("id", Value::Text(info.id.to_string())),
                    ("role", role),
                    ("model", Value::Text(model.into())),
                ])
            })
            .collect(),
    )
}

/// A sync status as the bridge sends it (RFC 0009, "Sync and status").
pub(super) fn sync_status(status: &SyncStatus) -> Value {
    let connection = match status.connection {
        Connection::Connected => "connected",
        Connection::Connecting => "connecting",
        Connection::Offline => "offline",
        Connection::Paused => "paused",
        Connection::None => "none",
    };
    entries([
        ("connection", Value::Text(connection.into())),
        ("unconfirmed", Value::Unsigned(status.unconfirmed)),
        ("refused", Value::Unsigned(status.refused)),
        ("epochChanged", Value::Bool(status.epoch_changed)),
        (
            "error",
            status
                .error
                .as_ref()
                .map_or(Value::Null, |error| Value::Text(error.clone())),
        ),
    ])
}

/// A space's status as the bridge sends it (RFC 0009, "Sync and status"):
/// `{"role": "reader" | "writer" | "admin" | "removed",
/// "initialSyncComplete": bool, "behind": u64, "gaps": [id], "parked":
/// {reason: u64}, "newestModel": u64 | null, "rejected": {reason: u64},
/// "refused": [{"change": id, "code": text}], "lostGrants": [id],
/// "equivocations": [id], "exposed": bool, "waitingForKey": bool,
/// "badPersonKeyRecords": [fingerprint], "provisionalErasures":
/// [{"change": id, "age": milliseconds, "canUndo": bool, "performer":
/// member, "notice": {"code": text, "text": text}}]}`. A reason is its
/// Rust name in lower camel case.
pub(super) fn space_status(status: &SpaceStatus) -> Value {
    let ids =
        |ids: &[ChangeId]| Value::Array(ids.iter().map(|id| Value::Text(id.to_string())).collect());
    let counts = |counts: Vec<(String, u64)>| {
        Value::Map(
            counts
                .into_iter()
                .map(|(reason, count)| (Key::Text(reason), Value::Unsigned(count)))
                .collect(),
        )
    };
    let role = status.role.map_or("removed", role_name);
    entries([
        ("role", Value::Text(role.into())),
        (
            "initialSyncComplete",
            Value::Bool(status.initial_sync_complete),
        ),
        ("behind", Value::Unsigned(status.behind)),
        ("gaps", ids(&status.gaps)),
        (
            "parked",
            counts(
                status
                    .parked
                    .iter()
                    .map(|(reason, count)| (lower_camel(&format!("{reason:?}")), *count))
                    .collect(),
            ),
        ),
        (
            "newestModel",
            status
                .newest_model
                .map_or(Value::Null, |version| Value::Unsigned(version.into())),
        ),
        (
            "rejected",
            counts(
                status
                    .rejected
                    .iter()
                    .map(|(reason, count)| (lower_camel(&format!("{reason:?}")), *count))
                    .collect(),
            ),
        ),
        (
            "refused",
            Value::Array(
                status
                    .refused
                    .iter()
                    .map(|refused| {
                        entries([
                            ("change", Value::Text(refused.change.to_string())),
                            ("code", Value::Text(refused.code.clone())),
                        ])
                    })
                    .collect(),
            ),
        ),
        ("lostGrants", ids(&status.lost_grants)),
        ("equivocations", ids(&status.equivocations)),
        ("exposed", Value::Bool(status.exposed)),
        ("waitingForKey", Value::Bool(status.waiting_for_key)),
        (
            "badPersonKeyRecords",
            Value::Array(
                status
                    .bad_person_key_records
                    .iter()
                    .map(|fingerprint| Value::Text(fingerprint.to_string()))
                    .collect(),
            ),
        ),
        (
            "provisionalErasures",
            Value::Array(
                status
                    .provisional_erasures
                    .iter()
                    .map(|erasure| {
                        entries([
                            ("change", Value::Text(erasure.change.to_string())),
                            (
                                "age",
                                Value::Unsigned(
                                    u64::try_from(erasure.age.as_millis()).unwrap_or(u64::MAX),
                                ),
                            ),
                            ("canUndo", Value::Bool(erasure.can_undo)),
                            // A member id's text form, 64 lowercase hex
                            // digits (Carsten, 2026-11-13, NOC-298).
                            ("performer", Value::Text(erasure.performer.to_string())),
                            (
                                "notice",
                                entries([
                                    (
                                        "code",
                                        Value::Text(lower_camel(&format!(
                                            "{:?}",
                                            erasure.notice.code
                                        ))),
                                    ),
                                    ("text", Value::Text(erasure.notice.text.into())),
                                ]),
                            ),
                        ])
                    })
                    .collect(),
            ),
        ),
    ])
}
