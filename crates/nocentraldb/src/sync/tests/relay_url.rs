//! A relay URL may carry a token in its query string, checked by the
//! relay's `$connect` authorizer (NOC-417, NOC-495). The SDK connects with
//! the whole URL but never shows its query string or fragment: not in the
//! sync status, not in an error, not in `Debug`; nor the user and password
//! before its host.

use super::fake::{FakeRelay, Trouble};
use super::support::*;
use crate::{Db, OpenOptions};

const ME: Who = 1;
const TOKEN: &str = "s3cr3t-relay-token";

#[tokio::test(flavor = "multi_thread")]
async fn a_token_in_the_relay_url_never_shows_in_the_sync_status() {
    let relay = FakeRelay::start(Trouble::Refuse("unsupported_version")).await;
    let url = format!("{}/prod?token={TOKEN}#{TOKEN}", relay.url());
    let options = OpenOptions::new(LOCK).in_memory().relay(&url);
    let options_debug = format!("{options:?}");
    let db = Db::open_as(options, [ME; 32], [ME ^ 0x80; 32], account())
        .await
        .expect("the test lock opens");
    let mut status = db.sync().current();
    // The relay's answer shows the SDK connected with the whole URL.
    eventually("the relay's refusal reaches the sync status", || {
        status = db.sync().current();
        status.error.is_some()
    })
    .await;
    assert_eq!(status.error.as_deref(), Some("unsupported_version"));
    for shown in [format!("{status:?}"), format!("{db:?}"), options_debug] {
        assert!(!shown.contains(TOKEN), "{shown}");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_token_in_a_refused_relay_url_never_shows_in_the_error() {
    let url = format!("https://app:{TOKEN}@relay.example.com/prod?token={TOKEN}#{TOKEN}");
    let options = OpenOptions::new(LOCK).in_memory().relay(&url);
    let error = Db::open_as(options, [ME; 32], [ME ^ 0x80; 32], account())
        .await
        .expect_err("an https URL is no relay URL");
    for shown in [format!("{error}"), format!("{error:?}")] {
        assert!(!shown.contains(TOKEN), "{shown}");
    }
}
