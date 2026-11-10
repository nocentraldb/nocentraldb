//! The code a relay sends when it refuses a pushed change, as the view's
//! log records it beside the change (NOC-188): mapped the same way as
//! `SyncStatus::error` (NOC-146), so every code RFC 0007 lists is stored
//! as the relay wrote it, and any other text as `unknown_error`.

use super::fake::{FakeRelay, Trouble};
use super::support::*;
use crate::value::Value;

const ME: Who = 1;

/// The codes the view's log records once a relay rejected every change
/// the database pushed with `code`.
async fn stored(code: &'static str) -> Vec<String> {
    let relay = FakeRelay::start(Trouble::RejectPush(code)).await;
    let db = open(ME, &relay.url()).await;
    let space = db.spaces().create().await.unwrap();
    space
        .write(|tx| {
            tx.create("tasks", [("title", Value::from("refused"))])?;
            Ok(())
        })
        .await
        .unwrap();
    eventually("the relay refused every change", || {
        let status = db.sync().current();
        status.unconfirmed == 0 && status.refused > 0
    })
    .await;
    let inner = db.lock();
    let refused = inner.view.refused(space.id().0).unwrap();
    refused.into_iter().map(|(_, refusal)| refusal.0).collect()
}

/// Asserts that every stored code is `expected`, and shows an overlong
/// one by its length only.
fn assert_all(stored: &[String], expected: &str) {
    assert!(!stored.is_empty(), "no refusal stored");
    for code in stored {
        assert!(
            code == expected,
            "stored {:?}, not {expected:?}",
            code.chars().take(40).collect::<String>()
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_code_rfc_0007_lists_is_stored_as_the_relay_sent_it() {
    assert_all(&stored("author_not_member").await, "author_not_member");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_code_rfc_0007_does_not_list_is_stored_as_unknown_error() {
    assert_all(&stored("the relay's own text").await, "unknown_error");
}

#[tokio::test(flavor = "multi_thread")]
async fn an_empty_code_is_stored_as_unknown_error() {
    assert_all(&stored("").await, "unknown_error");
}

#[tokio::test(flavor = "multi_thread")]
async fn an_overlong_code_is_stored_as_unknown_error() {
    let overlong: &'static str = "not_member".repeat(1000).leak();
    assert_all(&stored(overlong).await, "unknown_error");
}
