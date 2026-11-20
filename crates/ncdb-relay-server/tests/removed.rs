//! RFC 0007, "Subscribe" and "The errors", over a real socket: the error
//! that ends a removed member's subscription or refuses its pull carries
//! the control change that removed it, and nothing else of the space
//! (Carsten, 2026-11-20, NOC-383, option A).

mod support;

use ncdb_core::cbor::Value;
use ncdb_core::change::Change;
use ncdb_core::ids::SpaceId;
use ncdb_core::membership::Role;
use support::space::Space;
use support::*;

fn cursor(kind: &str, req: u64, space: SpaceId, epoch: &[u8], after: u64) -> Value {
    let mut fields = vec![
        (3, Value::Bytes(space.0.to_vec())),
        (4, Value::Bytes(epoch.to_vec())),
        (5, Value::Unsigned(after)),
    ];
    if kind == "pull" {
        fields.push((6, Value::Unsigned(100)));
    }
    message(kind, Some(req), &fields)
}

/// A space where member 0 added member 1 as a reader and then removed it:
/// the space and the removal, at position 3.
async fn removed(zero: &mut Client) -> (Space, Change) {
    let mut space = Space::new(0);
    let genesis = space.genesis.clone();
    let add = space.add(0, 1, Role::Reader, &[&genesis]);
    let remove = space.remove(0, 1, 0, &[&add]);
    zero.push(1, space.space, &[&genesis, &add, &remove]).await;
    (space, remove)
}

#[tokio::test(flavor = "multi_thread")]
async fn a_subscription_ends_with_the_removal_and_then_nothing() {
    let (addr, _dir) = server().await;
    let (mut zero, welcome) = Client::member(addr, 0).await;
    let mut space = Space::new(0);
    let genesis = space.genesis.clone();
    let add = space.add(0, 1, Role::Reader, &[&genesis]);
    zero.push(1, space.space, &[&genesis, &add]).await;
    let (mut one, _) = Client::member(addr, 1).await;
    one.ask(&cursor("subscribe", 3, space.space, &welcome.epoch, 2))
        .await;

    let remove = space.remove(0, 1, 0, &[&add]);
    assert_eq!(zero.push(4, space.space, &[&remove]).await, [accepted(3)]);
    assert_removal(&one.recv().await, 3, "removed_member", 3, &remove);
    let change = space.sealed(0, Vec::new());
    assert_eq!(zero.push(5, space.space, &[&change]).await, [accepted(4)]);
    one.quiet(300).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_former_member_s_pull_and_subscribe_get_the_removal() {
    let (addr, _dir) = server().await;
    let (mut zero, welcome) = Client::member(addr, 0).await;
    let (space, remove) = removed(&mut zero).await;
    let (mut one, _) = Client::member(addr, 1).await;
    for (req, kind) in [(1, "pull"), (2, "subscribe")] {
        let answer = one
            .ask(&cursor(kind, req, space.space, &welcome.epoch, 2))
            .await;
        assert_removal(&answer, req, "not_member", 3, &remove);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_stranger_gets_not_member_alone() {
    let (addr, _dir) = server().await;
    let (mut zero, welcome) = Client::member(addr, 0).await;
    let (space, _) = removed(&mut zero).await;
    let (mut stranger, _) = Client::member(addr, 5).await;
    for (req, kind) in [(1, "pull"), (2, "subscribe")] {
        let answer = stranger
            .ask(&cursor(kind, req, space.space, &welcome.epoch, 0))
            .await;
        assert_eq!(answer, error(Some(req), "not_member"));
    }
}
