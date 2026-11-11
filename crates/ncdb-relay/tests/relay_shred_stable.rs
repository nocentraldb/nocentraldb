//! RFC 0008, M20, "Effects that wait until a change is stable": the relay
//! deletes a shredded person key's record once the shred is stable in its
//! log, and keeps it while an admin's run does not reach a change that saw
//! the shred.

mod support;

use ncdb_core::change::Change;
use ncdb_core::ids::SpaceId;
use ncdb_core::membership::{self, Role};
use ncdb_relay::conformance::block_on;
use ncdb_relay::sqlite::SqliteStore;
use ncdb_relay::store::{PersonKeyId, PersonKeyRecord};
use ncdb_relay::{Pushed, Relay};
use support::space::{Space, id, key, other_account, person, wrapped};
use support::{push, push_one, relay};

const CREATOR: u8 = 0;
const WRITER: u8 = 1;
/// An admin of a second account, who signs off shreds.
const SECOND: u8 = 3;

/// A space on the relay with the creator and a second account's admin,
/// a writer who created the keys of persons 1 and 2, and both records
/// stored.
fn held() -> (Relay<SqliteStore>, support::TempDir, Space, Vec<Change>) {
    let (relay, dir) = relay();
    let mut space = Space::new(CREATOR);
    let genesis = space.genesis.clone();
    let writer = space.add(CREATOR, WRITER, Role::Writer, &[&genesis]);
    space.certify_by(SECOND, other_account(1));
    let second = space.add(CREATOR, SECOND, Role::Admin, &[&writer]);
    let one = space.person_key(WRITER, 1, 0xc1, &[&second]);
    let two = space.person_key(WRITER, 2, 0xc2, &[&one]);
    let held = vec![genesis, writer, second, one, two];
    let refs: Vec<&Change> = held.iter().collect();
    let pushed = push(&relay, CREATOR, space.space, &refs);
    assert!(pushed.iter().all(|p| matches!(p, Pushed::Accepted(_))));
    let records = vec![
        record(space.space, 1, space.first_generation),
        record(space.space, 2, space.first_generation),
    ];
    block_on(relay.put_person_keys(id(WRITER), space.space, records)).expect("both are stored");
    (relay, dir, space, held)
}

fn record(space: SpaceId, who: u8, generation: u64) -> PersonKeyRecord {
    let wrapped = wrapped(who, generation);
    let signature = membership::sign_record(&key(WRITER), space, person(who), generation, &wrapped);
    PersonKeyRecord {
        id: PersonKeyId(person(who)),
        generation,
        writer: id(WRITER),
        wrapped,
        signature: signature.to_vec(),
    }
}

/// The person-key ids whose records the relay holds.
fn stored(relay: &Relay<SqliteStore>, space: SpaceId) -> Vec<[u8; 16]> {
    block_on(relay.person_keys(id(WRITER), space))
        .expect("the writer reads the records")
        .into_iter()
        .map(|r| r.id.0)
        .collect()
}

fn accepted(relay: &Relay<SqliteStore>, uploader: u8, change: &Change) {
    assert!(matches!(
        push_one(relay, uploader, change),
        Pushed::Accepted(_)
    ));
}

#[test]
fn a_shred_no_admin_wrote_after_keeps_its_record() {
    let (relay, _dir, mut space, held) = held();
    let shred = space.shred(WRITER, 1, &other_account(1), &[&held[4]]);
    accepted(&relay, WRITER, &shred);
    assert_eq!(stored(&relay, space.space), [person(1), person(2)]);
}

#[test]
fn the_record_stays_while_one_admin_has_not_written_after_the_shred() {
    let (relay, _dir, mut space, held) = held();
    let shred = space.shred(WRITER, 1, &other_account(1), &[&held[4]]);
    accepted(&relay, WRITER, &shred);
    let by_creator = space.envelope(CREATOR, WRITER, &[&shred]);
    accepted(&relay, CREATOR, &by_creator);
    assert_eq!(stored(&relay, space.space), [person(1), person(2)]);
}

#[test]
fn the_record_is_deleted_once_the_shred_is_stable() {
    let (relay, _dir, mut space, held) = held();
    let shred = space.shred(WRITER, 1, &other_account(1), &[&held[4]]);
    accepted(&relay, WRITER, &shred);
    let by_creator = space.envelope(CREATOR, WRITER, &[&shred]);
    accepted(&relay, CREATOR, &by_creator);
    let by_second = space.envelope(SECOND, WRITER, &[&shred]);
    accepted(&relay, SECOND, &by_second);
    // Only the shredded key's record goes.
    assert_eq!(stored(&relay, space.space), [person(2)]);
}

#[test]
fn an_admin_change_after_a_gap_in_its_run_does_not_make_the_shred_stable() {
    // The second admin's change that saw the shred follows sealed changes
    // the relay never received, so its run in the relay's log has a gap.
    let (relay, _dir, mut space, held) = held();
    let shred = space.shred(WRITER, 1, &other_account(1), &[&held[4]]);
    accepted(&relay, WRITER, &shred);
    let by_creator = space.envelope(CREATOR, WRITER, &[&shred]);
    accepted(&relay, CREATOR, &by_creator);
    space.skip(SECOND, 2);
    let by_second = space.envelope(SECOND, WRITER, &[&shred]);
    accepted(&relay, SECOND, &by_second);
    assert_eq!(stored(&relay, space.space), [person(1), person(2)]);
}
