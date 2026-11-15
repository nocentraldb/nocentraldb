//! Erasing a person in every space of the database (NOC-325; Carsten,
//! 2026-11-15, on NOC-326; RFC 0003, "Shredding a person's key"):
//! `db.erasure_request(&object)` lists, for each space where the member
//! may write and the person has a key, the space, the person-key id and
//! the performer; a device of another account signs it off with
//! `db.account().sign_off(&request)`, where its account may; and
//! `db.erase(&request, &sign_offs)` writes one shred per signed space,
//! after checking every sign-off, and returns which it erased, which it
//! skipped and the notice `Erased`.

use super::support::*;
use crate::Value;
use crate::db::{Db, OpenOptions, lock};
use crate::erase::{ErasureRequest, SignOffs, Skipped};
use crate::error::ErrorCode;
use crate::id::{ObjectId, SpaceId};
use crate::members::{AddMember, NoticeCode, Role};
use crate::object::Object;
use crate::space::Space;
use crate::tx::Create;

/// The performer, of the simulated account.
const PERFORMER: Who = 1;
/// A second device of the performer's account.
const OWN_DEVICE: Who = 2;
/// The signer, of a second account.
const SIGNER: Who = 4;

/// The database of `who` of the second account, on the personal model.
async fn of_second_account(who: Who, url: &str) -> Db {
    let options = OpenOptions::new(&crate::personal_write_tests::lock())
        .in_memory()
        .relay(url);
    Db::open_as(
        options,
        [who; 32],
        [who ^ 0x80; 32],
        space::other_account(1),
    )
    .await
    .expect("the personal lock opens")
}

/// Adds `db`'s member to `space` with `role`, confirming its account.
async fn add(space: &Space, db: &Db, role: Role) {
    let invitation = db.invitation().await;
    let options = AddMember {
        role,
        fingerprint: Some(invitation.fingerprint()),
    };
    space.members().add(&invitation, options).await.unwrap();
}

/// Writes the person Anna into `space`, the same person as `same_as`
/// when it is given. Her object as `space` reads it.
async fn anna(space: &Space, same_as: Option<Object>) -> Object {
    let id = space
        .write(|tx| {
            tx.create_with(
                "projects",
                [("name", Value::from("Anna Schmidt"))],
                Create {
                    same_person_as: same_as,
                },
            )?;
            Ok(())
        })
        .await
        .unwrap()
        .ids[0];
    space.get("projects", id).await.unwrap().unwrap()
}

/// The fields of `object` that read as erased in `space`, once read.
async fn erased(space: &Space, object: ObjectId) -> Vec<String> {
    let read = space.get("projects", object).await.unwrap().unwrap();
    read.erased().to_vec()
}

/// Whether `db` holds `space` and a person key of it.
fn holds_a_person_key(db: &Db, space: SpaceId) -> bool {
    let inner = lock(&db.inner);
    inner
        .spaces
        .get(&space.0)
        .is_some_and(|state| state.checks.resolved().person_keys().next().is_some())
}

/// The performer's two spaces, each with Anna, the signer an admin of
/// each: the performer, the signer, both spaces and Anna in each.
async fn two_spaces(relay: &LocalRelay) -> (Db, Db, Space, Space, Object, Object) {
    let lock_json = crate::personal_write_tests::lock();
    let performer = open_with(PERFORMER, &relay.url(), &lock_json).await;
    let signer = of_second_account(SIGNER, &relay.url()).await;
    let x = performer.spaces().create().await.unwrap();
    let y = performer.spaces().create().await.unwrap();
    add(&x, &signer, Role::Admin).await;
    add(&y, &signer, Role::Admin).await;
    let in_x = anna(&x, None).await;
    let in_y = anna(&y, Some(in_x.clone())).await;
    synced(&performer).await;
    eventually("the signer holds both spaces and their person keys", || {
        holds_a_person_key(&signer, x.id()) && holds_a_person_key(&signer, y.id())
    })
    .await;
    (performer, signer, x, y, in_x, in_y)
}

