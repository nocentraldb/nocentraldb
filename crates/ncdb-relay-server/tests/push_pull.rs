//! RFC 0007, "Push" and "Pull", over a real socket: every frame goes
//! through the relay core and gets its result; a pull reads from a cursor,
//! in pages that each fit one message.

mod support;

use ncdb_core::cbor::Value;
use ncdb_core::change::Operation;
use ncdb_core::ids::{ObjectId, SpaceId};
use ncdb_core::membership::Role;
use support::space::Space;
use support::*;

fn pull(req: u64, space: SpaceId, epoch: &[u8], after: u64, limit: u64) -> Value {
    message(
        "pull",
        Some(req),
        &[
            (3, Value::Bytes(space.0.to_vec())),
            (4, Value::Bytes(epoch.to_vec())),
            (5, Value::Unsigned(after)),
            (6, Value::Unsigned(limit)),
        ],
    )
}

/// The positions and frames of an `entries` message.
fn entries(message: &Value) -> Vec<(u64, Vec<u8>)> {
    array(field(message, 4))
        .iter()
        .map(|entry| {
            assert_eq!(keys(entry), [1, 2, 3]);
            (uint(field(entry, 1)), bytes(field(entry, 3)).to_vec())
        })
        .collect()
}

/// A sealed change by member 0 whose frame is about `size` bytes.
fn large(space: &mut Space, size: usize) -> ncdb_core::change::Change {
    let value = Value::Bytes(vec![0x42; size]);
    let op = Operation::new(
        ObjectId::random(),
        "notes".into(),
        Some("body".into()),
        1,
        Some(value),
    );
    space.sealed(0, vec![op])
}

