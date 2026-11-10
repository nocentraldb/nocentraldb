//! RFC 0007, "Subscribe" and decision R13, over a real socket: when the
//! relay accepts a control change that makes a member current in a space,
//! it sends `added {space}` to each of that member's open connections,
//! without a request id. That includes the connection whose own push made
//! its member current, the creator's with a genesis: it gets `added` right
//! after `pushed`, which the support's `push` reads and checks.

mod support;

use ncdb_core::membership::Role;
use support::space::Space;
use support::*;

/// The `added` message's keys and space.
fn added(message: &ncdb_core::cbor::Value) -> Vec<u8> {
    assert_eq!(kind(message), "added", "{message:?}");
    assert_eq!(keys(message), [1, 3], "no request id, the space at key 3");
    bytes(field(message, 3)).to_vec()
}

#[tokio::test(flavor = "multi_thread")]
async fn every_connection_of_a_member_learns_that_it_was_added() {
    let (addr, _dir) = server().await;
    let (mut first, welcome) = Client::member(addr, 1).await;
    assert!(welcome.spaces.is_empty());
    let (mut second, _) = Client::member(addr, 1).await;
    let (mut stranger, _) = Client::member(addr, 2).await;
    let (mut creator, _) = Client::member(addr, 0).await;

    let mut space = Space::new(0);
    let genesis = space.genesis.clone();
    let add = space.add(0, 1, Role::Writer, &[&genesis]);
    let results = creator.push(1, space.space, &[&genesis, &add]).await;
    assert_eq!(results, [accepted(1), accepted(2)]);

    for connection in [&mut first, &mut second] {
        assert_eq!(added(&connection.recv().await), space.space.0);
    }
    stranger.quiet(200).await;
    // The creator's `added` for its genesis was read by `push`; the add of
    // member 1 sends it nothing.
    creator.quiet(200).await;

    // The member pulls the space, as for a space named in `welcome`.
    let pull = message(
        "pull",
        Some(2),
        &[
            (3, ncdb_core::cbor::Value::Bytes(space.space.0.to_vec())),
            (4, ncdb_core::cbor::Value::Bytes(welcome.epoch.clone())),
            (5, ncdb_core::cbor::Value::Unsigned(0)),
            (6, ncdb_core::cbor::Value::Unsigned(10)),
        ],
    );
    let page = first.ask(&pull).await;
    assert_eq!(kind(&page), "entries", "{page:?}");
    assert_eq!(array(field(&page, 4)).len(), 2);
}

#[tokio::test(flavor = "multi_thread")]
async fn every_connection_of_the_creator_learns_of_its_new_space() {
    let (addr, _dir) = server().await;
    let (mut other, _) = Client::member(addr, 0).await;
    let (mut creator, _) = Client::member(addr, 0).await;
    let space = Space::new(0);
    let results = creator.push_only(1, space.space, &[&space.genesis]).await;
    assert_eq!(results, [accepted(1)]);
    assert_eq!(added(&creator.recv().await), space.space.0);
    assert_eq!(added(&other.recv().await), space.space.0);
    creator.quiet(200).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_change_that_adds_nobody_sends_no_added() {
    let (addr, _dir) = server().await;
    let (mut creator, _) = Client::member(addr, 0).await;
    let mut space = Space::new(0);
    let genesis = space.genesis.clone();
    let add = space.add(0, 1, Role::Writer, &[&genesis]);
    creator.push(1, space.space, &[&genesis, &add]).await;

    let (mut member, welcome) = Client::member(addr, 1).await;
    assert_eq!(welcome.spaces, [space.space.0.to_vec()]);
    let set_role = space.set_role(0, 1, Role::Reader, 0, &[&add]);
    let sealed = space.sealed(0, Vec::new());
    let results = creator.push(2, space.space, &[&set_role, &sealed]).await;
    assert_eq!(results, [accepted(3), accepted(4)]);
    member.quiet(200).await;
}
