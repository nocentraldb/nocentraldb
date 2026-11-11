//! The relay core never loads the log's large frames for work that does not
//! need them (NOC-215): resolving a space's membership and answering `keys`
//! read its control changes only, and so does accepting a shred, which
//! deletes its record at once (RFC 0008, "A shred is final when
//! accepted").

mod support;

use ncdb_core::cbor::Value;
use ncdb_core::change::{Change, Operation};
use ncdb_core::ids::{ObjectId, SpaceId};
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
const SECOND: u8 = 3;
const MIB: usize = 1024 * 1024;

fn accepted(relay: &Relay<Store>, uploader: u8, change: &Change) {
    let space = change.header().space;
    let frames = vec![change.bytes().to_vec()];
    let pushed = block_on(relay.push(id(uploader), space, frames)).results;
    assert!(matches!(pushed[..], [Pushed::Accepted(_)]), "{pushed:?}");
}

/// A sealed change by `author` whose frame is a little over `size` bytes.
fn large(space: &mut Space, author: u8, size: usize) -> Change {
    let op = Operation::new(
        ObjectId::random(),
        "notes".into(),
        Some("body".into()),
        1,
        Some(Value::Bytes(vec![0x42; size])),
    );
    space.sealed(author, vec![op])
}

/// A space with a writer and a second admin, and 8 sealed changes of 1 MiB
/// by its creator.
fn with_large_frames() -> (Relay<Store>, TempDir, Space, Change) {
    let dir = TempDir::new();
    let relay = Relay::new(Counting::new(dir.store()), config());
    let mut space = Space::new(CREATOR);
    let genesis = space.genesis.clone();
    let writer = space.add(CREATOR, WRITER, Role::Writer, &[&genesis]);
    space.certify_by(SECOND, other_account(1));
    let second = space.add(CREATOR, SECOND, Role::Admin, &[&writer]);
    for change in [&genesis, &writer, &second] {
        accepted(&relay, CREATOR, change);
    }
    for _ in 0..8 {
        let change = large(&mut space, CREATOR, MIB);
        accepted(&relay, CREATOR, &change);
    }
    relay.store().take_bytes();
    (relay, dir, space, second)
}

#[test]
fn a_control_change_resolves_the_space_without_loading_a_sealed_frame() {
    let (relay, _dir, mut space, second) = with_large_frames();
    let reader = space.add(CREATOR, 4, Role::Reader, &[&second]);
    accepted(&relay, CREATOR, &reader);
    let (bytes, _) = relay.store().take_bytes();
    assert!(bytes < MIB, "the control change loaded {bytes} frame bytes");
}

#[test]
fn keys_load_no_sealed_frame() {
    let (relay, _dir, space, _) = with_large_frames();
    block_on(relay.keys(id(WRITER), space.space)).expect("the writer reads its keys");
    let (bytes, _) = relay.store().take_bytes();
    assert!(bytes < MIB, "keys loaded {bytes} frame bytes");
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

#[test]
fn accepting_a_shred_loads_no_large_frame() {
    let (relay, _dir, mut space, second) = with_large_frames();
    let entry = space.person_key(WRITER, 1, 0xc1, &[&second]);
    accepted(&relay, WRITER, &entry);
    let put = vec![record(space.space, space.first_generation)];
    block_on(relay.put_person_keys(id(WRITER), space.space, put)).expect("the record is stored");
    let shred = space.shred(WRITER, 1, &other_account(1), &[&entry]);
    relay.store().take_bytes();

    accepted(&relay, WRITER, &shred);

    let (bytes, _) = relay.store().take_bytes();
    assert!(
        bytes < MIB,
        "accepting the shred loaded {bytes} frame bytes"
    );
}
