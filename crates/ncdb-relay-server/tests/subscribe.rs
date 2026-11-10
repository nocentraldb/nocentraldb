//! RFC 0007, "Subscribe", over a real socket: catch-up pages as for a pull,
//! then every appended entry as it is appended, until the member
//! unsubscribes, the connection ends or the member is removed.

mod support;

use ncdb_core::cbor::Value;
use ncdb_core::ids::SpaceId;
use ncdb_core::membership::Role;
use support::space::Space;
use support::*;

fn subscribe(req: u64, space: SpaceId, epoch: &[u8], after: u64) -> Value {
    message(
        "subscribe",
        Some(req),
        &[
            (3, Value::Bytes(space.0.to_vec())),
            (4, Value::Bytes(epoch.to_vec())),
            (5, Value::Unsigned(after)),
        ],
    )
}

/// The request id, positions, frames and head of an `entries` message.
fn page(message: &Value) -> (Option<u64>, Vec<(u64, Vec<u8>)>, u64) {
    assert_eq!(kind(message), "entries", "{message:?}");
    let entries = array(field(message, 4))
        .iter()
        .map(|e| (uint(field(e, 1)), bytes(field(e, 3)).to_vec()))
        .collect();
    (get(message, 2).map(uint), entries, uint(field(message, 5)))
}

/// A space of member 0 with member 1 as a writer, pushed by member 0.
async fn shared_space(writer: &mut Client) -> Space {
    let mut space = Space::new(0);
    let genesis = space.genesis.clone();
    let add = space.add(0, 1, Role::Writer, &[&genesis]);
    let results = writer.push(1, space.space, &[&genesis, &add]).await;
    assert_eq!(results, [accepted(1), accepted(2)]);
    space
}

#[tokio::test(flavor = "multi_thread")]
async fn two_subscribers_of_one_space_see_a_push_live() {
    let (addr, _dir) = server().await;
    let (mut zero, welcome) = Client::member(addr, 0).await;
    let mut space = shared_space(&mut zero).await;
    let (mut one, _) = Client::member(addr, 1).await;

    let catch_up = zero
        .ask(&subscribe(5, space.space, &welcome.epoch, 2))
        .await;
    assert_eq!(page(&catch_up), (Some(5), Vec::new(), 2));
    let catch_up = one.ask(&subscribe(6, space.space, &welcome.epoch, 1)).await;
    let (req, entries, head) = page(&catch_up);
    assert_eq!((req, head), (Some(6), 2));
    assert_eq!(entries.iter().map(|e| e.0).collect::<Vec<_>>(), [2]);

    let change = space.sealed(1, Vec::new());
    let (mut pusher, _) = Client::member(addr, 1).await;
    assert_eq!(pusher.push(7, space.space, &[&change]).await, [accepted(3)]);
    for subscriber in [&mut zero, &mut one] {
        let live = subscriber.recv().await;
        assert_eq!(
            keys(&live),
            [1, 3, 4, 5],
            "a live message has no request id"
        );
        assert_eq!(bytes(field(&live, 3)), space.space.0);
        assert_eq!(page(&live), (None, vec![(3, change.bytes().to_vec())], 3));
    }
    pusher.quiet(200).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_subscription_catches_up_in_pages_until_the_head() {
    let (addr, _dir) = server().await;
    let (mut zero, welcome) = Client::member(addr, 0).await;
    let mut space = shared_space(&mut zero).await;
    let changes: Vec<_> = (0..20).map(|_| space.sealed(0, Vec::new())).collect();
    let refs: Vec<_> = changes.iter().collect();
    zero.push(2, space.space, &refs).await;

    let (mut one, _) = Client::member(addr, 1).await;
    one.send(&subscribe(3, space.space, &welcome.epoch, 0))
        .await;
    let mut seen = Vec::new();
    while seen.len() < 22 {
        let (req, entries, head) = page(&one.recv().await);
        assert_eq!((req, head), (Some(3), 22));
        seen.extend(entries.into_iter().map(|e| e.0));
    }
    assert_eq!(seen, (1..=22).collect::<Vec<_>>());
    one.quiet(200).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn an_unsubscribed_member_sees_no_more_entries() {
    let (addr, _dir) = server().await;
    let (mut zero, welcome) = Client::member(addr, 0).await;
    let mut space = shared_space(&mut zero).await;
    let (mut one, _) = Client::member(addr, 1).await;
    one.ask(&subscribe(3, space.space, &welcome.epoch, 2)).await;
    let unsubscribe = message(
        "unsubscribe",
        Some(4),
        &[(3, Value::Bytes(space.space.0.to_vec()))],
    );
    one.send(&unsubscribe).await;
    assert_eq!(
        one.ask(&message("ping", Some(5), &[])).await,
        message("pong", Some(5), &[])
    );

    let change = space.sealed(0, Vec::new());
    assert_eq!(zero.push(6, space.space, &[&change]).await, [accepted(3)]);
    one.quiet(300).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_removed_member_gets_removed_member_once_and_then_nothing() {
    let (addr, _dir) = server().await;
    let (mut zero, welcome) = Client::member(addr, 0).await;
    let mut space = Space::new(0);
    let genesis = space.genesis.clone();
    let add = space.add(0, 1, Role::Reader, &[&genesis]);
    zero.push(1, space.space, &[&genesis, &add]).await;
    let (mut one, _) = Client::member(addr, 1).await;
    one.ask(&subscribe(3, space.space, &welcome.epoch, 2)).await;

    let remove = space.remove(0, 1, 0, &[&add]);
    assert_eq!(zero.push(4, space.space, &[&remove]).await, [accepted(3)]);
    assert_eq!(one.recv().await, error(Some(3), "removed_member"));
    let change = space.sealed(0, Vec::new());
    assert_eq!(zero.push(5, space.space, &[&change]).await, [accepted(4)]);
    one.quiet(300).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_subscription_is_refused_as_a_pull_would_be() {
    let (addr, _dir) = server().await;
    let (mut zero, welcome) = Client::member(addr, 0).await;
    let space = shared_space(&mut zero).await;
    let (mut stranger, _) = Client::member(addr, 5).await;
    let answer = stranger
        .ask(&subscribe(1, space.space, &welcome.epoch, 0))
        .await;
    assert_eq!(answer, error(Some(1), "not_member"));
    let answer = zero.ask(&subscribe(2, space.space, &[9; 16], 0)).await;
    assert_eq!(answer, error(Some(2), "epoch_changed"));
}
