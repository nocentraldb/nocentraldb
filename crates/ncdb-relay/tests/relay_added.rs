//! RFC 0007, "Subscribe" and decision R13: when an append makes a member
//! current in a space, the relay tells that member's open connections
//! (`added`). The core says who became current; the server delivers.

mod support;

use ncdb_core::membership::Role;
use ncdb_relay::Pushed;
use ncdb_relay::conformance::block_on;
use support::relay;
use support::space::{Space, id};

#[test]
fn the_genesis_makes_its_creator_current() {
    let (relay, _dir) = relay();
    let space = Space::new(0);
    let frames = vec![space.genesis.bytes().to_vec()];
    let push = block_on(relay.push(id(0), space.space, frames));
    assert_eq!(push.results, [Pushed::Accepted(1)]);
    assert_eq!(push.added, [id(0)]);
}

#[test]
fn a_membership_entry_that_adds_a_member_makes_it_current() {
    let (relay, _dir) = relay();
    let mut space = Space::new(0);
    let genesis = space.genesis.clone();
    let add_1 = space.add(0, 1, Role::Writer, &[&genesis]);
    let add_2 = space.add(0, 2, Role::Reader, &[&add_1]);
    let frames = [&genesis, &add_1, &add_2]
        .iter()
        .map(|c| c.bytes().to_vec())
        .collect();
    let push = block_on(relay.push(id(0), space.space, frames));
    assert_eq!(push.added, [id(0), id(1), id(2)]);
}

#[test]
fn a_change_that_leaves_every_member_as_it_was_adds_nobody() {
    let (relay, _dir) = relay();
    let mut space = Space::new(0);
    let genesis = space.genesis.clone();
    let add = space.add(0, 1, Role::Writer, &[&genesis]);
    let frames = vec![genesis.bytes().to_vec(), add.bytes().to_vec()];
    block_on(relay.push(id(0), space.space, frames));

    // A role change of a current member, a key envelope, a sealed change
    // and a duplicate.
    let set_role = space.set_role(0, 1, Role::Reader, 0, &[&add]);
    let envelope = space.envelope(0, 1, &[&set_role]);
    let sealed = space.sealed(0, Vec::new());
    let frames = [&set_role, &envelope, &sealed, &add]
        .iter()
        .map(|c| c.bytes().to_vec())
        .collect();
    let push = block_on(relay.push(id(0), space.space, frames));
    assert_eq!(
        push.results,
        [
            Pushed::Accepted(3),
            Pushed::Accepted(4),
            Pushed::Accepted(5),
            Pushed::Duplicate(2)
        ]
    );
    assert_eq!(push.added, []);
}

#[test]
fn a_refused_change_adds_nobody() {
    let (relay, _dir) = relay();
    let mut space = Space::new(0);
    let genesis = space.genesis.clone();
    block_on(relay.push(id(0), space.space, vec![genesis.bytes().to_vec()]));
    // 1 is no member, so it may not upload, not even its own addition.
    let add = space.add(0, 1, Role::Writer, &[&genesis]);
    let push = block_on(relay.push(id(1), space.space, vec![add.bytes().to_vec()]));
    assert!(matches!(push.results[..], [Pushed::Rejected(_)]));
    assert_eq!(push.added, []);
}

#[test]
fn a_member_added_again_after_its_removal_becomes_current_again() {
    let (relay, _dir) = relay();
    let mut space = Space::new(0);
    let genesis = space.genesis.clone();
    let first = space.first_generation;
    let add = space.add(0, 1, Role::Writer, &[&genesis]);
    let remove = space.remove(0, 1, 0, &[&add]);
    let (rotation, next) = space.rotate(0, &[first], &[&remove]);
    let frames = [&genesis, &add, &remove, &rotation]
        .iter()
        .map(|c| c.bytes().to_vec())
        .collect();
    let push = block_on(relay.push(id(0), space.space, frames));
    assert_eq!(push.results.len(), 4);
    assert!(
        push.results
            .iter()
            .all(|r| matches!(r, Pushed::Accepted(_)))
    );

    // RFC 0008, M5: a removed member comes back with a new grant.
    let again = space.add_at(0, 1, Role::Writer, next, &[&rotation]);
    let push = block_on(relay.push(id(0), space.space, vec![again.bytes().to_vec()]));
    assert_eq!(push.results, [Pushed::Accepted(5)]);
    assert_eq!(push.added, [id(1)]);
}
