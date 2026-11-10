//! A member that reads slowly, or not at all (RFC 0007, "Subscribe" and
//! "Liveness"). Each connection's outgoing queue is bounded: a
//! subscription reads the log only as fast as its member takes the
//! entries, so a slow reader gets every entry, in order. A member that
//! takes nothing for longer than the server's write timeout is closed, as a
//! relay may close any connection at any time; it reconnects and
//! resubscribes from its cursor, and misses nothing.

mod support;

use std::time::Duration;

use ncdb_core::cbor::Value;
use ncdb_core::change::{Change, Operation};
use ncdb_core::ids::{ObjectId, SpaceId};
use ncdb_core::membership::Role;
use support::space::Space;
use support::*;

/// How long the test server waits for progress in writing to a member.
const WRITE_TIMEOUT: Duration = Duration::from_millis(500);

/// The size of each change's payload: about 100 KB, one entry per message.
const SIZE: usize = 100_000;

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

/// The positions in an `entries` message, and its head.
fn positions(message: &Value) -> (Vec<u64>, u64) {
    assert_eq!(kind(message), "entries", "{message:?}");
    let positions = array(field(message, 4))
        .iter()
        .map(|e| uint(field(e, 1)))
        .collect();
    (positions, uint(field(message, 5)))
}

/// A sealed change by member 0 with a payload of about `SIZE` bytes.
fn large(space: &mut Space) -> Change {
    let op = Operation::new(
        ObjectId::random(),
        "notes".into(),
        Some("body".into()),
        1,
        Some(Value::Bytes(vec![0x42; SIZE])),
    );
    space.sealed(0, vec![op])
}

/// A space of member 0 with member 1 as a reader, at head 2, and member
/// 1 subscribed to it from its head.
async fn subscribed(addr: std::net::SocketAddr) -> (Client, Client, Space, Vec<u8>) {
    let (mut zero, welcome) = Client::member(addr, 0).await;
    let mut space = Space::new(0);
    let genesis = space.genesis.clone();
    let add = space.add(0, 1, Role::Reader, &[&genesis]);
    let results = zero.push(1, space.space, &[&genesis, &add]).await;
    assert_eq!(results, [accepted(1), accepted(2)]);
    let (mut one, _) = Client::member(addr, 1).await;
    let catch_up = one.ask(&subscribe(2, space.space, &welcome.epoch, 2)).await;
    assert_eq!(positions(&catch_up), (Vec::new(), 2));
    (zero, one, space, welcome.epoch)
}

/// Member 0 pushes `count` large changes, one push each.
async fn push_large(zero: &mut Client, space: &mut Space, count: u64) {
    for req in 0..count {
        let change = large(space);
        let results = zero.push(100 + req, space.space, &[&change]).await;
        assert_eq!(results, [accepted(3 + req)]);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_slow_reader_gets_every_entry_in_order() {
    let (addr, _dir) = server_with_write_timeout(WRITE_TIMEOUT).await;
    let (mut zero, mut one, mut space, _) = subscribed(addr).await;
    push_large(&mut zero, &mut space, 60).await;

    let mut seen = Vec::new();
    while seen.len() < 60 {
        tokio::time::sleep(Duration::from_millis(10)).await;
        let (entries, head) = positions(&one.recv().await);
        seen.extend(entries);
        assert!(seen.last().is_some_and(|last| *last <= head));
    }
    assert_eq!(seen, (3..=62).collect::<Vec<_>>());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_member_that_stops_reading_is_closed_and_resumes_from_its_cursor() {
    let (addr, _dir) = server_with_write_timeout(WRITE_TIMEOUT).await;
    let (mut zero, mut one, mut space, epoch) = subscribed(addr).await;
    // About 30 MB: more than the server's queue and both sockets' buffers
    // hold, while member 1 reads nothing.
    let count = 300;
    let head = 2 + count;
    push_large(&mut zero, &mut space, count).await;
    tokio::time::sleep(WRITE_TIMEOUT * 4).await;

    let mut seen = Vec::new();
    for live in one.until_closed().await {
        seen.extend(positions(&live).0);
    }
    let cursor = seen.last().copied().unwrap_or(2);
    assert_eq!(seen, (3..=cursor).collect::<Vec<_>>(), "a dense prefix");
    assert!(
        cursor < head,
        "the server held back what member 1 did not take"
    );

    let (mut again, _) = Client::member(addr, 1).await;
    again.send(&subscribe(3, space.space, &epoch, cursor)).await;
    let mut rest = Vec::new();
    while rest.last().copied().unwrap_or(cursor) < head {
        rest.extend(positions(&again.recv().await).0);
    }
    assert_eq!(rest, (cursor + 1..=head).collect::<Vec<_>>());
}
