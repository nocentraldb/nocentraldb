//! The suite's rules for large changes (RFC 0007, "Large changes"): an
//! upload is filled once and taken once, by its member in its space, and a
//! download token names one entry of the log. Expired uploads and download
//! tokens are swept when a new one begins.

use ncdb_core::ids::MemberId;

use super::{Backend, append, block_on, entry, space};
use crate::store::{RelayStore, Token, Upload, UploadId};

/// An upload in space 1 for member `m`, of `size` bytes, its URL valid
/// until `expires_at`.
fn upload(n: u8, m: u8, size: u64, expires_at: u64) -> Upload {
    Upload {
        id: UploadId([n; 16]),
        token: Token([n; 32]),
        space: space(1),
        member: MemberId([m; 32]),
        size,
        expires_at,
    }
}

pub fn an_upload_is_filled_once_and_taken_once<B: Backend>(backend: &B) {
    let store = backend.connect();
    let begun = upload(1, 1, 3, 1_000);
    block_on(store.begin_upload(begun.clone(), 0)).unwrap();
    assert_eq!(
        block_on(store.upload(Token([1; 32]))).unwrap(),
        Some((begun.clone(), false))
    );
    assert!(block_on(store.fill_upload(Token([1; 32]), vec![1, 2, 3])).unwrap());
    assert!(!block_on(store.fill_upload(Token([1; 32]), vec![4, 5, 6])).unwrap());
    assert_eq!(
        block_on(store.upload(Token([1; 32]))).unwrap(),
        Some((begun, true))
    );
    let take = |member: u8| {
        block_on(store.take_upload(space(1), MemberId([member; 32]), UploadId([1; 16])))
    };
    assert_eq!(take(1).unwrap(), Some(vec![1, 2, 3]));
    assert_eq!(take(1).unwrap(), None);
    assert_eq!(block_on(store.upload(Token([1; 32]))).unwrap(), None);
}

pub fn an_upload_is_taken_only_by_its_member_in_its_space_once_filled<B: Backend>(backend: &B) {
    let store = backend.connect();
    block_on(store.begin_upload(upload(2, 1, 1, 1_000), 0)).unwrap();
    let take = |space_n: u8, member: u8| {
        block_on(store.take_upload(space(space_n), MemberId([member; 32]), UploadId([2; 16])))
            .unwrap()
    };
    assert_eq!(take(1, 1), None, "not filled yet");
    assert!(block_on(store.fill_upload(Token([2; 32]), vec![9])).unwrap());
    assert_eq!(take(1, 2), None, "another member");
    assert_eq!(take(2, 1), None, "another space");
    assert_eq!(take(1, 1), Some(vec![9]));
}

pub fn a_download_token_names_one_entry<B: Backend>(backend: &B) {
    let store = backend.connect();
    append(&store, space(1), 0, vec![entry(1), entry(2)]);
    block_on(store.begin_download(Token([3; 32]), space(1), 2, 500, 0)).unwrap();
    assert_eq!(
        block_on(store.download(Token([3; 32]))).unwrap(),
        Some((space(1), 2, 500))
    );
    assert_eq!(block_on(store.download(Token([4; 32]))).unwrap(), None);
}

pub fn expired_uploads_and_downloads_are_swept<B: Backend>(backend: &B) {
    let store = backend.connect();
    const DAY: u64 = 24 * 60 * 60 * 1000;
    block_on(store.begin_upload(upload(5, 1, 1, 100), 0)).unwrap();
    block_on(store.begin_download(Token([6; 32]), space(1), 1, 100, 0)).unwrap();
    // Within a day after its URL expired, an upload is kept: its frame may
    // still be pushed. A download token goes as soon as it expired.
    block_on(store.begin_upload(upload(7, 1, 1, 10_000), 200)).unwrap();
    assert!(block_on(store.upload(Token([5; 32]))).unwrap().is_some());
    assert_eq!(block_on(store.download(Token([6; 32]))).unwrap(), None);
    block_on(store.begin_upload(upload(8, 1, 1, 10_000 + DAY), 101 + DAY)).unwrap();
    assert_eq!(block_on(store.upload(Token([5; 32]))).unwrap(), None);
    assert!(block_on(store.upload(Token([7; 32]))).unwrap().is_some());
}
