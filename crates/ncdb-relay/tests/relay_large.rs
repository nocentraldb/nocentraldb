//! RFC 0007, "Large changes": the core hands out an upload's id and token,
//! takes its frame once while its URL is valid, checks the frame like any
//! pushed frame, and serves an entry's frame through a short-lived token.

mod support;

use std::cell::Cell;

use ncdb_relay::conformance::block_on;
use ncdb_relay::store::Token;
use ncdb_relay::{
    DOWNLOAD_URL_VALID_FOR, ErrorCode, Filled, PushFrame, Pushed, Relay, UPLOAD_URL_VALID_FOR,
};
use support::space::{Space, id};
use support::{TempDir, config, push_one};

thread_local! {
    /// The clock of these tests, which they move. Each test runs on its own
    /// thread and polls the relay's futures on it, so the tests do not
    /// share it.
    static NOW: Cell<u64> = const { Cell::new(1_700_000_000_000) };
}

fn relay() -> (Relay<ncdb_relay::sqlite::SqliteStore>, TempDir) {
    let dir = TempDir::new();
    let mut config = config();
    config.clock = || NOW.get();
    (Relay::new(dir.store(), config), dir)
}

fn later(ms: u64) {
    NOW.set(NOW.get() + ms);
}

#[test]
fn an_uploaded_frame_is_checked_and_appended_and_served_until_its_token_expires() {
    let (relay, _dir) = relay();
    let space = Space::new(0);
    let frame = space.genesis.bytes().to_vec();
    // A space is begun with its genesis inline; an upload needs a space.
    assert_eq!(
        block_on(relay.begin_upload(id(0), space.space, 10)),
        Err(ErrorCode::UnknownSpace)
    );
    assert_eq!(push_one(&relay, 0, &space.genesis), Pushed::Accepted(1));
    let (upload, token) = block_on(relay.begin_upload(id(0), space.space, 3)).unwrap();
    assert_eq!(block_on(relay.upload_size(token)).unwrap(), Some(3));
    assert_eq!(
        block_on(relay.fill_upload(token, vec![1, 2])).unwrap(),
        Filled::WrongSize
    );
    assert_eq!(
        block_on(relay.fill_upload(token, vec![1, 2, 3])).unwrap(),
        Filled::Stored
    );
    assert_eq!(
        block_on(relay.fill_upload(token, vec![1, 2, 3])).unwrap(),
        Filled::NotFound
    );
    // Its frame is checked like any other: three bytes are no change.
    let pushed = block_on(relay.push_frames(id(0), space.space, vec![PushFrame::Upload(upload)]));
    assert_eq!(pushed.results, [Pushed::Rejected(ErrorCode::Malformed)]);

    let download = block_on(relay.begin_download(space.space, 1)).unwrap();
    assert_eq!(block_on(relay.download(download)).unwrap(), Some(frame));
    assert_eq!(block_on(relay.download(Token([0; 32]))).unwrap(), None);
    later(DOWNLOAD_URL_VALID_FOR);
    assert_eq!(block_on(relay.download(download)).unwrap(), None);
}

#[test]
fn an_upload_url_takes_no_frame_once_it_expired() {
    let (relay, _dir) = relay();
    let space = Space::new(0);
    push_one(&relay, 0, &space.genesis);
    let (_, token) = block_on(relay.begin_upload(id(0), space.space, 1)).unwrap();
    later(UPLOAD_URL_VALID_FOR);
    assert_eq!(block_on(relay.upload_size(token)).unwrap(), None);
    assert_eq!(
        block_on(relay.fill_upload(token, vec![1])).unwrap(),
        Filled::NotFound
    );
}

#[test]
fn only_a_current_member_begins_an_upload_and_never_above_the_largest_frame() {
    let (relay, _dir) = relay();
    let space = Space::new(0);
    push_one(&relay, 0, &space.genesis);
    assert_eq!(
        block_on(relay.begin_upload(id(1), space.space, 10)),
        Err(ErrorCode::NotMember)
    );
    let largest = ncdb_relay::MAX_FRAME as u64;
    assert!(block_on(relay.begin_upload(id(0), space.space, largest)).is_ok());
    assert_eq!(
        block_on(relay.begin_upload(id(0), space.space, largest + 1)),
        Err(ErrorCode::TooLarge)
    );
}

#[test]
fn an_upload_is_pushed_only_by_its_member() {
    let (relay, _dir) = relay();
    let mut space = Space::new(0);
    let genesis = space.genesis.clone();
    let add = space.add(0, 1, ncdb_core::membership::Role::Writer, &[&genesis]);
    push_one(&relay, 0, &genesis);
    push_one(&relay, 0, &add);
    let frame = add.bytes().to_vec();
    let (upload, token) =
        block_on(relay.begin_upload(id(0), space.space, frame.len() as u64)).unwrap();
    block_on(relay.fill_upload(token, frame)).unwrap();
    let by = |who| {
        block_on(relay.push_frames(id(who), space.space, vec![PushFrame::Upload(upload)])).results
    };
    assert_eq!(by(1), [Pushed::Rejected(ErrorCode::Malformed)]);
    // Its member's push takes it: the change is already held.
    assert_eq!(by(0), [Pushed::Duplicate(2)]);
    assert_eq!(by(0), [Pushed::Rejected(ErrorCode::Malformed)]);
}
