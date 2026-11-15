//! Erasing in a space whose members are only agents of one account, such
//! as an ad-hoc copy (Carsten, 2026-11-15, on NOC-326, Q4; pm, NOC-325, on
//! the coding agent's proposal): a device of the account is no member
//! there, so it holds no copy to check a sign-off against. It signs such a
//! space off only for an agent of its own account, by the certificate the
//! request carries; the erasure is provisional, and the agent's
//! `db.erase` checks the sign-off before it writes anything.

use super::support::*;
use crate::Value;
use crate::db::{Db, OpenOptions};
use crate::id::ObjectId;
use crate::space::Space;

const AGENT: Who = 1;
const DEVICE: Who = 2;
const STRANGER: Who = 5;

/// The database of `who` of the simulated account, an agent when `agent`.
async fn of_account(who: Who, url: &str, agent: bool) -> Db {
    let mut options = OpenOptions::new(&crate::personal_write_tests::lock())
        .in_memory()
        .relay(url);
    if agent {
        options = options.agent();
    }
    Db::open_as(options, [who; 32], [who ^ 0x80; 32], account())
        .await
        .expect("the personal lock opens")
}

async fn anna(space: &Space) -> ObjectId {
    space
        .write(|tx| {
            tx.create("projects", [("name", Value::from("Anna Schmidt"))])?;
            Ok(())
        })
        .await
        .unwrap()
        .ids[0]
}

#[tokio::test(flavor = "multi_thread")]
async fn a_device_of_the_account_signs_off_an_agents_space_it_does_not_hold() {
    let relay = LocalRelay::start().await;
    let agent = of_account(AGENT, &relay.url(), true).await;
    let device = of_account(DEVICE, &relay.url(), false).await;
    let copy = agent.spaces().create().await.unwrap();
    let id = anna(&copy).await;
    let object = copy.get("projects", id).await.unwrap().unwrap();

    let request = agent.erasure_request(&object).await.unwrap();
    let request = request.to_string().parse().unwrap();
    let signed = device.account().sign_off(&request).await.unwrap();
    assert!(signed.not_signed.is_empty(), "{:?}", signed.not_signed);
    let erased = agent.erase(&request, &signed.sign_offs).await.unwrap();

    assert_eq!(erased.shreds.len(), 1);
    assert!(erased.shreds[0].provisional);
    let read = copy.get("projects", id).await.unwrap().unwrap();
    assert_eq!(read.erased(), ["name"]);
}

/// A device performer is no agent: another device does not sign off a
/// space of it that it does not hold.
#[tokio::test(flavor = "multi_thread")]
async fn a_device_does_not_sign_off_a_space_it_does_not_hold_for_a_device() {
    let relay = LocalRelay::start().await;
    let performer = of_account(AGENT, &relay.url(), false).await;
    let device = of_account(DEVICE, &relay.url(), false).await;
    let space = performer.spaces().create().await.unwrap();
    let id = anna(&space).await;
    let object = space.get("projects", id).await.unwrap().unwrap();

    let request = performer.erasure_request(&object).await.unwrap();
    let signed = device.account().sign_off(&request).await.unwrap();

    assert_eq!(signed.not_signed, vec![space.id()]);
}

/// A device of another account does not sign off an agent's space it
/// does not hold.
#[tokio::test(flavor = "multi_thread")]
async fn a_device_of_another_account_does_not_sign_off_an_agents_space() {
    let relay = LocalRelay::start().await;
    let agent = of_account(AGENT, &relay.url(), true).await;
    let options = OpenOptions::new(&crate::personal_write_tests::lock())
        .in_memory()
        .relay(&relay.url());
    let stranger = Db::open_as(
        options,
        [STRANGER; 32],
        [STRANGER ^ 0x80; 32],
        space::other_account(1),
    )
    .await
    .unwrap();
    let copy = agent.spaces().create().await.unwrap();
    let id = anna(&copy).await;
    let object = copy.get("projects", id).await.unwrap().unwrap();

    let request = agent.erasure_request(&object).await.unwrap();
    let signed = stranger.account().sign_off(&request).await.unwrap();

    assert_eq!(signed.not_signed, vec![copy.id()]);
}
