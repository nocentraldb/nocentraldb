//! NOC-497's measurement: what one warm push costs the relay, at 100,
//! 1,000, 5,000 and 10,000 persons in a space. A space here is a genesis,
//! the writer's add and a chain of person-key entries by the writer, as
//! the SDK writes them (NOC-492's space).
//!
//! For each size it builds the log through one relay, which then
//! remembers the space, as a warm Lambda instance does. On that relay it
//! times, one by one, the push of a person-key entry and the put of its
//! record, and then the push of another kind of control change: the
//! creator adding a member. It prints the median of each and asserts
//! nothing about the times, which depend on the machine; it is ignored
//! because building a log of 10,000 persons takes about half a minute in
//! a release build and far longer in a debug one. Run it with `cargo test --release -p ncdb-relay --test relay_warm_push
//! -- --ignored --nocapture`, and `NCDB_PERSONS=100,1000` for other sizes.

mod support;

use std::time::{Duration, Instant};

use ncdb_core::change::Change;
use ncdb_core::membership::{self, Entry, PersonKeyEntry, Role};
use ncdb_relay::conformance::block_on;
use ncdb_relay::sqlite::SqliteStore;
use ncdb_relay::store::{PersonKeyId, PersonKeyRecord};
use ncdb_relay::{Pushed, Relay};
use support::space::{Space, Who, id, key};
use support::{TempDir, config};

const CREATOR: Who = 0;
const WRITER: Who = 1;

/// How many warm pushes of each kind it times per size.
const PERSON_PUSHES: u32 = 21;
const ADDS: u8 = 9;

/// The person-key id of person `n`.
fn person(n: u32) -> [u8; 16] {
    let mut id = [0x50; 16];
    id[..4].copy_from_slice(&n.to_be_bytes());
    id
}

fn wrapped(n: u32, generation: u64) -> Vec<u8> {
    let mut bytes = n.to_be_bytes().to_vec();
    bytes.extend(generation.to_be_bytes());
    bytes
}

/// The person-key entry of person `n`, by the writer, after `deps`.
fn entry(space: &mut Space, n: u32, deps: &[&Change]) -> Change {
    let generation = space.first_generation;
    let entry = PersonKeyEntry {
        person_key: person(n),
        commitment: [0xc0; 32],
        generation,
        record_hash: membership::record_hash(&wrapped(n, generation)),
    };
    space.write(WRITER, Entry::PersonKey(entry), deps)
}

fn record(space: &Space, n: u32) -> PersonKeyRecord {
    let generation = space.first_generation;
    let wrapped = wrapped(n, generation);
    let signature =
        membership::sign_record(&key(WRITER), space.space, person(n), generation, &wrapped);
    PersonKeyRecord {
        id: PersonKeyId(person(n)),
        generation,
        writer: id(WRITER),
        wrapped,
        signature: signature.to_vec(),
    }
}

fn pushed(relay: &Relay<SqliteStore>, who: Who, space: &Space, change: &Change) -> Duration {
    let start = Instant::now();
    let frames = vec![change.bytes().to_vec()];
    let pushed = block_on(relay.push(id(who), space.space, frames)).results;
    let took = start.elapsed();
    assert!(matches!(pushed[..], [Pushed::Accepted(_)]), "{pushed:?}");
    took
}

fn median(mut times: Vec<Duration>) -> f64 {
    times.sort();
    times[times.len() / 2].as_secs_f64() * 1e3
}

/// The median warm push of a person-key entry, put of its record and
/// push of an add, in milliseconds, in a space of `persons` persons.
fn measure(persons: u32) -> (f64, f64, f64) {
    let dir = TempDir::new();
    let relay = Relay::new(dir.store(), config());
    let mut space = Space::new(CREATOR);
    let genesis = space.genesis.clone();
    let writer = space.add(CREATOR, WRITER, Role::Writer, &[&genesis]);
    pushed(&relay, CREATOR, &space, &genesis);
    pushed(&relay, CREATOR, &space, &writer);
    let mut head = writer;
    let mut n = 0;
    while n < persons {
        let mut frames = Vec::new();
        for _ in 0..500.min(persons - n) {
            let change = entry(&mut space, n, &[&head]);
            frames.push(change.bytes().to_vec());
            head = change;
            n += 1;
        }
        let pushed = block_on(relay.push(id(WRITER), space.space, frames)).results;
        assert!(pushed.iter().all(|p| matches!(p, Pushed::Accepted(_))));
    }
    let (mut pushes, mut puts, mut adds) = (Vec::new(), Vec::new(), Vec::new());
    for _ in 0..PERSON_PUSHES {
        let change = entry(&mut space, n, &[&head]);
        pushes.push(pushed(&relay, WRITER, &space, &change));
        let start = Instant::now();
        block_on(relay.put_person_keys(id(WRITER), space.space, vec![record(&space, n)]))
            .expect("the relay stores the record");
        puts.push(start.elapsed());
        head = change;
        n += 1;
    }
    for subject in 2..2 + ADDS {
        let add = space.add(CREATOR, subject, Role::Reader, &[&head]);
        adds.push(pushed(&relay, CREATOR, &space, &add));
        head = add;
    }
    (median(pushes), median(puts), median(adds))
}

#[test]
#[ignore = "a measurement: run it with --release --ignored --nocapture"]
fn what_a_warm_push_costs_the_relay() {
    let sizes: Vec<u32> = match std::env::var("NCDB_PERSONS") {
        Ok(sizes) => sizes
            .split(',')
            .map(|n| n.parse().expect("a number"))
            .collect(),
        Err(_) => vec![100, 1_000, 5_000, 10_000],
    };
    println!("persons | person-key push (ms) | its put (ms) | add push (ms)");
    for persons in sizes {
        let (push, put, add) = measure(persons);
        println!("{persons:7} | {push:20.2} | {put:12.2} | {add:13.2}");
    }
}
