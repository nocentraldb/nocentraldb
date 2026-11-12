//! RFC 0008, "A provisional erasure in a space with one account" (Carsten,
//! 2026-11-11, NOC-230): in a space whose members all belong to one
//! account, the performer's own account signs off, and the shred is
//! provisional. The relay keeps the person key's record while the shred is
//! provisional or taken back, and deletes it once it accepts an erasure
//! decision that finalizes it. The 7 days the relay waits for a finalize
//! (`too_early`) are not checked here.

mod support;

use ncdb_core::change::Change;
use ncdb_core::ids::SpaceId;
use ncdb_core::membership::{self, Entry, ErasureDecision, Role};
use ncdb_relay::conformance::block_on;
use ncdb_relay::sqlite::SqliteStore;
use ncdb_relay::store::{PersonKeyId, PersonKeyRecord};
use ncdb_relay::{Pushed, Relay};
use support::space::{Space, account, id, key, person, wrapped};
use support::{push, push_one, relay};

/// The creator, an admin of the one account.
const CREATOR: u8 = 0;
/// A writer of the same account, who performs the erasure.
const WRITER: u8 = 1;

/// A one-account space on the relay: the creator and a writer of its
/// account, who created person 1's key, whose record is stored.
fn held() -> (Relay<SqliteStore>, support::TempDir, Space, Vec<Change>) {
    let (relay, dir) = relay();
    let mut space = Space::new(CREATOR);
    let genesis = space.genesis.clone();
    let writer = space.add(CREATOR, WRITER, Role::Writer, &[&genesis]);
    let one = space.person_key(WRITER, 1, 0xc1, &[&writer]);
    let held = vec![genesis, writer, one];
    let refs: Vec<&Change> = held.iter().collect();
    let pushed = push(&relay, CREATOR, space.space, &refs);
    assert!(pushed.iter().all(|p| matches!(p, Pushed::Accepted(_))));
    let records = vec![record(space.space, space.first_generation)];
    block_on(relay.put_person_keys(id(WRITER), space.space, records)).expect("it is stored");
    (relay, dir, space, held)
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

fn stored(relay: &Relay<SqliteStore>, space: SpaceId) -> Vec<[u8; 16]> {
    block_on(relay.person_keys(id(CREATOR), space))
        .expect("the creator reads the records")
        .into_iter()
        .map(|r| r.id.0)
        .collect()
}

fn accepted(relay: &Relay<SqliteStore>, uploader: u8, change: &Change) {
    let pushed = push_one(relay, uploader, change);
    assert!(matches!(pushed, Pushed::Accepted(_)), "{pushed:?}");
}

fn decide(space: &mut Space, shred: &Change, finalize: bool) -> Change {
    let entry = Entry::ErasureDecision(ErasureDecision {
        shred: shred.id(),
        finalize,
    });
    space.write(CREATOR, entry, &[shred])
}

#[test]
fn a_provisional_shred_keeps_the_record_until_a_finalize_is_accepted() {
    let (relay, _dir, mut space, held) = held();
    let shred = space.shred(WRITER, 1, &account(), &[&held[2]]);

    accepted(&relay, WRITER, &shred);
    assert_eq!(stored(&relay, space.space), [person(1)], "provisional");

    let finalize = decide(&mut space, &shred, true);
    accepted(&relay, CREATOR, &finalize);
    assert!(stored(&relay, space.space).is_empty(), "finalized");
}

#[test]
fn a_take_back_keeps_the_record() {
    let (relay, _dir, mut space, held) = held();
    let shred = space.shred(WRITER, 1, &account(), &[&held[2]]);
    accepted(&relay, WRITER, &shred);

    let take_back = decide(&mut space, &shred, false);
    accepted(&relay, CREATOR, &take_back);

    assert_eq!(stored(&relay, space.space), [person(1)]);
}