#[tokio::test(flavor = "multi_thread")]
async fn push_then_pull() {
    let (addr, _dir) = server().await;
    let (mut writer, welcome) = Client::member(addr, 0).await;
    let mut space = Space::new(0);
    let genesis = space.genesis.clone();
    let add = space.add(0, 1, Role::Reader, &[&genesis]);
    let one = space.sealed(0, Vec::new());
    let results = writer.push(1, space.space, &[&genesis, &add, &one]).await;
    assert_eq!(results, [accepted(1), accepted(2), accepted(3)]);

    let (mut reader, _) = Client::member(addr, 1).await;
    let page = reader
        .ask(&pull(4, space.space, &welcome.epoch, 1, 10))
        .await;
    assert_eq!(kind(&page), "entries");
    assert_eq!(keys(&page), [1, 2, 3, 4, 5]);
    assert_eq!(uint(field(&page, 2)), 4);
    assert_eq!(bytes(field(&page, 3)), space.space.0);
    assert_eq!(uint(field(&page, 5)), 3);
    let expected = vec![(2, add.bytes().to_vec()), (3, one.bytes().to_vec())];
    assert_eq!(entries(&page), expected);
    let accepted_at = uint(field(&array(field(&page, 4))[0], 2));
    assert!(accepted_at > 1_700_000_000_000);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_duplicate_push_returns_its_position() {
    let (addr, _dir) = server().await;
    let (mut client, _) = Client::member(addr, 0).await;
    let mut space = Space::new(0);
    let genesis = space.genesis.clone();
    let one = space.sealed(0, Vec::new());
    assert_eq!(
        client.push(1, space.space, &[&genesis, &one]).await,
        [accepted(1), accepted(2)]
    );
    let two = space.sealed(0, Vec::new());
    let results = client.push(2, space.space, &[&one, &two, &genesis]).await;
    assert_eq!(results, [duplicate(2), accepted(3), duplicate(1)]);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_non_member_is_refused() {
    let (addr, _dir) = server().await;
    let (mut creator, welcome) = Client::member(addr, 0).await;
    let mut space = Space::new(0);
    let genesis = space.genesis.clone();
    assert_eq!(
        creator.push(1, space.space, &[&genesis]).await,
        [accepted(1)]
    );

    let (mut stranger, _) = Client::member(addr, 5).await;
    let one = space.sealed(0, Vec::new());
    assert_eq!(
        stranger.push(2, space.space, &[&one]).await,
        [rejected("not_member")]
    );
    let page = stranger
        .ask(&pull(3, space.space, &welcome.epoch, 0, 10))
        .await;
    assert_eq!(page, error(Some(3), "not_member"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_refused_frame_does_not_stop_the_frames_after_it() {
    let (addr, _dir) = server().await;
    let (mut client, _) = Client::member(addr, 0).await;
    let mut space = Space::new(0);
    let genesis = space.genesis.clone();
    let one = space.sealed(0, Vec::new());
    let frames = Value::Array(vec![
        Value::Bytes(genesis.bytes().to_vec()),
        Value::Bytes(vec![0xff, 0x00]),
        Value::Bytes(one.bytes().to_vec()),
    ]);
    let push = message(
        "push",
        Some(1),
        &[(3, Value::Bytes(space.space.0.to_vec())), (4, frames)],
    );
    let pushed = client.ask(&push).await;
    let results = array(field(&pushed, 3));
    assert_eq!(results, [accepted(1), rejected("malformed"), accepted(2)]);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_pull_errs_on_another_epoch_and_on_an_unknown_space() {
    let (addr, _dir) = server().await;
    let (mut client, welcome) = Client::member(addr, 0).await;
    let space = Space::new(0);
    let page = client
        .ask(&pull(1, space.space, &welcome.epoch, 0, 10))
        .await;
    assert_eq!(page, error(Some(1), "unknown_space"));
    assert_eq!(
        client.push(2, space.space, &[&space.genesis]).await,
        [accepted(1)]
    );
    let page = client.ask(&pull(3, space.space, &[0; 16], 0, 10)).await;
    assert_eq!(page, error(Some(3), "epoch_changed"));
    let page = client
        .ask(&pull(4, space.space, &welcome.epoch, 2, 10))
        .await;
    assert_eq!(page, error(Some(4), "bad_cursor"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_pull_page_fits_one_message() {
    let (addr, _dir) = server().await;
    let (mut client, welcome) = Client::member(addr, 0).await;
    let mut space = Space::new(0);
    let genesis = space.genesis.clone();
    let big: Vec<_> = (0..3).map(|_| large(&mut space, 50_000)).collect();
    let mut changes = vec![&genesis];
    changes.extend(big.iter());
    for (n, change) in changes.iter().enumerate() {
        let results = client.push(n as u64, space.space, &[change]).await;
        assert_eq!(results, [accepted(n as u64 + 1)]);
    }
    let mut after = 0;
    let mut pages = 0;
    while after < 4 {
        let page = client
            .ask(&pull(9, space.space, &welcome.epoch, after, 100))
            .await;
        assert_eq!(uint(field(&page, 5)), 4);
        let entries = entries(&page);
        assert!(!entries.is_empty());
        for (position, frame) in &entries {
            assert_eq!(*frame, changes[*position as usize - 1].bytes());
        }
        after = entries.last().unwrap().0;
        pages += 1;
    }
    assert!(pages >= 2, "{pages} pages");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_push_of_more_than_32_frames_is_malformed() {
    let (addr, _dir) = server().await;
    let (mut client, _) = Client::member(addr, 0).await;
    let space = Space::new(0);
    let frames = vec![Value::Bytes(space.genesis.bytes().to_vec()); 33];
    let push = message(
        "push",
        Some(5),
        &[
            (3, Value::Bytes(space.space.0.to_vec())),
            (4, Value::Array(frames)),
        ],
    );
    assert_eq!(client.ask(&push).await, error(Some(5), "malformed"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_frame_larger_than_the_server_carries_is_too_large() {
    let (addr, _dir) = server().await;
    let (mut client, _) = Client::member(addr, 0).await;
    let mut space = Space::new(0);
    let genesis = space.genesis.clone();
    let big = large(&mut space, ncdb_relay_server::MAX_FRAME);
    assert!(big.bytes().len() > ncdb_relay_server::MAX_FRAME);
    assert!(big.bytes().len() < ncdb_relay_server::MAX_MESSAGE - 200);
    let results = client.push(1, space.space, &[&genesis, &big]).await;
    assert_eq!(results, [accepted(1), rejected("too_large")]);
}