#[tokio::test(flavor = "multi_thread")]
async fn a_second_account_signs_off_and_the_person_is_erased_in_both_spaces_for_good() {
    let relay = LocalRelay::start().await;
    let (performer, signer, x, y, in_x, in_y) = two_spaces(&relay).await;

    let request = performer.erasure_request(&in_x).await.unwrap();
    assert_eq!(request.spaces(), vec![x.id(), y.id()].into_iter().collect());
    let signed = signer.account().sign_off(&request).await.unwrap();
    assert!(signed.not_signed.is_empty(), "{:?}", signed.not_signed);
    let erased_now = performer.erase(&request, &signed.sign_offs).await.unwrap();

    assert_eq!(erased_now.notice.code, NoticeCode::Erased);
    assert!(erased_now.skipped.is_empty(), "{:?}", erased_now.skipped);
    let mut spaces: Vec<SpaceId> = erased_now.shreds.iter().map(|s| s.space).collect();
    spaces.sort();
    let mut expected = vec![x.id(), y.id()];
    expected.sort();
    assert_eq!(spaces, expected);
    assert!(erased_now.shreds.iter().all(|shred| !shred.provisional));
    assert_eq!(erased(&x, in_x.id()).await, ["name"]);
    assert_eq!(erased(&y, in_y.id()).await, ["name"]);
    // Each shred is the one the space's audit lists.
    for shred in &erased_now.shreds {
        let space = if shred.space == x.id() { &x } else { &y };
        let audit = space.erasures().list().await.unwrap();
        assert!(audit.iter().any(|erasure| erasure.shred == shred.shred));
    }
}

/// A space whose members all belong to one account: a device of the
/// account signs off, and the erasure is provisional (RFC 0008, "A
/// provisional erasure in a space with one account").
#[tokio::test(flavor = "multi_thread")]
async fn in_a_space_of_one_account_a_device_of_it_signs_off_and_the_erasure_is_provisional() {
    let relay = LocalRelay::start().await;
    let lock_json = crate::personal_write_tests::lock();
    let performer = open_with(PERFORMER, &relay.url(), &lock_json).await;
    let device = open_with(OWN_DEVICE, &relay.url(), &lock_json).await;
    let x = performer.spaces().create().await.unwrap();
    add(&x, &device, Role::Admin).await;
    let in_x = anna(&x, None).await;
    synced(&performer).await;
    eventually("the device holds the space and its person key", || {
        holds_a_person_key(&device, x.id())
    })
    .await;

    let request = performer.erasure_request(&in_x).await.unwrap();
    let signed = device.account().sign_off(&request).await.unwrap();
    let erased_now = performer.erase(&request, &signed.sign_offs).await.unwrap();

    assert_eq!(erased_now.shreds.len(), 1);
    assert!(erased_now.shreds[0].provisional);
    assert_eq!(erased(&x, in_x.id()).await, ["name"]);
}

/// The signer is no member of the second space: it signs off the first
/// only, says so, and the erasure skips the second.
#[tokio::test(flavor = "multi_thread")]
async fn a_space_the_signer_cannot_sign_is_reported_and_skipped() {
    let relay = LocalRelay::start().await;
    let lock_json = crate::personal_write_tests::lock();
    let performer = open_with(PERFORMER, &relay.url(), &lock_json).await;
    let signer = of_second_account(SIGNER, &relay.url()).await;
    let x = performer.spaces().create().await.unwrap();
    let y = performer.spaces().create().await.unwrap();
    add(&x, &signer, Role::Admin).await;
    let in_x = anna(&x, None).await;
    let in_y = anna(&y, Some(in_x.clone())).await;
    synced(&performer).await;
    eventually("the signer holds the first space's person key", || {
        holds_a_person_key(&signer, x.id())
    })
    .await;

    let request = performer.erasure_request(&in_x).await.unwrap();
    let signed = signer.account().sign_off(&request).await.unwrap();
    assert_eq!(signed.not_signed, vec![y.id()]);
    let erased_now = performer.erase(&request, &signed.sign_offs).await.unwrap();

    assert_eq!(erased_now.shreds.len(), 1);
    assert_eq!(erased_now.shreds[0].space, x.id());
    assert_eq!(erased_now.skipped, vec![(y.id(), Skipped::NoSignOff)]);
    assert_eq!(erased(&x, in_x.id()).await, ["name"]);
    assert!(erased(&y, in_y.id()).await.is_empty());
}

