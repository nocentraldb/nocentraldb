//! Pairing a device that already syncs while it waits (NOC-337): a real
//! app opens a new device with its relay before it pairs it, so the device
//! receives the spaces the pairing adds it to while it still waits. Once
//! it completes the pairing it follows them, and writes its records into
//! the account space, at once, without a reopen.

use super::support::*;
use crate::db::{Db, OpenOptions};
use crate::pairing::PairKind;
use crate::value::Value;

/// The laptop, the account's first device.
const LAPTOP: Who = 1;

/// A new database waiting to be paired, syncing with the relay at `url`.
async fn waiting(url: &str) -> Db {
    Db::open(OpenOptions::new(LOCK).in_memory().relay(url).pairing())
        .await
        .expect("the test lock opens")
}

/// The member id of `db`.
fn member(db: &Db) -> [u8; 32] {
    crate::db::lock(&db.inner).member.key().id().0
}

/// The laptop pairs a phone that syncs while it waits, and the phone
/// completes the pairing; returns the phone. With `received`, the phone
/// first holds every space the pairing added it to, the account space
/// too: the relay announces each with an `added` of its own, in no order
/// RFC 0007 sets, so holding the app space says nothing of the account
/// space. Without, it completes at once, before the spaces may reach it.
async fn paired(laptop: &Db, url: &str, received: bool) -> Db {
    let phone = waiting(url).await;
    let invitation = now(phone.invitation());
    let paired = now(laptop.account().pair(PairKind::Device, &invitation)).unwrap();
    if received {
        synced(laptop).await;
        let account = crate::account::space(&crate::db::lock(&laptop.inner)).unwrap();
        let added = paired.added[0].0;
        eventually("the phone holds its spaces before it completes", || {
            let inner = crate::db::lock(&phone.inner);
            crate::space::writable_by(&inner, account, crate::account::id()).is_ok()
                && crate::space::writable_by(&inner, added.0, inner.model_id).is_ok()
        })
        .await;
    }
    now(phone.account().complete_pairing(&paired.answer)).unwrap();
    phone
}

/// The laptop reads the phone's membership record of `space` in the
/// account space.
async fn recorded(laptop: &Db, phone: &Db, space: crate::id::SpaceId) {
    let me = member(phone);
    eventually("the laptop reads the phone's membership record", || {
        let inner = crate::db::lock(&laptop.inner);
        crate::account::memberships(&inner).is_ok_and(|records| {
            records
                .iter()
                .any(|record| record.member == me && record.space == space.0.0)
        })
    })
    .await;
}

/// The phone reads a task the laptop writes into `project` after the
/// pairing.
async fn reads_a_new_task(laptop: &Db, phone: &Db, project: &crate::space::Space) {
    let task = now(project.write(|tx| {
        tx.create("tasks", [("title", Value::from("Buy milk"))])?;
        Ok(())
    }))
    .unwrap()
    .ids[0];
    synced(laptop).await;
    eventually("the phone reads the task written after the pairing", || {
        now(phone.spaces().get(project.id()))
            .is_ok_and(|space| now(space.get("tasks", task)).is_ok_and(|read| read.is_some()))
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_phone_that_syncs_while_it_waits_writes_its_records_into_the_account_space() {
    let relay = LocalRelay::start().await;
    let laptop = open(LAPTOP, &relay.url()).await;
    let project = now(laptop.spaces().create()).unwrap();
    let phone = paired(&laptop, &relay.url(), true).await;
    recorded(&laptop, &phone, project.id()).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_phone_that_syncs_while_it_waits_reads_a_task_written_after_the_pairing() {
    let relay = LocalRelay::start().await;
    let laptop = open(LAPTOP, &relay.url()).await;
    let project = now(laptop.spaces().create()).unwrap();
    let phone = paired(&laptop, &relay.url(), true).await;
    reads_a_new_task(&laptop, &phone, &project).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_phone_that_syncs_while_it_waits_holds_the_accounts_account_space() {
    let relay = LocalRelay::start().await;
    let laptop = open(LAPTOP, &relay.url()).await;
    now(laptop.spaces().create()).unwrap();
    let phone = paired(&laptop, &relay.url(), true).await;

    let ours = crate::account::space(&crate::db::lock(&laptop.inner)).unwrap();
    let held = crate::account::space(&crate::db::lock(&phone.inner));
    assert_eq!(held, Some(ours), "at once, without waiting for a fold");
}

/// The phone completes the pairing before the spaces it was added to
/// reach it: they arrive after, and it writes its records and reads the
/// laptop's new task all the same.
#[tokio::test(flavor = "multi_thread")]
async fn a_phone_that_completes_before_its_spaces_arrive_records_and_reads_them() {
    let relay = LocalRelay::start().await;
    let laptop = open(LAPTOP, &relay.url()).await;
    let project = now(laptop.spaces().create()).unwrap();
    let phone = paired(&laptop, &relay.url(), false).await;
    recorded(&laptop, &phone, project.id()).await;
    reads_a_new_task(&laptop, &phone, &project).await;
}
