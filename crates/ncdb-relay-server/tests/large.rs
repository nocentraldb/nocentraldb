//! RFC 0007, "Large changes", over a real socket: a member asks for an
//! upload, puts the frame to the upload URL, and pushes a reference to it;
//! a pull hands out a download URL for an entry that does not fit into one
//! message. The URLs are served by the same listener (NOC-208): `PUT
//! /uploads/<token>` once, with the declared size, and `GET
//! /frames/<token>`.

mod support;

use ncdb_core::cbor::Value;
use ncdb_core::change::{Change, Operation};
use ncdb_core::ids::{ObjectId, SpaceId};
use support::http;
use support::space::Space;
use support::*;

/// The largest frame the relay accepts (RFC 0007, "Large changes").
const LARGEST: usize = 16 * 1024 * 1024;

/// A sealed change by member 0 whose frame is about `size` bytes.
fn large(space: &mut Space, size: usize) -> Change {
    let op = Operation::new(
        ObjectId::random(),
        "notes".into(),
        Some("body".into()),
        1,
        Some(Value::Bytes(vec![0x42; size])),
    );
    space.sealed(0, vec![op])
}

fn upload(req: u64, space: SpaceId, size: usize) -> Value {
    message(
        "upload",
        Some(req),
        &[
            (3, Value::Bytes(space.0.to_vec())),
            (4, Value::Unsigned(size as u64)),
        ],
    )
}

/// Asks for an upload of `size` bytes; its id and URL.
async fn begin(client: &mut Client, req: u64, space: SpaceId, size: usize) -> (Vec<u8>, String) {
    let answer = client.ask(&upload(req, space, size)).await;
    assert_eq!(kind(&answer), "upload", "{answer:?}");
    assert_eq!(uint(field(&answer, 2)), req);
    assert_eq!(keys(&answer), [1, 2, 3, 4]);
    let url = text(field(&answer, 4)).to_owned();
    assert!(url.contains("/uploads/"), "{url}");
    (bytes(field(&answer, 3)).to_vec(), url)
}

/// Pushes a reference to the upload `id`; its result.
async fn push_upload(client: &mut Client, req: u64, space: SpaceId, id: &[u8]) -> Value {
    let reference =
        Value::Map([(ncdb_core::cbor::Key::Unsigned(1), Value::Bytes(id.to_vec()))].into());
    let push = message(
        "push",
        Some(req),
        &[
            (3, Value::Bytes(space.0.to_vec())),
            (4, Value::Array(vec![reference])),
        ],
    );
    let pushed = client.ask(&push).await;
    assert_eq!(kind(&pushed), "pushed", "{pushed:?}");
    array(field(&pushed, 3))[0].clone()
}

/// A member with a space whose genesis is appended, and the relay epoch.
struct Held {
    client: Client,
    space: Space,
    addr: std::net::SocketAddr,
    epoch: Vec<u8>,
    _dir: TempDir,
}

async fn member_with_space() -> Held {
    let (addr, dir) = server().await;
    let (mut client, welcome) = Client::member(addr, 0).await;
    let space = Space::new(0);
    let genesis = space.genesis.clone();
    assert_eq!(
        client.push(1, space.space, &[&genesis]).await,
        [accepted(1)]
    );
    Held {
        client,
        space,
        addr,
        epoch: welcome.epoch,
        _dir: dir,
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_large_change_goes_up_through_an_upload_and_comes_down_through_a_download() {
    let Held {
        mut client,
        mut space,
        epoch,
        ..
    } = member_with_space().await;
    let big = large(&mut space, 300 * 1024);
    assert!(big.bytes().len() > ncdb_relay_server::MAX_MESSAGE);

    let (id, url) = begin(&mut client, 2, space.space, big.bytes().len()).await;
    assert_eq!(http::put(&url, big.bytes()).await.status, 200);
    assert_eq!(
        push_upload(&mut client, 3, space.space, &id).await,
        accepted(2)
    );

    let pull = message(
        "pull",
        Some(4),
        &[
            (3, Value::Bytes(space.space.0.to_vec())),
            (4, Value::Bytes(epoch)),
            (5, Value::Unsigned(1)),
            (6, Value::Unsigned(10)),
        ],
    );
    let entries = client.ask(&pull).await;
    assert_eq!(kind(&entries), "entries", "{entries:?}");
    let entry = &array(field(&entries, 4))[0];
    assert_eq!(uint(field(entry, 1)), 2);
    let download = field(entry, 3);
    assert_eq!(keys(download), [1, 2]);
    assert_eq!(uint(field(download, 2)), big.bytes().len() as u64);
    let url = text(field(download, 1));
    assert!(url.contains("/frames/"), "{url}");
    let got = http::get(url).await;
    assert_eq!(got.status, 200);
    assert_eq!(got.body, big.bytes());
}

#[tokio::test(flavor = "multi_thread")]
async fn an_upload_larger_than_the_largest_frame_is_too_large() {
    let Held {
        mut client, space, ..
    } = member_with_space().await;
    let answer = client.ask(&upload(2, space.space, LARGEST + 1)).await;
    assert_eq!(answer, error(Some(2), "too_large"));
}

#[tokio::test(flavor = "multi_thread")]
async fn an_upload_url_takes_its_declared_size_and_only_once() {
    let Held {
        mut client,
        mut space,
        ..
    } = member_with_space().await;
    let big = large(&mut space, 200 * 1024);
    let frame = big.bytes();
    let (id, url) = begin(&mut client, 2, space.space, frame.len()).await;

    assert_eq!(http::put(&url, &frame[..frame.len() - 1]).await.status, 400);
    assert_eq!(http::put(&url, frame).await.status, 200);
    assert_eq!(http::put(&url, frame).await.status, 404);
    assert_eq!(
        push_upload(&mut client, 3, space.space, &id).await,
        accepted(2)
    );
    // The upload is gone once its frame was pushed.
    let again = push_upload(&mut client, 4, space.space, &id).await;
    assert_eq!(again, rejected("malformed"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_body_longer_than_the_largest_frame_is_refused_before_it_is_read() {
    let Held {
        mut client, space, ..
    } = member_with_space().await;
    let (_, url) = begin(&mut client, 2, space.space, 1024).await;
    let response = http::request_with_length("PUT", &url, LARGEST + 1, &[0; 16]).await;
    assert_eq!(response.status, 413);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_push_of_an_upload_nobody_filled_is_malformed() {
    let Held {
        mut client, space, ..
    } = member_with_space().await;
    let (id, _url) = begin(&mut client, 2, space.space, 1024).await;
    assert_eq!(
        push_upload(&mut client, 3, space.space, &id).await,
        rejected("malformed")
    );
    let unknown = push_upload(&mut client, 4, space.space, &[7; 16]).await;
    assert_eq!(unknown, rejected("malformed"));
}

#[tokio::test(flavor = "multi_thread")]
async fn only_a_current_member_gets_an_upload_and_unknown_paths_are_not_found() {
    let Held { space, addr, .. } = member_with_space().await;
    let (mut stranger, _) = Client::member(addr, 9).await;
    let answer = stranger.ask(&upload(2, space.space, 1024)).await;
    assert_eq!(answer, error(Some(2), "not_member"));
    let base = format!("http://{addr}");
    assert_eq!(
        http::get(&format!("{base}/frames/{}", "00".repeat(32)))
            .await
            .status,
        404
    );
    assert_eq!(http::get(&format!("{base}/elsewhere")).await.status, 404);
}
