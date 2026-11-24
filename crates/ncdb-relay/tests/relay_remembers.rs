//! The relay remembers a space's control changes and their resolve from
//! one request to the next (NOC-427): a warm process reads only the
//! control changes appended since, and judges a new one without resolving
//! the space from scratch. What it answers stays a function of the log as
//! the store holds it now: a log replaced under it, as a restore does, is
//! read again from the start.

mod support;

use ncdb_core::change::Change;
use ncdb_core::membership::{self, Entry, PersonKeyEntry, Role};
use ncdb_relay::conformance::block_on;
use ncdb_relay::store::{PersonKeyId, PersonKeyRecord};
use ncdb_relay::{ErrorCode, Pushed, Relay};
use support::counting::Counting;
use support::space::{Space, id, key};
use support::swap::Swap;
use support::{TempDir, config, push};

const CREATOR: u8 = 0;
const WRITER: u8 = 1;
const OTHER: u8 = 2;

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

#[test]
fn a_warm_relay_reads_only_the_control_changes_it_has_not_read() {
    let dir = TempDir::new();
    let relay = Relay::new(Counting::new(dir.store()), config());
    let mut space = Space::new(CREATOR);
    let genesis = space.genesis.clone();
    let writer = space.add(CREATOR, WRITER, Role::Writer, &[&genesis]);
    let pushed = push_with(&relay, CREATOR, &[&genesis, &writer]);
    assert!(pushed.iter().all(|p| matches!(p, Pushed::Accepted(_))));
    let mut head = writer;
    for n in 0..20 {
        let change = entry(&mut space, n, &[&head]);
        let pushed = push_with(&relay, WRITER, &[&change]);
        assert!(matches!(pushed[..], [Pushed::Accepted(_)]), "{pushed:?}");
        block_on(relay.put_person_keys(id(WRITER), space.space, vec![record(&space, n)]))
            .expect("the relay stores the record");
        head = change;
    }
    let change = entry(&mut space, 20, &[&head]);
    let frame = change.bytes().len();
    relay.store().take_bytes();
    let pushed = push_with(&relay, WRITER, &[&change]);
    assert!(matches!(pushed[..], [Pushed::Accepted(_)]), "{pushed:?}");
    block_on(relay.put_person_keys(id(WRITER), space.space, vec![record(&space, 20)]))
        .expect("the relay stores the record");
    let (bytes, _) = relay.store().take_bytes();
    // The push reads the last entry it holds, to see that it is still
    // there; the put reads that entry again and the new one: three frames,
    // not the 22 control changes of the log twice over.
    assert!(
        bytes <= 3 * frame + 64,
        "the push and the put read {bytes} frame bytes, for a new frame of {frame}"
    );
}

#[test]
fn a_put_after_a_warm_push_does_not_read_the_pushed_change_again() {
    let dir = TempDir::new();
    let relay = Relay::new(Counting::new(dir.store()), config());
    let mut space = Space::new(CREATOR);
    let genesis = space.genesis.clone();
    let writer = space.add(CREATOR, WRITER, Role::Writer, &[&genesis]);
    let pushed = push_with(&relay, CREATOR, &[&genesis, &writer]);
    assert!(pushed.iter().all(|p| matches!(p, Pushed::Accepted(_))));
    let change = entry(&mut space, 0, &[&writer]);
    let frame = change.bytes().len();
    let pushed = push_with(&relay, WRITER, &[&change]);
    assert!(matches!(pushed[..], [Pushed::Accepted(_)]), "{pushed:?}");
    relay.store().take_bytes();
    block_on(relay.put_person_keys(id(WRITER), space.space, vec![record(&space, 0)]))
        .expect("the relay stores the record");
    let (bytes, _) = relay.store().take_bytes();
    // The push kept the change it appended, so the put reads only that
    // entry, to see that it is still there, and resolves nothing.
    assert!(
        bytes <= frame + 64,
        "the put read {bytes} frame bytes, for a pushed frame of {frame}"
    );
}

