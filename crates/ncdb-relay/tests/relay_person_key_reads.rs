//! A person-key put reads only the stored records of the persons it
//! touches, not every record of the space (NOC-435): its cost no longer
//! grows with the persons in the space.

mod support;

use ncdb_core::change::Change;
use ncdb_core::membership::{self, Entry, PersonKeyEntry, Role};
use ncdb_relay::conformance::block_on;
use ncdb_relay::sqlite::SqliteStore;
use ncdb_relay::store::{PersonKeyId, PersonKeyRecord, RelayStore};
use ncdb_relay::{Pushed, Relay};
use support::counting::Counting;
use support::space::{Space, id, key};
use support::{TempDir, config};

const CREATOR: u8 = 0;
const WRITER: u8 = 1;
const PERSONS: u8 = 8;

fn person(n: u8) -> [u8; 16] {
    [0x50 ^ n; 16]
}

fn wrapped(n: u8) -> Vec<u8> {
    vec![n; 12]
}

/// The person-key entry of person `n`, by the writer, after `deps`.
fn entry(space: &mut Space, n: u8, deps: &[&Change]) -> Change {
    let entry = PersonKeyEntry {
        person_key: person(n),
        commitment: [0xc0; 32],
        generation: space.first_generation,
        record_hash: membership::record_hash(&wrapped(n)),
    };
    space.write(WRITER, Entry::PersonKey(entry), deps)
}

fn record(space: &Space, n: u8) -> PersonKeyRecord {
    let generation = space.first_generation;
    let signature = membership::sign_record(
        &key(WRITER),
        space.space,
        person(n),
        generation,
        &wrapped(n),
    );
    PersonKeyRecord {
        id: PersonKeyId(person(n)),
        generation,
        writer: id(WRITER),
        wrapped: wrapped(n),
        signature: signature.to_vec(),
    }
}

fn push(relay: &Relay<Counting<SqliteStore>>, by: u8, space: &Space, change: &Change) {
    let pushed = block_on(relay.push(id(by), space.space, vec![change.bytes().to_vec()]));
    assert!(matches!(pushed.results[..], [Pushed::Accepted(_)]), "{pushed:?}");
}

#[test]
fn a_put_reads_only_the_records_of_the_persons_it_touches() {
    let dir = TempDir::new();
    let relay = Relay::new(Counting::new(dir.store()), config());
    let mut space = Space::new(CREATOR);
    let genesis = space.genesis.clone();
    let writer = space.add(CREATOR, WRITER, Role::Writer, &[&genesis]);
    push(&relay, CREATOR, &space, &genesis);
    push(&relay, CREATOR, &space, &writer);
    let mut head = writer;
    for n in 0..=PERSONS {
        let change = entry(&mut space, n, &[&head]);
        push(&relay, WRITER, &space, &change);
        head = change;
    }
    for n in 0..PERSONS {
        block_on(relay.put_person_keys(id(WRITER), space.space, vec![record(&space, n)]))
            .expect("the relay stores the record");
    }
    let stored = block_on(relay.store().person_keys(space.space)).expect("a read");
    assert_eq!(stored.len(), usize::from(PERSONS));
    relay.store().take_records();

    block_on(relay.put_person_keys(id(WRITER), space.space, vec![record(&space, PERSONS)]))
        .expect("the relay stores a new person's record");
    assert_eq!(
        relay.store().take_records(),
        0,
        "a new person's put reads no stored record"
    );

    block_on(relay.put_person_keys(id(WRITER), space.space, vec![record(&space, 3)]))
        .expect("the relay stores the record again");
    assert_eq!(
        relay.store().take_records(),
        1,
        "a put of one stored person reads that person's record alone"
    );
}
