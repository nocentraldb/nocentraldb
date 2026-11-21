//! NOC-427's measurement: what one more person costs the relay, at 100,
//! 400 and 800 persons in a space. A new person is a person-key entry (a
//! control change) and then its person-key record, as the SDK writes
//! them. It prints, per size, the time of the push and of the put, how
//! often and how many frame bytes the relay read for them, and the
//! process's resident memory once the space holds them all (Linux only). It is ignored: run
//! it with `cargo test --release -p ncdb-relay --test
//! relay_person_key_cost -- --ignored --nocapture`.

mod support;

use std::time::{Duration, Instant};

use ncdb_core::change::Change;
use ncdb_core::membership::{self, Entry, PersonKeyEntry, Role};
use ncdb_relay::conformance::block_on;
use ncdb_relay::store::{PersonKeyId, PersonKeyRecord};
use ncdb_relay::{Pushed, Relay};
use support::counting::Counting;
use support::space::{Space, id, key};
use support::{TempDir, config};

const CREATOR: u8 = 0;
const WRITER: u8 = 1;

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

/// What one more person cost.
struct Cost {
    push: Duration,
    put: Duration,
    reads: usize,
    bytes: usize,
}

/// Adds persons one by one up to `persons`, and returns what each of the
/// last `last` cost, on average.
fn measure(persons: u32, last: u32) -> Cost {
    let dir = TempDir::new();
    let relay = Relay::new(Counting::new(dir.store()), config());
    let mut space = Space::new(CREATOR);
    let genesis = space.genesis.clone();
    let writer = space.add(CREATOR, WRITER, Role::Writer, &[&genesis]);
    let mut head = writer.clone();
    for change in [&genesis, &writer] {
        let frames = vec![change.bytes().to_vec()];
        block_on(relay.push(id(CREATOR), space.space, frames));
    }
    let mut cost = Cost {
        push: Duration::ZERO,
        put: Duration::ZERO,
        reads: 0,
        bytes: 0,
    };
    for n in 0..persons {
        let change = entry(&mut space, n, &[&head]);
        let measured = n >= persons - last;
        relay.store().take_reads();
        relay.store().take_bytes();
        let start = Instant::now();
        let frames = vec![change.bytes().to_vec()];
        let pushed = block_on(relay.push(id(WRITER), space.space, frames)).results;
        let push = start.elapsed();
        assert!(matches!(pushed[..], [Pushed::Accepted(_)]), "{pushed:?}");
        let start = Instant::now();
        block_on(relay.put_person_keys(id(WRITER), space.space, vec![record(&space, n)]))
            .expect("the relay stores the record");
        let put = start.elapsed();
        if measured {
            cost.push += push;
            cost.put += put;
            cost.reads += relay.store().take_reads();
            cost.bytes += relay.store().take_bytes().0;
        }
        head = change;
    }
    cost.push /= last;
    cost.put /= last;
    cost.reads /= last as usize;
    cost.bytes /= last as usize;
    cost
}

/// The process's resident memory in MB, where Linux reports it.
fn resident_mb() -> f64 {
    let statm = std::fs::read_to_string("/proc/self/statm").unwrap_or_default();
    let pages: f64 = statm
        .split_whitespace()
        .nth(1)
        .and_then(|p| p.parse().ok())
        .unwrap_or(0.0);
    pages * 4096.0 / 1e6
}

#[test]
#[ignore = "a measurement: run it with --release --ignored --nocapture"]
fn what_one_more_person_costs_the_relay() {
    println!(
        "persons | push (ms) | put (ms) | reads of the log | frame bytes read | resident (MB)"
    );
    for persons in [100, 400, 800] {
        let cost = measure(persons, 10);
        println!(
            "{persons:7} | {:9.2} | {:8.2} | {:16} | {:16} | {:.0}",
            cost.push.as_secs_f64() * 1e3,
            cost.put.as_secs_f64() * 1e3,
            cost.reads,
            cost.bytes,
            resident_mb(),
        );
    }
}
