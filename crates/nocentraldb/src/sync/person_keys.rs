//! The person-key store at the relay, on the member's side (RFC 0003, "The
//! person key"; RFC 0007, "Keys and the person-key store"; NOC-206).
//!
//! A member that wrote a person-key entry hands its signed record to
//! syncing with [`put`]; the session puts the waiting records after it
//! pushed the space's changes, since the relay accepts a record only once
//! the log holds its entry. When a space is synced, the session fetches its
//! records, and the member holds each person key whose record [`accept`]s:
//! the writer's signature verifies, the log announces the record's
//! generation and hash, the record unwraps under that generation's key,
//! and the key matches the commitment in the log. A record that fails is
//! not held.
//!
//! The member keeps each record whose key it holds, as the relay stores
//! it, in its own database ([`hold`]; NOC-547), and a reopened database
//! holds those keys again from it ([`restore`]), with no relay: the record
//! unwraps under a space key the envelopes in the log give the member, so
//! no person key is on the disk in the clear. A connected member still
//! fetches the records again on every connection (NOC-222), for the keys
//! other members drew or re-wrapped.
//!
//! A record its writer signed that still fails, because it does not unwrap
//! under its generation or its key does not match the commitment, is
//! [`Outcome::Bad`]: the space status names the writer's account
//! (`SpaceStatus::bad_person_key_records`; NOC-222 point 2). A record whose
//! signature does not verify names no writer anyone vouched for, and one
//! the log does not announce yet, or under a generation whose key the
//! member lacks, may still hold later: those are only ignored.

use ncdb_core::ids::{MemberId, SpaceId};
use ncdb_core::membership::{record_hash, unwrap_person_key, verify_record};

use ncdb_core::view::{PersonKeyRecord, View};

use super::wire::Record;
use crate::db::{Inner, SpaceState};

/// Hands `records` of `space` to syncing to put, and holds each person
/// key whose record the member accepts, as its writer does. A rotation
/// that re-wrapped person keys calls it once its batch is folded (NOC-206
/// part 3).
/// Without a relay the device keeps the records all the same, so a
/// reopened database holds the keys again (NOC-547).
pub(crate) fn put(inner: &mut Inner, space: SpaceId, records: Vec<Record>) {
    let Inner {
        view, spaces, sync, ..
    } = inner;
    if let Some(state) = spaces.get_mut(&space) {
        for record in &records {
            // A record that cannot be kept is held for this session all the
            // same; the status names its key as missing after a reopen.
            let _ = hold(view, state, space, record);
        }
    }
    if let Some(sync) = sync.as_mut() {
        sync.puts.entry(space).or_default().extend(records);
    }
}

/// [`accept`]s `record` of `space`, and keeps it in the device's database
/// when the member holds its key (NOC-547). Says what `accept` did.
pub(crate) fn hold(
    view: &mut View,
    state: &mut SpaceState,
    space: SpaceId,
    record: &Record,
) -> Result<Outcome, ncdb_core::view::Error> {
    let outcome = accept(state, space, record);
    if outcome == Outcome::Held {
        view.keep_person_key_record(space, &stored(record))?;
    }
    Ok(outcome)
}

/// Holds again each person key of `space` whose record the device kept,
/// when the record still holds ([`accept`]), as a reopened database does
/// (NOC-547), and opens the space's personal fields with them. The record
/// of a key a shred destroyed meanwhile is forgotten.
pub(crate) fn restore(
    view: &mut View,
    space: SpaceId,
    state: &mut SpaceState,
) -> Result<(), ncdb_core::view::Error> {
    for record in view.person_key_records(space)? {
        let destroyed = state
            .checks
            .resolved()
            .person_key(record.person_key)
            .is_some_and(|key| key.destroyed());
        if destroyed {
            view.forget_person_key_records(space, record.person_key)?;
            continue;
        }
        accept(state, space, &from_stored(record));
    }
    crate::personal::refresh(view, space, state)
}

fn stored(record: &Record) -> PersonKeyRecord {
    PersonKeyRecord {
        person_key: record.person_key,
        generation: record.generation,
        writer: record.writer,
        wrapped: record.wrapped.clone(),
        signature: record.signature,
    }
}