/// A sign-off whose signature was altered does not hold: the erasure is
/// refused before anything is written, in any space.
#[tokio::test(flavor = "multi_thread")]
async fn a_sign_off_that_would_not_hold_refuses_the_erasure_before_anything_is_written() {
    let relay = LocalRelay::start().await;
    let (performer, signer, x, y, in_x, in_y) = two_spaces(&relay).await;
    let request = performer.erasure_request(&in_x).await.unwrap();
    let signed = signer.account().sign_off(&request).await.unwrap();
    // The last byte of the text's layout is the last sign-off's signature.
    let text = signed.sign_offs.to_string();
    let body = text.strip_prefix("ncdb-sign-off-1:").unwrap();
    let mut bytes = data_encoding::BASE64URL_NOPAD
        .decode(body.as_bytes())
        .unwrap();
    *bytes.last_mut().unwrap() ^= 1;
    let forged: SignOffs = format!(
        "ncdb-sign-off-1:{}",
        data_encoding::BASE64URL_NOPAD.encode(&bytes)
    )
    .parse()
    .unwrap();

    let refused = performer.erase(&request, &forged).await.unwrap_err();

    assert_eq!(refused.code(), ErrorCode::InvalidValue);
    assert!(x.erasures().list().await.unwrap().is_empty());
    assert!(y.erasures().list().await.unwrap().is_empty());
    assert!(erased(&x, in_x.id()).await.is_empty());
    assert!(erased(&y, in_y.id()).await.is_empty());
}

/// Sign-offs of another person's keys sign off none of this person's:
/// every space is skipped, and nothing is written.
#[tokio::test(flavor = "multi_thread")]
async fn sign_offs_of_another_person_sign_off_nothing_of_this_one() {
    let relay = LocalRelay::start().await;
    let (performer, signer, x, _y, in_x, _in_y) = two_spaces(&relay).await;
    let bob = x
        .write(|tx| {
            tx.create("projects", [("name", Value::from("Bob Meier"))])?;
            Ok(())
        })
        .await
        .unwrap()
        .ids[0];
    let bob = x.get("projects", bob).await.unwrap().unwrap();
    synced(&performer).await;
    let for_bob = performer.erasure_request(&bob).await.unwrap();
    eventually("the signer holds Bob's person key", || {
        lock(&signer.inner).spaces[&x.id().0]
            .checks
            .resolved()
            .person_keys()
            .count()
            == 2
    })
    .await;
    let bobs = signer.account().sign_off(&for_bob).await.unwrap().sign_offs;

    let for_anna = performer.erasure_request(&in_x).await.unwrap();
    let erased_now = performer.erase(&for_anna, &bobs).await.unwrap();

    assert!(erased_now.shreds.is_empty());
    assert_eq!(erased_now.skipped.len(), 2);
    assert!(
        erased_now
            .skipped
            .iter()
            .all(|(_, why)| *why == Skipped::NoSignOff)
    );
    assert!(erased(&x, in_x.id()).await.is_empty());
}

/// Both texts round-trip, and a text of the wrong kind is refused.
#[tokio::test(flavor = "multi_thread")]
async fn the_request_and_the_sign_offs_have_a_text_form() {
    let relay = LocalRelay::start().await;
    let (performer, signer, _x, _y, in_x, _in_y) = two_spaces(&relay).await;
    let request = performer.erasure_request(&in_x).await.unwrap();
    let text = request.to_string();
    assert!(text.starts_with("ncdb-erase-1:"));
    assert_eq!(text.parse::<ErasureRequest>().unwrap(), request);

    let signed = signer.account().sign_off(&request).await.unwrap();
    let text = signed.sign_offs.to_string();
    assert!(text.starts_with("ncdb-sign-off-1:"));
    assert_eq!(text.parse::<SignOffs>().unwrap(), signed.sign_offs);

    let wrong = "ncdb-invite-1:AAAA".parse::<ErasureRequest>().unwrap_err();
    assert_eq!(wrong.code(), ErrorCode::InvalidValue);
    let wrong = request.to_string().parse::<SignOffs>().unwrap_err();
    assert_eq!(wrong.code(), ErrorCode::InvalidValue);
}

/// The performer's own account may not sign off where a second account
/// is a member, and a database that holds no space of the request signs
/// none of them.
#[tokio::test(flavor = "multi_thread")]
async fn an_account_signs_off_only_where_rfc_0008_lets_it() {
    let relay = LocalRelay::start().await;
    let (performer, _signer, x, y, in_x, _in_y) = two_spaces(&relay).await;
    let request = performer.erasure_request(&in_x).await.unwrap();

    let own = performer.account().sign_off(&request).await.unwrap();
    assert_eq!(own.sign_offs, SignOffs::default());
    let mut not_signed = own.not_signed;
    not_signed.sort();
    let mut expected = vec![x.id(), y.id()];
    expected.sort();
    assert_eq!(not_signed, expected);

    let stranger = of_second_account(5, &relay.url()).await;
    let none = stranger.account().sign_off(&request).await.unwrap();
    assert_eq!(none.sign_offs, SignOffs::default());
    assert_eq!(none.not_signed.len(), 2);
}
