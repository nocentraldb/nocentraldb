//! RFC 0008, "A shred is final when accepted" (the owner's NOC-224,
//! NOC-229): the relay deletes a shredded person key's record as soon as it
//! accepts the shred, since no later change can reject an accepted shred.

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

/// The person-key ids whose records the relay holds, as the creator reads
/// them.
fn stored(relay: &Relay<SqliteStore>, space: SpaceId) -> Vec<[u8; 16]> {
    block_on(relay.person_keys(id(CREATOR), space))
        .expect("the creator reads the records")
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
fn accepting_a_shred_deletes_its_record_at_once() {
    let (relay, _dir, mut space, held) = held();
    let shred = space.shred(WRITER, 1, &other_account(1), &[&held[4]]);

    accepted(&relay, WRITER, &shred);

    assert_eq!(stored(&relay, space.space), [person(2)]);
}

/// A revocation of the performer that the shred did not see arrives later.
/// It does not reject the shred, so the record stays deleted and no record
/// of the shredded id is taken again.
#[test]
fn a_later_revocation_of_the_performer_does_not_bring_the_record_back() {
    let (relay, _dir, mut space, held) = held();
    let shred = space.shred(WRITER, 1, &other_account(1), &[&held[4]]);
    accepted(&relay, WRITER, &shred);
    let before_shred = shred.header().sequence - 1;
    let removal = space.remove(CREATOR, WRITER, before_shred, &[&held[2]]);

    accepted(&relay, CREATOR, &removal);

    assert_eq!(stored(&relay, space.space), [person(2)]);
    let again = vec![record(space.space, 1, space.first_generation)];
    assert!(block_on(relay.put_person_keys(id(CREATOR), space.space, again)).is_err());
}
