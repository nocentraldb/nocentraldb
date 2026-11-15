//! Taking back a provisional erasure (NOC-325; Carsten, 2026-11-15, on
//! NOC-326, Q7 a; RFC 0008, "A provisional erasure in a space with one
//! account"): within its 7 days, an admin of the performer's account calls
//! `space.erasures().take_back(shred)`, which writes the erasure decision
//! that takes it back. The person's fields read again, and the notice
//! `TakenBack` says what it does not do. Anyone else, or a shred that is
//! not provisional, is refused, and nothing is written.

use super::support::*;
use crate::Value;
use crate::db::{Db, OpenOptions};
use crate::erasures::ErasureState;
use crate::error::ErrorCode;
use crate::id::ObjectId;
use crate::members::{AddMember, NoticeCode, Role};
use crate::space::Space;

const PERFORMER: Who = 1;
const OWN_DEVICE: Who = 2;
const SIGNER: Who = 4;

async fn add(space: &Space, db: &Db, role: Role) {
    let invitation = db.invitation().await;
    let options = AddMember {
        role,
        fingerprint: Some(invitation.fingerprint()),
    };
    space.members().add(&invitation, options).await.unwrap();
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

async fn erased(space: &Space, object: ObjectId) -> Vec<String> {
    let read = space.get("projects", object).await.unwrap().unwrap();
    read.erased().to_vec()
}

/// A space of one account, the performer and its second device admins,
/// with Anna erased provisionally: the performer, the device, the space,
/// Anna and the shred.
async fn provisional(relay: &LocalRelay) -> (Db, Db, Space, ObjectId, crate::id::ChangeId) {
    let lock_json = crate::personal_write_tests::lock();
    let performer = open_with(PERFORMER, &relay.url(), &lock_json).await;
    let device = open_with(OWN_DEVICE, &relay.url(), &lock_json).await;
    let x = performer.spaces().create().await.unwrap();
    add(&x, &device, Role::Admin).await;
    let id = anna(&x).await;
    synced(&performer).await;
    let object = x.get("projects", id).await.unwrap().unwrap();
    let request = performer.erasure_request(&object).await.unwrap();
    eventually("the device signs the space off", || {
        let signed = now(device.account().sign_off(&request)).unwrap();
        signed.not_signed.is_empty()
    })
    .await;
    let signed = device.account().sign_off(&request).await.unwrap();
    let erased_now = performer.erase(&request, &signed.sign_offs).await.unwrap();
    assert!(erased_now.shreds[0].provisional);
    (performer, device, x, id, erased_now.shreds[0].shred)
}

#[tokio::test(flavor = "multi_thread")]
async fn an_admin_of_the_account_takes_a_provisional_erasure_back_and_the_fields_read_again() {
    let relay = LocalRelay::start().await;
    let (_performer, _device, x, anna, shred) = provisional(&relay).await;
    assert_eq!(erased(&x, anna).await, ["name"]);

    let notice = x.erasures().take_back(shred).await.unwrap();

    assert_eq!(notice.code, NoticeCode::TakenBack);
    assert!(erased(&x, anna).await.is_empty());
    let audit = x.erasures().list().await.unwrap();
    let erasure = audit.iter().find(|e| e.shred == shred).unwrap();
    assert_eq!(erasure.state, ErasureState::TakenBack);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_shred_taken_back_once_is_not_taken_back_again() {
    let relay = LocalRelay::start().await;
    let (_performer, _device, x, _anna, shred) = provisional(&relay).await;
    x.erasures().take_back(shred).await.unwrap();

    let again = x.erasures().take_back(shred).await.unwrap_err();

    assert_eq!(again.code(), ErrorCode::InvalidValue);
}

/// A final erasure, signed off by a second account, cannot be taken back.
#[tokio::test(flavor = "multi_thread")]
async fn a_final_erasure_is_not_taken_back() {
    let relay = LocalRelay::start().await;
    let lock_json = crate::personal_write_tests::lock();
    let performer = open_with(PERFORMER, &relay.url(), &lock_json).await;
    let options = OpenOptions::new(&lock_json).in_memory().relay(&relay.url());
    let signer = Db::open_as(
        options,
        [SIGNER; 32],
        [SIGNER ^ 0x80; 32],
        space::other_account(1),
    )
    .await
    .unwrap();
    let x = performer.spaces().create().await.unwrap();
    add(&x, &signer, Role::Admin).await;
    let id = anna(&x).await;
    synced(&performer).await;
    let object = x.get("projects", id).await.unwrap().unwrap();
    let request = performer.erasure_request(&object).await.unwrap();
    eventually("the signer signs the space off", || {
        now(signer.account().sign_off(&request))
            .unwrap()
            .not_signed
            .is_empty()
    })
    .await;
    let signed = signer.account().sign_off(&request).await.unwrap();
    let shred = performer.erase(&request, &signed.sign_offs).await.unwrap().shreds[0].shred;

    let refused = x.erasures().take_back(shred).await.unwrap_err();

    assert_eq!(refused.code(), ErrorCode::InvalidValue);
    assert_eq!(erased(&x, id).await, ["name"]);
}

/// A writer of the account is no admin: it may not take an erasure back.
#[tokio::test(flavor = "multi_thread")]
async fn a_member_that_is_no_admin_may_not_take_it_back() {
    let relay = LocalRelay::start().await;
    let lock_json = crate::personal_write_tests::lock();
    let performer = open_with(PERFORMER, &relay.url(), &lock_json).await;
    let writer = open_with(OWN_DEVICE, &relay.url(), &lock_json).await;
    let x = performer.spaces().create().await.unwrap();
    add(&x, &writer, Role::Writer).await;
    let id = anna(&x).await;
    synced(&performer).await;
    let object = x.get("projects", id).await.unwrap().unwrap();
    let request = performer.erasure_request(&object).await.unwrap();
    let signed = performer.account().sign_off(&request).await.unwrap();
    let shred = performer.erase(&request, &signed.sign_offs).await.unwrap().shreds[0].shred;
    synced(&performer).await;
    let on_writer = writer.spaces().get(x.id()).await;
    eventually("the writer holds the space", || {
        now(writer.spaces().get(x.id())).is_ok()
    })
    .await;
    let on_writer = on_writer.unwrap_or_else(|_| now(writer.spaces().get(x.id())).unwrap());
    eventually("the writer holds the shred", || {
        now(on_writer.erasures().list()).is_ok_and(|audit| !audit.is_empty())
    })
    .await;

    let refused = on_writer.erasures().take_back(shred).await.unwrap_err();

    assert_eq!(refused.code(), ErrorCode::NotAllowed);
}