fn from_stored(record: PersonKeyRecord) -> Record {
    Record {
        person_key: record.person_key,
        generation: record.generation,
        writer: record.writer,
        wrapped: record.wrapped,
        signature: record.signature,
    }
}

/// What a person-key entry announces of its record: the person-key id, the
/// generation and the record's hash.
type Announced = ([u8; 16], u64, [u8; 32]);

/// Splits the waiting `records` of `space` into those a session puts now
/// and those it keeps: a record waits while the relay has not confirmed
/// the person-key entry that announces it, since the relay refuses with
/// `bad_record`, and stores none of the request, a record whose entry its
/// log does not hold yet. An entry the member wrote while a push was on the
/// wire reaches the relay with the next push. A record no entry announces
/// is put, and the relay's refusal drops it.
pub(crate) fn ready(
    inner: &Inner,
    space: SpaceId,
    records: Vec<Record>,
) -> (Vec<Record>, Vec<Record>) {
    let Some(log) = inner.view.log(space) else {
        return (records, Vec::new());
    };
    let entries: Vec<(Announced, ncdb_core::ids::ChangeId)> = log
        .controls()
        .filter_map(|(change, _)| {
            let body = ncdb_core::membership::ControlBody::decode(&change.header().payload).ok()?;
            match body.entry {
                ncdb_core::membership::Entry::PersonKey(entry) => Some((
                    (entry.person_key, entry.generation, entry.record_hash),
                    change.id(),
                )),
                _ => None,
            }
        })
        .collect();
    records.into_iter().partition(|record| {
        let announced = (
            record.person_key,
            record.generation,
            record_hash(&record.wrapped),
        );
        let mut changes = entries
            .iter()
            .filter(|(key, _)| *key == announced)
            .peekable();
        changes.peek().is_none()
            || changes.any(|(_, change)| inner.view.relay_position(space, *change).is_some())
    })
}

/// What [`accept`] did with a record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Outcome {
    /// The member holds the record's person key.
    Held,
    /// The record's writer, whose account key this is, signed it, but it
    /// does not unwrap, or its key does not match the commitment.
    Bad([u8; 32]),
    /// The record was not checked to the end: the log does not announce
    /// it, its key was destroyed, its writer is unknown or did not sign it,
    /// or the member lacks its generation's key.
    Ignored,
}

/// Holds the person key of `record` in `state` when the record holds: its
/// writer, current or former, signed it; an accepted person-key entry
/// announces its generation and hash and no shred destroyed the key; it
/// unwraps under the generation's key, which the member holds; and the key
/// matches the commitment in the log. Says what it did.
pub(crate) fn accept(state: &mut SpaceState, space: SpaceId, record: &Record) -> Outcome {
    let resolved = state.checks.resolved();
    let Some(person) = resolved.person_key(record.person_key) else {
        return Outcome::Ignored;
    };
    let announced = person
        .records
        .contains(&(record.generation, record_hash(&record.wrapped)));
    let Some(commitment) = person
        .commitment
        .filter(|_| !person.destroyed() && announced)
    else {
        return Outcome::Ignored;
    };
    let Some(writer) = resolved.member(MemberId(record.writer)) else {
        return Outcome::Ignored;
    };
    let account = writer.certificate.account_key;
    let signed = verify_record(
        &writer.key,
        space,
        record.person_key,
        record.generation,
        &record.wrapped,
        &record.signature,
    );
    let Some(space_key) = state.checks.space_key(record.generation).filter(|_| signed) else {
        return Outcome::Ignored;
    };
    let unwrapped = unwrap_person_key(
        space_key,
        space,
        record.generation,
        record.person_key,
        &record.wrapped,
    );
    match unwrapped.filter(|key| key.commitment(space, record.person_key) == commitment) {
        Some(key) => {
            state.person_keys.insert(record.person_key, key);
            Outcome::Held
        }
        None => Outcome::Bad(account),
    }
}

/// Whether the log of `space` names a person key that the member does not
/// hold and that is not destroyed: its record may be at the relay now, so
/// the session fetches the records again.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub(crate) fn missing(inner: &Inner, space: SpaceId) -> bool {
    inner.spaces.get(&space).is_some_and(|state| {
        state
            .checks
            .resolved()
            .person_keys()
            .any(|(id, key)| !key.destroyed() && !state.person_keys.contains_key(&id))
    })
}
