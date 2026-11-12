//! The account space (RFC 0010, "The account space"; NOC-266): the
//! account's first device creates it with the model the SDK ships, and
//! records itself and each space it joins or leaves. A second device of
//! the account, once in the account space, reads the first device's
//! spaces from it. The account space is no app space: `spaces().list()`
//! leaves it out and `spaces().get` refuses it. Its content never holds
//! the account key.
//!
//! Pairing, which adds the second device to the account space, is
//! NOC-267's: here the first device adds the second as an admin directly,
//! as an admin adds any member (RFC 0008, "Adding a member").

use super::support::*;
use crate::account::{self, Kind, MembershipRecord};
use crate::db::{Db, lock};
use crate::error::ErrorCode;
use crate::id::SpaceId;
use crate::members::{AddMember, Role};
use crate::value::Value;

/// The account's first device.
const A: Who = 1;
/// The account's second device.
const B: Who = 2;
/// Another member, which A adds to a space so that A can leave it.
const C: Who = 3;

fn account_space(db: &Db) -> SpaceId {
    SpaceId(account::space(&lock(&db.inner)).expect("the account space"))
}

fn memberships(db: &Db) -> Vec<MembershipRecord> {
    account::memberships(&lock(&db.inner)).unwrap()
}

/// The spaces `member` belongs to, as the account space of `db` records
/// them.
fn spaces_of(db: &Db, member: [u8; 32]) -> Vec<SpaceId> {
    memberships(db)
        .into_iter()
        .filter(|m| m.member == member)
        .map(|m| SpaceId(m.space))
        .collect()
}

fn member_id(db: &Db) -> [u8; 32] {
    lock(&db.inner).member.key().id().0
}

#[tokio::test(flavor = "multi_thread")]
async fn the_first_device_creates_the_account_space_and_records_itself() {
    let relay = LocalRelay::start().await;
    let a = open(A, &relay.url()).await;

    let members = account::members(&lock(&a.inner)).unwrap();
    assert_eq!(members.len(), 1, "{members:?}");
    assert_eq!(members[0].member, member_id(&a));
    assert_eq!(members[0].kind, Kind::Device);
    assert!(!members[0].removed);
}

#[tokio::test(flavor = "multi_thread")]
async fn the_account_space_is_no_app_space() {
    let relay = LocalRelay::start().await;
    let a = open(A, &relay.url()).await;
    let app = now(a.spaces().create()).unwrap();

    let listed: Vec<SpaceId> = now(a.spaces().list())
        .unwrap()
        .into_iter()
        .map(|info| info.id)
        .collect();
    assert_eq!(listed, [app.id()]);
    let refused = now(a.spaces().get(account_space(&a))).unwrap_err();
    assert_eq!(refused.code(), ErrorCode::SpaceOfAnotherModel);
}

#[tokio::test(flavor = "multi_thread")]
async fn each_space_the_device_joins_or_leaves_is_recorded() {
    let relay = LocalRelay::start().await;
    let a = open(A, &relay.url()).await;
    let c = open(C, &relay.url()).await;
    let first = now(a.spaces().create()).unwrap();
    let second = now(a.spaces().create()).unwrap();

    let mut recorded = spaces_of(&a, member_id(&a));
    recorded.sort();
    let mut created = vec![first.id(), second.id()];
    created.sort();
    assert_eq!(recorded, created);
    assert!(memberships(&a).iter().all(|m| m.role == Role::Admin));

    // A hands the second space to C and leaves it.
    let invitation = now(c.invitation());
    let options = AddMember {
        role: Role::Admin,
        fingerprint: None,
    };
    now(second.members().add(&invitation, options)).unwrap();
    now(second.leave()).unwrap();

    assert_eq!(spaces_of(&a, member_id(&a)), [first.id()]);

    // C records the space it was added to, once it holds it.
    synced(&a).await;
    eventually("C records the space it joined", || {
        spaces_of(&c, member_id(&c)).contains(&second.id())
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_second_device_reads_the_first_devices_spaces_from_the_account_space() {
    let relay = LocalRelay::start().await;
    let a = open(A, &relay.url()).await;
    let b = open_joining(B, &relay.url()).await;
    let app = now(a.spaces().create()).unwrap();
    now(app.write(|tx| {
        tx.create("tasks", [("title", Value::from("on A"))])?;
        Ok(())
    }))
    .unwrap();
    assert!(
        account::space(&lock(&b.inner)).is_none(),
        "a joining device creates no account space"
    );

    // Pairing is NOC-267's: A adds B to the account space directly.
    let invitation = now(b.invitation());
    let options = AddMember {
        role: Role::Admin,
        fingerprint: None,
    };
    let on_a = handle(&a, account_space(&a).0);
    now(on_a.members().add(&invitation, options)).unwrap();
    synced(&a).await;

    eventually("B reads A's spaces from the account space", || {
        account::space(&lock(&b.inner)) == Some(account_space(&a).0)
            && spaces_of(&b, member_id(&a)) == [app.id()]
    })
    .await;
    // B records itself in the account space it joined.
    eventually("B records itself", || {
        account::members(&lock(&b.inner))
            .unwrap()
            .iter()
            .any(|m| m.member == member_id(&b) && m.kind == Kind::Device)
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn the_account_space_never_holds_the_account_key() {
    let relay = LocalRelay::start().await;
    let a = open(A, &relay.url()).await;
    now(a.spaces().create()).unwrap();

    let inner = lock(&a.inner);
    let secret = inner.account.to_bytes();
    let hex: String = secret.iter().map(|b| format!("{b:02x}")).collect();
    let texts = account::texts(&inner).unwrap();
    assert!(!texts.is_empty(), "the account space holds records");
    for text in texts {
        assert!(!text.contains(&hex), "a record holds the account key");
        assert!(
            !text.contains(&data_encoding::BASE64URL_NOPAD.encode(&secret)),
            "a record holds the account key"
        );
    }
}
