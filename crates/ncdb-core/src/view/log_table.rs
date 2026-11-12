//! The member's log in the view's database (RFC 0002, "The log on disk";
//! RFC 0006, "Where the view lives"): each change's frame, its state and
//! its reason, written in the transaction that folds it, and read back
//! into the in-memory [`Log`] of each space when the view opens.

use std::collections::BTreeMap;

use rusqlite::{Connection, params};

use super::Error;
use crate::change::Change;
use crate::ids::{ChangeId, SpaceId};
use crate::log::{
    Log, OperationParkReason, ParkReason, ParkedOperation, RejectReason, RelayRefusal, State,
};

/// The `state` and `reason` columns of a state.
fn columns(state: &State) -> (&'static str, Option<&'static str>) {
    match state {
        State::Folded { .. } => ("folded", None),
        State::Parked(reason) => ("parked", Some(park_reason(*reason))),
        State::Rejected(reason) => ("rejected", Some(reject_reason(*reason))),
    }
}

/// Every reason, by the name the `reason` column holds.
const PARK: [(ParkReason, &str); 7] = [
    (ParkReason::UnknownSuite, "unknown_suite"),
    (ParkReason::UnknownKind, "unknown_kind"),
    (ParkReason::UnknownHeaderFormat, "unknown_header_format"),
    (ParkReason::UnknownHeaderKey, "unknown_header_key"),
    (ParkReason::UnknownPayloadFormat, "unknown_payload_format"),
    (ParkReason::UnknownPayloadKey, "unknown_payload_key"),
    (ParkReason::AnotherModel, "another_model"),
];
const REJECT: [(RejectReason, &str); 5] = [
    (RejectReason::UnknownAuthor, "unknown_author"),
    (RejectReason::BadSignature, "bad_signature"),
    (RejectReason::NotMember, "not_member"),
    (RejectReason::NoKey, "no_key"),
    (RejectReason::Undecryptable, "undecryptable"),
];
const OPERATION: [(OperationParkReason, &str); 6] = [
    (OperationParkReason::UnknownCollection, "unknown_collection"),
    (OperationParkReason::UnknownField, "unknown_field"),
    (OperationParkReason::UnknownKey, "unknown_key"),
    (OperationParkReason::UnknownOperation, "unknown_operation"),
    (OperationParkReason::UnknownArgument, "unknown_argument"),
    (
        OperationParkReason::MalformedArguments,
        "malformed_arguments",
    ),
];

fn name<T: PartialEq + Copy>(table: &[(T, &'static str)], reason: T) -> &'static str {
    table
        .iter()
        .find(|(r, _)| *r == reason)
        .map(|(_, name)| *name)
        .expect("every reason has a name")
}

fn parse<T: Copy>(table: &[(T, &'static str)], name: Option<&str>) -> Result<T, Error> {
    table
        .iter()
        .find(|(_, n)| Some(*n) == name)
        .map(|(reason, _)| *reason)
        .ok_or(Error::Corrupt("log reason"))
}

fn park_reason(reason: ParkReason) -> &'static str {
    name(&PARK, reason)
}

fn reject_reason(reason: RejectReason) -> &'static str {
    name(&REJECT, reason)
}

/// Inserts a change the log of its space holds as a new entry, with its
/// state, and with `written_at` when the member wrote it then.
pub(super) fn insert(
    conn: &Connection,
    log: &Log,
    id: ChangeId,
    written_at: Option<u64>,
) -> Result<(), Error> {
    let position = log.position_of(id).expect("the log holds the change");
    let change = log.get(position).expect("the log holds the change");
    let written_at = written_at.map(|at| i64::try_from(at).unwrap_or(i64::MAX));
    conn.execute(
        "INSERT INTO log (space_id, change_id, state, frame, written_at)
         VALUES (?1, ?2, '', ?3, ?4)",
        params![log.space().0, id.0, change.bytes(), written_at],
    )?;
    update(conn, log, id)
}

/// Writes the state the log of its space holds for a change, with its
/// parked operations.
pub(super) fn update(conn: &Connection, log: &Log, id: ChangeId) -> Result<(), Error> {
    let space = log.space().0;
    let state = log.state(id).expect("a received change has a state");
    let (column, reason) = columns(state);
    conn.execute(
        "UPDATE log SET state = ?3, reason = ?4 WHERE space_id = ?1 AND change_id = ?2",
        params![space, id.0, column, reason],
    )?;
    conn.execute(
        "DELETE FROM parked WHERE space_id = ?1 AND change_id = ?2",
        params![space, id.0],
    )?;
    if let State::Folded { parked } = state {
        for operation in parked {
            conn.execute(
                "INSERT INTO parked (space_id, change_id, position, reason)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    space,
                    id.0,
                    i64::try_from(operation.position).map_err(|_| Error::Corrupt("position"))?,
                    name(&OPERATION, operation.reason)
                ],
            )?;
        }
    }
    Ok(())
}

/// The log of every space the database holds, in local-position order,
/// with each change's state, whether the relay confirmed it and the
/// relay's refusal.
pub(super) fn load(conn: &Connection) -> Result<BTreeMap<SpaceId, Log>, Error> {
    let mut logs = BTreeMap::new();
    let mut statement = conn.prepare(
        "SELECT space_id, frame, state, reason, relay_refusal, relay_position IS NOT NULL FROM log
         ORDER BY local_position",
    )?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let space = SpaceId(row.get(0)?);
        let frame: Vec<u8> = row.get(1)?;
        let change = Change::decode(&frame).map_err(|_| Error::Corrupt("log frame"))?;
        let id = change.id();
        let log = logs.entry(space).or_insert_with(|| Log::new(space));
        log.append(change)
            .map_err(|_| Error::Corrupt("log space"))?;
        let reason: Option<String> = row.get(3)?;
        let reason = reason.as_deref();
        let state = match row.get::<_, String>(2)?.as_str() {
            "folded" => State::Folded {
                parked: parked(conn, space, id)?,
            },
            "parked" => State::Parked(parse(&PARK, reason)?),
            "rejected" => State::Rejected(parse(&REJECT, reason)?),
            _ => return Err(Error::Corrupt("log state")),
        };
        log.set_state(id, state);
        if let Some(refusal) = row.get::<_, Option<String>>(4)? {
            log.record_relay_refusal(id, RelayRefusal(refusal));
        }
        if row.get::<_, bool>(5)? {
            log.record_relay_confirmation(id);
        }
    }
    Ok(logs)
}

/// The parked operations of a folded change, in order.
fn parked(conn: &Connection, space: SpaceId, id: ChangeId) -> Result<Vec<ParkedOperation>, Error> {
    let mut statement = conn.prepare(
        "SELECT position, reason FROM parked WHERE space_id = ?1 AND change_id = ?2
         ORDER BY position",
    )?;
    let mut rows = statement.query(params![space.0, id.0])?;
    let mut parked = Vec::new();
    while let Some(row) = rows.next()? {
        let reason: String = row.get(1)?;
        let position: i64 = row.get(0)?;
        parked.push(ParkedOperation {
            position: u64::try_from(position).map_err(|_| Error::Corrupt("parked position"))?,
            reason: parse(&OPERATION, Some(&reason))?,
        });
    }
    Ok(parked)
}
