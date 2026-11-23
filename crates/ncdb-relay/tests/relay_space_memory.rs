//! NOC-492's measurement: the memory the relay keeps for one remembered
//! space, and the most one request on it allocates at once, at 100, 1,000,
//! 5,000 and 10,000 persons. A space here is a genesis, the writer's add
//! and a chain of person-key entries by the writer, as the SDK writes them.
//!
//! For each size it builds the log through a relay, then opens a cold
//! relay on it and lets it take one more person: a push and a put, which
//! read and resolve the space from scratch and keep it in memory. It
//! prints the bytes allocated and still held once that request is done
//! (the remembered space), the most allocated at once during it (the
//! request's work), and the growth of the process's resident memory over
//! it (Linux only). This test binary counts every allocation, so it holds
//! this one measurement only. It is ignored: run it with `cargo test
//! --release -p ncdb-relay --test relay_space_memory -- --ignored
//! --nocapture`.

mod support;

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use ncdb_core::change::Change;
use ncdb_core::membership::{self, Entry, PersonKeyEntry, Role};
use ncdb_relay::conformance::block_on;
use ncdb_relay::store::{PersonKeyId, PersonKeyRecord};
use ncdb_relay::{Pushed, Relay};
use support::space::{Space, id, key};
use support::{TempDir, config};

/// The system allocator, counting the bytes allocated now and the most
/// allocated at once since the last [`Counting::reset`].
struct Counting {
    now: AtomicUsize,
    peak: AtomicUsize,
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: the caller's contract for `alloc` is passed on unchanged.
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            let now = self.now.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
            self.peak.fetch_max(now, Ordering::Relaxed);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: the caller's contract for `dealloc` is passed on unchanged.
        unsafe { System.dealloc(ptr, layout) };
        self.now.fetch_sub(layout.size(), Ordering::Relaxed);
    }
}

impl Counting {
    /// Starts a new peak from the bytes allocated now, and returns them.
    fn reset(&self) -> usize {
        let now = self.now.load(Ordering::Relaxed);
        self.peak.store(now, Ordering::Relaxed);
        now
    }
}

#[global_allocator]
static ALLOCATED: Counting = Counting {
    now: AtomicUsize::new(0),
    peak: AtomicUsize::new(0),
};

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

/// The process's resident memory in bytes, where Linux reports it.
fn resident() -> usize {
    let statm = std::fs::read_to_string("/proc/self/statm").unwrap_or_default();
    let pages: usize = statm
        .split_whitespace()
        .nth(1)
        .and_then(|p| p.parse().ok())
        .unwrap_or(0);
    pages * 4096
}

/// What a remembered space of some persons takes.
struct Taken {
    /// Bytes allocated by the cold request and still held after it.
    held: usize,
    /// The most the cold request allocated at once.
    peak: usize,
    /// The growth of resident memory over the cold request.
    resident: usize,
    /// The time of the cold request.
    millis: f64,
}

/// Builds a space of `persons` persons, and measures a cold relay's
/// request that adds one more.
fn measure(persons: u32) -> Taken {
    let dir = TempDir::new();
    let mut space = Space::new(CREATOR);
    let genesis = space.genesis.clone();
    let writer = space.add(CREATOR, WRITER, Role::Writer, &[&genesis]);
    let mut head = writer.clone();
    {
        let relay = Relay::new(dir.store(), config());
        let frames = vec![genesis.bytes().to_vec(), writer.bytes().to_vec()];
        let pushed = block_on(relay.push(id(CREATOR), space.space, frames)).results;
        assert!(pushed.iter().all(|p| matches!(p, Pushed::Accepted(_))));
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
    }
    let relay = Relay::new(dir.store(), config());
    let change = entry(&mut space, persons, &[&head]);
    let frames = vec![change.bytes().to_vec()];
    let resident_before = resident();
    let before = ALLOCATED.reset();
    let start = Instant::now();
    let pushed = block_on(relay.push(id(WRITER), space.space, frames)).results;
    assert!(matches!(pushed[..], [Pushed::Accepted(_)]), "{pushed:?}");
    block_on(relay.put_person_keys(id(WRITER), space.space, vec![record(&space, persons)]))
        .expect("the relay stores the record");
    let millis = start.elapsed().as_secs_f64() * 1e3;
    let peak = ALLOCATED.peak.load(Ordering::Relaxed) - before;
    let held = ALLOCATED.now.load(Ordering::Relaxed).saturating_sub(before);
    let resident = resident().saturating_sub(resident_before);
    Taken {
        held,
        peak,
        resident,
        millis,
    }
}

#[test]
#[ignore = "a measurement: run it with --release --ignored --nocapture"]
fn what_a_remembered_space_takes_in_the_relay() {
    let sizes: Vec<u32> = match std::env::var("NCDB_PERSONS") {
        Ok(sizes) => sizes
            .split(',')
            .map(|n| n.parse().expect("a number"))
            .collect(),
        Err(_) => vec![100, 1_000, 5_000, 10_000],
    };
    println!(
        "persons | held (MiB) | held per person (KiB) | peak of the request (MiB) | resident growth (MiB) | cold request (ms)"
    );
    for persons in sizes {
        let taken = measure(persons);
        println!(
            "{persons:7} | {:10.2} | {:21.2} | {:25.2} | {:21.2} | {:.0}",
            taken.held as f64 / (1 << 20) as f64,
            taken.held as f64 / 1024.0 / f64::from(persons),
            taken.peak as f64 / (1 << 20) as f64,
            taken.resident as f64 / (1 << 20) as f64,
            taken.millis,
        );
    }
}
