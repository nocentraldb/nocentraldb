//! Pairing a device, with the account's spaces (RFC 0010, "Adding a device
//! or an agent to an account": "Which spaces a new device joins"; RFC 0009,
//! "Accounts and devices"; NOC-267): the device of the account that pairs
//! a new device adds it to every space where it is an admin, the account
//! space too, with one `DeviceAdded` per app space, and lists the spaces
//! whose admins the app must ask. The new device then reads them from the
//! relay.

use super::support::*;
use crate::db::{Db, OpenOptions};
use crate::members::{AddMember, NoticeCode, Role};
use crate::pairing::PairKind;
use crate::value::Value;

/// The laptop, the account's first device.
const LAPTOP: Who = 1;
/// A member of another account.
const OTHER: Who = 3;

/// A new database waiting to be paired, syncing with the relay at `url`.
async fn waiting(url: &str) -> Db {
    Db::open(OpenOptions::new(LOCK).in_memory().relay(url).pairing())
        .await
        .expect("the test lock opens")
}

#[tokio::test(flavor = "multi_thread")]
async fn a_paired_phone_reads_the_laptops_spaces() {
    let relay = LocalRelay::start().await;
    let laptop = open(LAPTOP, &relay.url()).await;
    let project = now(laptop.spaces().create()).unwrap();
    let task = now(project.write(|tx| {
        tx.create("tasks", [("title", Value::from("Buy milk"))])?;
        Ok(())
    }))
    .unwrap()
    .ids[0];

    let phone = waiting(&relay.url()).await;
    let invitation = now(phone.invitation());
    let paired = now(laptop.account().pair(PairKind::Device, &invitation)).unwrap();
    let added: Vec<_> = paired.added.iter().map(|(space, _)| *space).collect();
    assert_eq!(
        added,
        [project.id()],
        "the app space, not the account space"
    );
    assert!(
        paired
            .added
            .iter()
            .all(|(_, notice)| notice.code == NoticeCode::DeviceAdded)
    );
    assert!(paired.ask_admins.is_empty());
    let notices = now(phone.account().complete_pairing(&paired.answer)).unwrap();
    assert_eq!(notices[0].code, NoticeCode::DevicePaired);
    synced(&laptop).await;

    eventually("the phone reads the laptop's task", || {
        now(phone.spaces().get(project.id()))
            .is_ok_and(|space| now(space.get("tasks", task)).is_ok_and(|read| read.is_some()))
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_space_where_no_device_of_the_account_is_an_admin_is_listed_to_ask() {
    let theirs_account = ed25519_dalek::SigningKey::from_bytes(&[0x33; 32]);
    let relay = LocalRelay::with(std::collections::BTreeSet::from([
        account().verifying_key().to_bytes(),
        theirs_account.verifying_key().to_bytes(),
    ]))
    .await;
    let laptop = open(LAPTOP, &relay.url()).await;
    let own = now(laptop.spaces().create()).unwrap();
    // A space of another account, where the laptop is a writer.
    let other = Db::open_as(
        OpenOptions::new(LOCK).in_memory().relay(&relay.url()),
        [OTHER; 32],
        [OTHER ^ 0x80; 32],
        theirs_account,
    )
    .await
    .unwrap();
    let theirs = now(other.spaces().create()).unwrap();
    let invitation = now(laptop.invitation());
    let options = AddMember {
        role: Role::Writer,
        fingerprint: Some(invitation.fingerprint()),
    };
    now(theirs.members().add(&invitation, options)).unwrap();
    synced(&other).await;
    eventually("the laptop joined their space", || {
        now(laptop.spaces().get(theirs.id())).is_ok()
    })
    .await;

    let phone = waiting(&relay.url()).await;
    let paired = now(laptop
        .account()
        .pair(PairKind::Device, &now(phone.invitation())))
    .unwrap();
    let added: Vec<_> = paired.added.iter().map(|(space, _)| *space).collect();
    assert_eq!(added, [own.id()]);
    assert_eq!(paired.ask_admins, [theirs.id()]);
}

#[tokio::test(flavor = "multi_thread")]
async fn an_agent_joins_no_space_when_it_is_paired() {
    let relay = LocalRelay::start().await;
    let laptop = open(LAPTOP, &relay.url()).await;
    now(laptop.spaces().create()).unwrap();
    let agent = Db::open(
        OpenOptions::new(LOCK)
            .in_memory()
            .relay(&relay.url())
            .pairing()
            .agent(),
    )
    .await
    .unwrap();
    let paired = now(laptop
        .account()
        .pair(PairKind::Agent, &now(agent.invitation())))
    .unwrap();
    assert!(paired.added.is_empty() && paired.ask_admins.is_empty());
}

/// A device with an account of its own.
const TABLET: Who = 4;

#[tokio::test(flavor = "multi_thread")]
async fn a_tablet_with_an_account_of_its_own_is_linked_and_its_link_reaches_the_account_space() {
    let tablet_account = ed25519_dalek::SigningKey::from_bytes(&[0x44; 32]);
    let old_key = tablet_account.verifying_key().to_bytes();
    let relay = LocalRelay::with(std::collections::BTreeSet::from([
        account().verifying_key().to_bytes(),
        old_key,
    ]))
    .await;
    let laptop = open(LAPTOP, &relay.url()).await;
    let project = now(laptop.spaces().create()).unwrap();
    let tablet = Db::open_as(
        OpenOptions::new(LOCK).in_memory().relay(&relay.url()),
        [TABLET; 32],
        [TABLET ^ 0x80; 32],
        tablet_account,
    )
    .await
    .unwrap();
    let before = now(tablet.invitation()).fingerprint();

    let paired = now(laptop
        .account()
        .pair(PairKind::Device, &now(tablet.invitation())))
    .unwrap();
    let added: Vec<_> = paired.added.iter().map(|(space, _)| *space).collect();
    assert_eq!(added, [project.id()]);
    let notices: Vec<_> = now(tablet.account().complete_pairing(&paired.answer))
        .unwrap()
        .into_iter()
        .map(|notice| notice.code)
        .collect();
    assert_eq!(
        notices,
        [NoticeCode::DevicePaired, NoticeCode::AccountLinked]
    );
    let after = now(tablet.invitation()).fingerprint();
    assert_eq!(after, now(laptop.invitation()).fingerprint());
    assert_ne!(after, before);
    synced(&laptop).await;

    // The tablet writes the whole link statement into the account space,
    // which the laptop reads.
    eventually("the laptop's account space holds the tablet's link", || {
        crate::account::links(&crate::db::lock(&laptop.inner))
            .unwrap()
            .iter()
            .any(|link| link.old_account_key == old_key && link.verify())
    })
    .await;
    // Once the relay holds it, the tablet forgets its pending row; the
    // next fold, here of the laptop's task, reaches the account space.
    now(project.write(|tx| {
        tx.create("tasks", [("title", Value::from("Linked"))])?;
        Ok(())
    }))
    .unwrap();
    synced(&laptop).await;
    eventually(
        "the tablet forgets the link once the relay holds it",
        || {
            crate::db::lock(&tablet.inner)
                .view
                .pending_links()
                .unwrap()
                .is_empty()
        },
    )
    .await;
}

/// A guest: a member of another account's space, whose own account the
/// relay does not let create spaces.
const GUEST: Who = 5;

#[tokio::test(flavor = "multi_thread")]
async fn a_guest_that_pairs_a_device_creates_the_account_space_the_relay_admits() {
    // Only the owner's account may create spaces on this relay; the guest's
    // may create its account space once it is a member of an admitted space
    // (NOC-283, #288).
    let guest_account = ed25519_dalek::SigningKey::from_bytes(&[0x55; 32]);
    let relay = LocalRelay::start().await;
    let owner = open(LAPTOP, &relay.url()).await;
    let shared = now(owner.spaces().create()).unwrap();
    let guest = Db::open_as(
        OpenOptions::new(LOCK).in_memory().relay(&relay.url()),
        [GUEST; 32],
        [GUEST ^ 0x80; 32],
        guest_account,
    )
    .await
    .unwrap();
    let invitation = now(guest.invitation());
    let options = AddMember {
        role: Role::Writer,
        fingerprint: Some(invitation.fingerprint()),
    };
    now(shared.members().add(&invitation, options)).unwrap();
    synced(&owner).await;
    eventually("the guest joined the owner's space", || {
        now(guest.spaces().get(shared.id())).is_ok()
    })
    .await;
    assert!(crate::account::space(&crate::db::lock(&guest.inner)).is_none());

    let phone = waiting(&relay.url()).await;
    let paired = now(guest
        .account()
        .pair(PairKind::Device, &now(phone.invitation())))
    .unwrap();
    assert!(
        paired.added.is_empty(),
        "the guest administers no app space"
    );
    assert_eq!(paired.ask_admins, [shared.id()]);
    let account_space = crate::account::space(&crate::db::lock(&guest.inner))
        .expect("pairing created the account space");
    now(phone.account().complete_pairing(&paired.answer)).unwrap();
    synced(&guest).await;

    eventually(
        "the phone holds the account space the relay admitted",
        || crate::account::space(&crate::db::lock(&phone.inner)) == Some(account_space),
    )
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_space_where_another_device_of_the_account_is_an_admin_is_in_neither_list() {
    // The account space's `memberships` records carry each device's role
    // (RFC 0010, "The account space"), so the laptop knows the phone is an
    // admin of the phone's space although the laptop does not hold it: the
    // phone adds the new device there, the app asks no one.
    let relay = LocalRelay::start().await;
    let laptop = open(LAPTOP, &relay.url()).await;
    now(laptop.spaces().create()).unwrap();
    let phone = waiting(&relay.url()).await;
    let paired = now(laptop
        .account()
        .pair(PairKind::Device, &now(phone.invitation())))
    .unwrap();
    now(phone.account().complete_pairing(&paired.answer)).unwrap();
    synced(&laptop).await;
    eventually("the phone holds the account space", || {
        crate::account::space(&crate::db::lock(&phone.inner)).is_some()
    })
    .await;
    let phones = now(phone.spaces().create()).unwrap();
    synced(&phone).await;
    eventually("the laptop reads the phone's membership record", || {
        crate::account::memberships(&crate::db::lock(&laptop.inner))
            .unwrap()
            .iter()
            .any(|record| ncdb_core::ids::SpaceId(record.space) == phones.id().0)
    })
    .await;
    assert!(now(laptop.spaces().get(phones.id())).is_err());

    let tablet = waiting(&relay.url()).await;
    let paired = now(laptop
        .account()
        .pair(PairKind::Device, &now(tablet.invitation())))
    .unwrap();
    assert!(paired.added.iter().all(|(space, _)| *space != phones.id()));
    assert!(
        paired.ask_admins.is_empty(),
        "the phone is the admin of its space: {:?}",
        paired.ask_admins
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_tablet_that_created_a_space_before_the_link_writes_its_link_into_the_persons_account_space()
 {
    // NOC-310: a database with an account of its own that created a space
    // holds an account space under that old account. After the link, the
    // account space it writes to is the person's, whose genesis creator is
    // its current account, not the old one.
    let tablet_account = ed25519_dalek::SigningKey::from_bytes(&[0x47; 32]);
    let old_key = tablet_account.verifying_key().to_bytes();
    let relay = LocalRelay::with(std::collections::BTreeSet::from([
        account().verifying_key().to_bytes(),
        tablet_account.verifying_key().to_bytes(),
    ]))
    .await;
    let laptop = open(LAPTOP, &relay.url()).await;
    now(laptop.spaces().create()).unwrap();
    let tablet = Db::open_as(
        OpenOptions::new(LOCK).in_memory().relay(&relay.url()),
        [TABLET; 32],
        [TABLET ^ 0x80; 32],
        tablet_account,
    )
    .await
    .unwrap();
    now(tablet.spaces().create()).unwrap();
    let old_account_space =
        crate::account::space(&crate::db::lock(&tablet.inner)).expect("an old account space");

    let paired = now(laptop
        .account()
        .pair(PairKind::Device, &now(tablet.invitation())))
    .unwrap();
    now(tablet.account().complete_pairing(&paired.answer)).unwrap();
    synced(&laptop).await;

    eventually("the laptop's account space holds the tablet's link", || {
        crate::account::links(&crate::db::lock(&laptop.inner))
            .unwrap()
            .iter()
            .any(|link| link.old_account_key == old_key && link.verify())
    })
    .await;
    // The tablet writes to the person's account space now; its old one
    // stays as it was.
    let tablet_inner = crate::db::lock(&tablet.inner);
    assert_eq!(
        crate::account::space(&tablet_inner),
        crate::account::space(&crate::db::lock(&laptop.inner))
    );
    assert!(tablet_inner.spaces.contains_key(&old_account_space));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_paired_phone_reopened_before_its_first_sync_holds_only_the_accounts_account_space() {
    // NOC-311, with the owner's NOC-312 C3: the phone remembers beside its
    // log that it joined the account, so after a reopen it creates no
    // account space of its own; once it syncs it holds the account's one,
    // and forgets that it is waiting for it.
    let relay = LocalRelay::start().await;
    let laptop = open(LAPTOP, &relay.url()).await;
    now(laptop.spaces().create()).unwrap();
    let dir = TempDir::new();
    let at = |pairing: bool| {
        let options = OpenOptions::new(LOCK)
            .path(dir.0.join("data"))
            .key_file(dir.0.join("phone.keys"));
        if pairing { options.pairing() } else { options }
    };
    // Opened for pairing, but with no relay yet, so nothing syncs.
    let phone = Db::open(at(true)).await.unwrap();
    let paired = now(laptop
        .account()
        .pair(PairKind::Device, &now(phone.invitation())))
    .unwrap();
    now(phone.account().complete_pairing(&paired.answer)).unwrap();
    drop(phone);
    synced(&laptop).await;

    // Reopened, still offline, it creates a space and no account space.
    let phone = Db::open(at(false)).await.unwrap();
    now(phone.spaces().create()).unwrap();
    assert!(crate::account::space(&crate::db::lock(&phone.inner)).is_none());
    drop(phone);

    // With the relay, it receives the account's account space and holds
    // only that one.
    let phone = Db::open(at(false).relay(&relay.url())).await.unwrap();
    let ours = crate::account::space(&crate::db::lock(&laptop.inner)).unwrap();
    eventually("the phone holds the account's account space", || {
        crate::account::space(&crate::db::lock(&phone.inner)) == Some(ours)
    })
    .await;
    let inner = crate::db::lock(&phone.inner);
    let shipped = crate::account::id();
    let held = inner
        .spaces
        .values()
        .filter(|state| state.checks.model_id() == Some(shipped))
        .count();
    assert_eq!(held, 1, "only the account's account space");
    assert!(!inner.view.joined().unwrap(), "no longer waiting for it");
}
