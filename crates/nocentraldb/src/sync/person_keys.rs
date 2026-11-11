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
//! **What this does not do yet.** The member holds its person keys in
//! memory only, and fetches them again on every connection: where a member
//! keeps them on the device is not decided. A record that fails is ignored
//! without naming its writer, which RFC 0003 asks the SDK to report.

use ncdb_core::ids::{MemberId, SpaceId};
use ncdb_core::membership::{record_hash, unwrap_person_key, verify_record};

use super::wire::Record;
use crate::db::{Inner, SpaceState};

/// Hands `records` of `space` to syncing to put, and holds each person
/// key whose record the member accepts, as its writer does. The rotation
/// that re-wraps records calls it (NOC-206 part 3); until then only the
/// tests do.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn put(inner: &mut Inner, space: SpaceId, records: Vec<Record>) {
    if let Some(state) = inner.spaces.get_mut(&space) {
        for record in &records {
            accept(state, space, record);
        }
    }
    if let Some(sync) = inner.sync.as_mut() {
        sync.puts.entry(space).or_default().extend(records);
    }
}

/// Holds the person key of `record` in `state` when the record holds: its
/// writer, current or former, signed it; an accepted person-key entry
/// announces its generation and hash and no shred destroyed the key; it
/// unwraps under the generation's key, which the member holds; and the key
/// matches the commitment in the log. Says whether it held.
pub(crate) fn accept(state: &mut SpaceState, space: SpaceId, record: &Record) -> bool {
    let resolved = state.checks.resolved();
    let Some(person) = resolved.person_key(record.person_key) else {
        return false;
    };
    let announced = person
        .records
        .contains(&(record.generation, record_hash(&record.wrapped)));
    let Some(commitment) = person
        .commitment
        .filter(|_| person.shred.is_none() && announced)
    else {
        return false;
    };
    let Some(writer) = resolved.member(MemberId(record.writer)) else {
        return false;
    };
    let signed = verify_record(
        &writer.key,
        space,
        record.person_key,
        record.generation,
        &record.wrapped,
        &record.signature,
    );
    let Some(space_key) = state.checks.space_key(record.generation).filter(|_| signed) else {
        return false;
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
            true
        }
        None => false,
    }
}
