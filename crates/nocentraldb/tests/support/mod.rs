//! What every test of the SDK shares: the test model's lock, and running
//! the SDK's futures. Every call of the SDK finishes on the device without
//! waiting (RFC 0009, "One write, one change"), so a future is polled once
//! and must be ready; a subscription's next page is polled the same way.
#![allow(dead_code)]

use std::future::Future;
use std::pin::pin;
use std::task::{Context, Poll, Waker};

use futures_core::Stream;
use nocentraldb::{Db, OpenOptions};

/// The lock of `tests/model/model.ts`.
pub const LOCK: &str = include_str!("../model/model.lock.json");

/// The output of `future`, which must be ready at its first poll.
pub fn now<F: Future>(future: F) -> F::Output {
    let mut cx = Context::from_waker(Waker::noop());
    match pin!(future).poll(&mut cx) {
        Poll::Ready(output) => output,
        Poll::Pending => panic!("the future waits, but the SDK never waits on the device"),
    }
}

/// The item `stream` has ready now, or `None` when it has none yet.
pub fn ready<S: Stream + Unpin>(stream: &mut S) -> Option<S::Item> {
    let mut cx = Context::from_waker(Waker::noop());
    match std::pin::Pin::new(stream).poll_next(&mut cx) {
        Poll::Ready(Some(item)) => Some(item),
        Poll::Ready(None) => panic!("the subscription ended"),
        Poll::Pending => None,
    }
}

/// A database in memory, opened from the test model's lock.
pub fn open() -> Db {
    now(Db::open(OpenOptions::new(LOCK).in_memory())).expect("the test lock opens")
}
