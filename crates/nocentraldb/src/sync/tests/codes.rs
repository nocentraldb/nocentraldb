//! What `SyncStatus::error` shows of the code the relay sent (RFC 0009,
//! "Sync and status"; NOC-146): every code RFC 0007 lists ("The errors")
//! as the relay wrote it, and any other text as the one fixed code
//! `unknown_error`, so the relay's own text never reaches the app.

use super::fake::{FakeRelay, Trouble};
use super::support::*;

const ME: Who = 1;

/// RFC 0007, whose table "The errors" lists the codes.
const RFC_0007: &str = include_str!("../../../../../docs/rfc/0007-relay.md");

/// Every code in RFC 0007's table of errors.
fn listed_codes() -> Vec<&'static str> {
    let table = RFC_0007
        .split("### The errors")
        .nth(1)
        .expect("RFC 0007 has the section \"The errors\"");
    let codes: Vec<_> = table
        .lines()
        .skip_while(|line| !line.starts_with('|'))
        .take_while(|line| line.starts_with('|'))
        .filter_map(|line| line.strip_prefix("| `"))
        .filter_map(|line| line.split('`').next())
        .collect();
    assert!(codes.len() >= 19, "RFC 0007 lists {} codes", codes.len());
    codes
}

/// What the sync status of a database shows once a relay refused its
/// `hello` with `code`.
async fn shown(code: &'static str) -> String {
    let relay = FakeRelay::start(Trouble::Refuse(code)).await;
    let db = open(ME, &relay.url()).await;
    let mut error = None;
    eventually("the sync status reports an error", || {
        error = db.sync().current().error;
        error.is_some()
    })
    .await;
    error.expect("an error")
}

#[tokio::test(flavor = "multi_thread")]
async fn every_code_rfc_0007_lists_is_shown_as_the_relay_sent_it() {
    for code in listed_codes() {
        assert_eq!(shown(code).await, code);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_code_rfc_0007_does_not_list_is_shown_as_unknown_error() {
    assert_eq!(shown("the relay's own text").await, "unknown_error");
}

#[tokio::test(flavor = "multi_thread")]
async fn an_empty_code_is_shown_as_unknown_error() {
    assert_eq!(shown("").await, "unknown_error");
}

#[tokio::test(flavor = "multi_thread")]
async fn an_overlong_code_is_shown_as_unknown_error() {
    let overlong: &'static str = "not_member".repeat(1000).leak();
    let shown = shown(overlong).await;
    assert!(
        shown == "unknown_error",
        "shown: {} bytes, not unknown_error",
        shown.len()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_listed_code_in_another_spelling_is_shown_as_unknown_error() {
    for code in ["NOT_MEMBER", "notMember", " not_member", "not_member\n"] {
        assert_eq!(shown(code).await, "unknown_error", "{code:?}");
    }
}
