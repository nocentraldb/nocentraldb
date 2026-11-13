//! The bridge's sync status and the status of a space (NOC-176, slice 4;
//! RFC 0009, "Sync and status"): what it answers, and the events it sends.

use ncdb_core::cbor::Value;

use super::{Bridge, LOCK, as_text, code, event, field, map, ok, opened, taken, text, write_one};

/// The `sync` status a message carries.
fn sync_of(message: &Value) -> Value {
    field(message, "sync")
        .or_else(|| field(message, "status"))
        .unwrap_or_else(|| panic!("no status: {message:?}"))
        .clone()
}

#[test]
fn open_answers_the_first_sync_status_as_rfc_0009_names_it() {
    let mut bridge = Bridge::new();
    let answer = ok(
        &mut bridge,
        "open",
        map(&[("lock", text(LOCK)), ("inMemory", Value::Bool(true))]),
    );
    assert_eq!(
        sync_of(&answer),
        map(&[
            ("connection", text("none")),
            ("unconfirmed", Value::Unsigned(0)),
            ("refused", Value::Unsigned(0)),
            ("epochChanged", Value::Bool(false)),
            ("error", Value::Null),
        ])
    );
}

#[test]
fn a_write_yields_a_sync_event_once_the_status_changed() {
    let mut bridge = opened();
    let events = bridge.events().expect("events once open");
    assert!(event(&events, "sync").is_none(), "open answered the status");
    let space = ok(&mut bridge, "spaces.create", map(&[]));
    let before = match event(&events, "sync") {
        Some(message) => field(&sync_of(&message), "unconfirmed").cloned(),
        None => Some(Value::Unsigned(0)),
    };
    let Some(Value::Unsigned(before)) = before else {
        panic!("{before:?}")
    };
    write_one(
        &mut bridge,
        &space,
        map(&[
            ("op", text("create")),
            ("collection", text("tasks")),
            ("values", map(&[("title", text("a"))])),
        ]),
    );
    let status = sync_of(&event(&events, "sync").expect("a sync event"));
    assert_eq!(
        field(&status, "unconfirmed"),
        Some(&Value::Unsigned(before + 1))
    );
    assert_eq!(field(&status, "connection"), Some(&text("none")));
    assert!(event(&events, "sync").is_none(), "taken");
}

#[test]
fn a_pause_reports_paused_and_a_resume_syncs_again() {
    let mut bridge = Bridge::new();
    let answer = ok(
        &mut bridge,
        "open",
        map(&[
            ("lock", text(LOCK)),
            ("inMemory", Value::Bool(true)),
            ("relay", text("ws://127.0.0.1:9")),
        ]),
    );
    assert_ne!(
        field(&sync_of(&answer), "connection"),
        Some(&text("none")),
        "{answer:?}"
    );
    let events = bridge.events().expect("events once open");
    events.take();
    assert_eq!(ok(&mut bridge, "sync.pause", map(&[])), Value::Null);
    let paused = event(&events, "sync").expect("a sync event");
    assert_eq!(
        field(&sync_of(&paused), "connection"),
        Some(&text("paused"))
    );
    assert_eq!(ok(&mut bridge, "sync.resume", map(&[])), Value::Null);
    let resumed = event(&events, "sync").expect("a sync event");
    assert_ne!(
        field(&sync_of(&resumed), "connection"),
        Some(&text("paused"))
    );
    ok(&mut bridge, "close", map(&[]));
}

#[test]
fn a_change_the_relay_does_not_hold_is_not_confirmed() {
    let mut bridge = opened();
    let space = ok(&mut bridge, "spaces.create", map(&[]));
    let written = write_one(
        &mut bridge,
        &space,
        map(&[
            ("op", text("create")),
            ("collection", text("tasks")),
            ("values", map(&[("title", text("a"))])),
        ]),
    );
    let change = field(&written, "change").expect("a change").clone();
    assert_eq!(
        ok(&mut bridge, "sync.isConfirmed", map(&[("change", change)])),
        Value::Bool(false)
    );
    assert_eq!(
        code(
            &mut bridge,
            "sync.isConfirmed",
            map(&[("change", text("nope"))])
        ),
        "invalidValue"
    );
}

#[test]
fn a_closed_database_sends_no_more_events() {
    let mut bridge = opened();
    let events = bridge.events().expect("events once open");
    ok(&mut bridge, "close", map(&[]));
    assert!(taken(&events).is_empty());
}

// NOC-176 slice 4: a space's status (RFC 0009, "Sync and status").

#[test]
fn space_status_answers_rfc_0009s_values_for_a_new_space() {
    let mut bridge = opened();
    let space = ok(&mut bridge, "spaces.create", map(&[]));
    let status = ok(&mut bridge, "space.status", map(&[("space", space)]));
    let empty = Value::Array(Vec::new());
    assert_eq!(
        status,
        map(&[
            ("role", text("admin")),
            ("initialSyncComplete", Value::Bool(false)),
            ("behind", Value::Unsigned(0)),
            ("gaps", empty.clone()),
            ("parked", map(&[])),
            ("newestModel", Value::Null),
            ("rejected", map(&[])),
            ("refused", empty.clone()),
            ("lostGrants", empty.clone()),
            ("equivocations", empty.clone()),
            ("exposed", Value::Bool(false)),
            ("waitingForKey", Value::Bool(false)),
            ("badPersonKeyRecords", empty.clone()),
            ("provisionalErasures", empty),
        ])
    );
}

#[test]
fn space_status_of_an_unknown_space_is_not_found() {
    let mut bridge = opened();
    assert_eq!(
        code(
            &mut bridge,
            "space.status",
            map(&[("space", text(&"ab".repeat(16)))])
        ),
        "notFound"
    );
}

#[test]
fn a_followed_space_yields_a_space_status_event_when_its_status_changed() {
    let mut bridge = Bridge::new();
    ok(
        &mut bridge,
        "open",
        map(&[
            ("lock", text(LOCK)),
            ("inMemory", Value::Bool(true)),
            ("relay", text("ws://127.0.0.1:9")),
        ]),
    );
    let events = bridge.events().expect("events once open");
    let space = ok(&mut bridge, "spaces.create", map(&[]));
    ok(
        &mut bridge,
        "space.status",
        map(&[("space", space.clone())]),
    );
    events.take();
    assert!(event(&events, "spaceStatus").is_none(), "unchanged");
    {
        // As a pull that reached the relay's head would.
        let db = bridge.db.as_ref().expect("open");
        let id: crate::id::SpaceId = as_text(&space).parse().expect("an id");
        let mut inner = db.lock();
        inner.sync.as_mut().expect("syncs").pulled.insert(id.0);
        inner.status.notify();
    }
    let message = event(&events, "spaceStatus").expect("a spaceStatus event");
    assert_eq!(field(&message, "space"), Some(&space));
    assert_eq!(
        field(
            field(&message, "status").expect("a status"),
            "initialSyncComplete"
        ),
        Some(&Value::Bool(true))
    );
    ok(&mut bridge, "close", map(&[]));
}