/// The person-key entry of the `n`th of many persons, by the writer, after
/// `head`: person ids here are numbers, so a space can hold more than 255.
fn many(space: &mut Space, n: u32, head: &Change) -> Change {
    let mut person = [0x60; 16];
    person[..4].copy_from_slice(&n.to_be_bytes());
    let entry = PersonKeyEntry {
        person_key: person,
        commitment: [0xc0; 32],
        generation: space.first_generation,
        record_hash: membership::record_hash(&n.to_be_bytes()),
    };
    space.write(WRITER, Entry::PersonKey(entry), &[head])
}

#[test]
fn a_warm_relay_remembers_a_space_above_the_square_limit_of_1200_control_changes() {
    let dir = TempDir::new();
    let relay = Relay::new(Counting::new(dir.store()), config());
    let mut space = Space::new(CREATOR);
    let genesis = space.genesis.clone();
    let writer = space.add(CREATOR, WRITER, Role::Writer, &[&genesis]);
    let pushed = push_with(&relay, CREATOR, &[&genesis, &writer]);
    assert!(pushed.iter().all(|p| matches!(p, Pushed::Accepted(_))));
    // 1,300 control changes in all, above the 1,200 the memory held while
    // it weighed a space by the square of its control changes.
    let mut head = writer;
    let mut changes = Vec::new();
    for n in 0..1_298 {
        let change = many(&mut space, n, &head);
        changes.push(change.clone());
        head = change;
    }
    let pushed = push_with(&relay, WRITER, &changes.iter().collect::<Vec<_>>());
    assert!(pushed.iter().all(|p| matches!(p, Pushed::Accepted(_))));
    let change = many(&mut space, 1_298, &head);
    let frame = change.bytes().len();
    relay.store().take_bytes();
    let pushed = push_with(&relay, WRITER, &[&change]);
    assert!(matches!(pushed[..], [Pushed::Accepted(_)]), "{pushed:?}");
    let (bytes, _) = relay.store().take_bytes();
    // The push reads only the last entry it holds, to see that it is still
    // there, not the 1,300 control changes of the log.
    assert!(
        bytes <= 2 * frame + 64,
        "the push read {bytes} frame bytes, for a new frame of {frame}"
    );
}

/// `uploader` pushes these changes to a relay over any store.
fn push_with<S: ncdb_relay::store::RelayStore>(
    relay: &Relay<S>,
    uploader: u8,
    changes: &[&Change],
) -> Vec<Pushed> {
    let space = changes[0].header().space;
    let frames = changes.iter().map(|c| c.bytes().to_vec()).collect();
    block_on(relay.push(id(uploader), space, frames)).results
}

#[test]
fn a_relay_whose_log_was_replaced_judges_by_the_log_it_holds_now() {
    let (first, second) = (TempDir::new(), TempDir::new());
    let mut space = Space::new(CREATOR);
    let genesis = space.genesis.clone();
    let writer = space.add(CREATOR, WRITER, Role::Writer, &[&genesis]);
    let other = space.add(CREATOR, OTHER, Role::Writer, &[&genesis]);
    // The second log holds the same space, with another change at the
    // position where the first holds the writer's add.
    let restored = Relay::new(second.store(), config());
    let pushed = push(&restored, CREATOR, space.space, &[&genesis, &other]);
    assert!(pushed.iter().all(|p| matches!(p, Pushed::Accepted(_))));
    drop(restored);
    let relay = Relay::new(Swap::new(first.store(), second.store()), config());
    let pushed = push_with(&relay, CREATOR, &[&genesis, &writer]);
    assert!(pushed.iter().all(|p| matches!(p, Pushed::Accepted(_))));
    relay.store().swap();
    // The writer is no member in the log the relay holds now, so its
    // person-key entry, pushed by the creator, has an author who is no
    // member there.
    let change = entry(&mut space, 1, &[&writer]);
    let pushed = push_with(&relay, CREATOR, &[&change]);
    assert_eq!(
        pushed,
        vec![Pushed::Rejected(ErrorCode::AuthorNotMember)],
        "the relay judges by the log it holds now"
    );
}
