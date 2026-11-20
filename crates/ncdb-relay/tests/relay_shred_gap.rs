//! RFC 0008, "A shred is final when accepted" (NOC-391): the relay gives a
//! shred its verdict only once it holds the shred's whole causal past, and
//! deletes the person key's record only then. A shred pushed over a gap in
//! its causal past is not accepted, so the record stays.
//!
//! Writer shreds person 1, signed off by Second's account. Creator had
//! demoted Second to a reader before, and the shred depends on that
//! demotion through Creator's add of a reader, which saw it: in the shred's
//! whole causal past, Second's account holds no admin, so the sign-off is
//! not valid. A relay that holds that add but not yet the demotion must not
//! accept the shred and delete the record.

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
/// An admin of a second account, who signs off the shred.
const SECOND: u8 = 3;
/// A reader the creator adds after the demotion.
const READER: u8 = 4;

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

fn stored(relay: &Relay<SqliteStore>, space: SpaceId) -> Vec<[u8; 16]> {
    block_on(relay.person_keys(id(CREATOR), space))
        .expect("the creator reads the records")
        .into_iter()
        .map(|r| r.id.0)
        .collect()
}

#[test]
fn a_shred_pushed_over_a_gap_in_its_causal_past_keeps_the_record() {
    let (relay, _dir) = relay();
    let mut space = Space::new(CREATOR);
    let genesis = space.genesis.clone();
    let writer = space.add(CREATOR, WRITER, Role::Writer, &[&genesis]);
    space.certify_by(SECOND, other_account(1));
    let second = space.add(CREATOR, SECOND, Role::Admin, &[&writer]);
    let one = space.person_key(WRITER, 1, 0xc1, &[&second]);
    let held: Vec<&Change> = vec![&genesis, &writer, &second, &one];
    let pushed = push(&relay, CREATOR, space.space, &held);
    assert!(pushed.iter().all(|p| matches!(p, Pushed::Accepted(_))));
    let records = vec![record(space.space, 1, space.first_generation)];
    block_on(relay.put_person_keys(id(WRITER), space.space, records)).expect("it is stored");
    let demote = space.set_role(
        CREATOR,
        SECOND,
        Role::Reader,
        space.sequence(SECOND),
        &[&one],
    );
    let reader = space.add(CREATOR, READER, Role::Reader, &[&demote, &one]);
    let shred = space.shred(WRITER, 1, &other_account(1), &[&reader]);
    let after_demotion = push_one(&relay, CREATOR, &reader);
    assert!(
        matches!(after_demotion, Pushed::Accepted(_)),
        "{after_demotion:?}"
    );

    let over_the_gap = push_one(&relay, WRITER, &shred);

    assert!(
        !matches!(over_the_gap, Pushed::Accepted(_)),
        "the relay accepted the shred without its whole causal past: {over_the_gap:?}"
    );
    assert_eq!(stored(&relay, space.space), [person(1)]);
    assert!(matches!(
        push_one(&relay, CREATOR, &demote),
        Pushed::Accepted(_)
    ));
    let whole = push_one(&relay, WRITER, &shred);
    assert!(
        !matches!(whole, Pushed::Accepted(_)),
        "the sign-off is not valid in the whole causal past: {whole:?}"
    );
    assert_eq!(stored(&relay, space.space), [person(1)]);
}
