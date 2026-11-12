//! A shred is final when accepted (RFC 0008, "A shred is final when
//! accepted"), so no sealed change, a writer's or an admin's, makes the
//! relay read and resolve the log again for a shred.

mod support;

use ncdb_core::change::Change;
use ncdb_core::ids::SpaceId;
use ncdb_core::membership::{self, Role};
use ncdb_relay::conformance::block_on;
use ncdb_relay::sqlite::SqliteStore;
use ncdb_relay::store::{PersonKeyId, PersonKeyRecord};
use ncdb_relay::{Pushed, Relay};
use support::counting::Counting;
use support::space::{Space, id, key, other_account, person, wrapped};
use support::{TempDir, config};

type Store = Counting<SqliteStore>;

const CREATOR: u8 = 0;
const WRITER: u8 = 1;
/// An admin of a second account, who signs off shreds.
const SECOND: u8 = 3;

fn push(relay: &Relay<Store>, uploader: u8, change: &Change) -> Pushed {
    let space = change.header().space;
    let frames = vec![change.bytes().to_vec()];
    block_on(relay.push(id(uploader), space, frames))
        .results
        .remove(0)
}

fn accepted(relay: &Relay<Store>, uploader: u8, change: &Change) {
    assert!(matches!(push(relay, uploader, change), Pushed::Accepted(_)));
}

fn record(space: SpaceId, generation: u64) -> PersonKeyRecord {
    let wrapped = wrapped(1, generation);
    let signature = membership::sign_record(&key(WRITER), space, person(1), generation, &wrapped);
    PersonKeyRecord {
        id: PersonKeyId(person(1)),
        generation,
        writer: id(WRITER),
        wrapped,
        signature: signature.to_vec(),
    }
}

/// A space whose writer shredded person 1's key after its record was
/// stored.
fn waiting() -> (Relay<Store>, TempDir, Space, Change) {
    let dir = TempDir::new();
    let relay = Relay::new(Counting::new(dir.store()), config());
    let mut space = Space::new(CREATOR);
    let genesis = space.genesis.clone();
    let writer = space.add(CREATOR, WRITER, Role::Writer, &[&genesis]);
    space.certify_by(SECOND, other_account(1));
    let second = space.add(CREATOR, SECOND, Role::Admin, &[&writer]);
    let entry = space.person_key(WRITER, 1, 0xc1, &[&second]);
    for (who, change) in [
        (CREATOR, &genesis),
        (CREATOR, &writer),
        (CREATOR, &second),
        (WRITER, &entry),
    ] {
        accepted(&relay, who, change);
    }
    let put = vec![record(space.space, space.first_generation)];
    block_on(relay.put_person_keys(id(WRITER), space.space, put)).expect("the record is stored");
    let shred = space.shred(WRITER, 1, &other_account(1), &[&entry]);
    accepted(&relay, WRITER, &shred);
    (relay, dir, space, shred)
}

fn stored(relay: &Relay<Store>, space: SpaceId) -> usize {
    block_on(relay.person_keys(id(WRITER), space))
        .expect("the writer reads the records")
        .len()
}

#[test]
fn a_writers_sealed_change_does_not_make_the_relay_read_the_log() {
    let (relay, _dir, mut space, _) = waiting();
    relay.store().take_reads();
    let sealed = space.sealed(WRITER, Vec::new());
    accepted(&relay, WRITER, &sealed);
    assert_eq!(relay.store().take_reads(), 0);
    assert_eq!(stored(&relay, space.space), 0);
}

#[test]
fn an_admins_sealed_change_does_not_make_the_relay_read_the_log_either() {
    let (relay, _dir, mut space, _) = waiting();
    relay.store().take_reads();
    let sealed = space.sealed(CREATOR, Vec::new());
    accepted(&relay, CREATOR, &sealed);
    assert_eq!(relay.store().take_reads(), 0);
}
