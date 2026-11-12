//! A provisional erasure in a space with one account (RFC 0008, "A
//! provisional erasure in a space with one account"; RFC 0009, "Sync and
//! status": `provisionalErasures`; Carsten, 2026-11-11, NOC-230): every
//! member of the space belongs to one account, so the performer's own
//! account signs the shred off and it is provisional. Every device of the
//! account sees it in the space status, with its age on this device, its
//! performer and the notice `ErasurePending`; only an admin of the account
//! may take it back. Members keep the person key while it is provisional.

use std::time::Duration;

use ncdb_core::membership::{self, ControlBody, Entry};

use super::person_keys::{PERSON, acknowledgements, held, person_key, shared, stored};
use super::support::*;
use crate::db::{Db, lock};
use crate::members::{MemberId, NoticeCode};
use crate::space::Space;

/// `admin` erases `PERSON` in `space`, signed off by its own account,
/// folds the shred and wakes syncing. Returns the shred's change id.
pub(super) fn erase(admin: &Db, space: &Space) -> crate::ChangeId {
    erase_key(admin, space, PERSON)
}

/// [`erase`] of the person key `person`.
pub(super) fn erase_key(admin: &Db, space: &Space, person: [u8; 16]) -> crate::ChangeId {
    let id = space.id().0;
    let mut inner = lock(&admin.inner);
    let member = inner.member.key();
    let crate::db::Inner { view, spaces, .. } = &mut *inner;
    let state = spaces.get_mut(&id).expect("the admin holds the space");
    let next = crate::next::next(view, id, &state.checks, member.id());
    let generation = next.generation.expect("a generation");
    let entry = Entry::Shred(membership::ShredEntry {
        person_key: person,
        sign_off: membership::SignOff::sign(&account(), id, person, member.id()),
    });
    let change = membership::write(
        &member,
        id,
        next.sequence,
        generation,
        &ControlBody::new(crate::members::control_heads(view, id), entry),
    );
    crate::space::fold(view, &mut state.checks, &change).unwrap();
    crate::sync::wake(&inner);
    crate::ChangeId(change.id())
}

#[tokio::test(flavor = "multi_thread")]
async fn every_device_of_the_account_sees_the_provisional_erasure() {
    let relay = LocalRelay::start().await;
    let (admin, member, project) = shared(&relay).await;
    let key = ncdb_core::keys::PersonKey::generate();
    person_key(&admin, &project, &key, &key);
    synced(&admin).await;
    let space = project.id().0;
    stored(&relay, space).await;
    relay.proxy.cut();
    relay.proxy.restore();
    eventually("the member holds the person key", || {
        held(&member, space).is_some()
    })
    .await;

    let shred = erase(&admin, &project);
    synced(&admin).await;

    let mine = project.status().current().provisional_erasures;
    assert_eq!(mine.len(), 1, "the admin sees it");
    let erasure = &mine[0];
    assert_eq!(erasure.change, shred);
    assert!(erasure.age < Duration::from_secs(60));
    assert_eq!(
        erasure.performer,
        MemberId(lock(&admin.inner).member.key().id())
    );
    assert_eq!(erasure.notice.code, NoticeCode::ErasurePending);
    assert!(erasure.can_undo, "an admin of the performer's account");

    let on_member = member.spaces().get(project.id()).await.unwrap();
    eventually("the member sees it", || {
        on_member.status().current().provisional_erasures.len() == 1
    })
    .await;
    let theirs = &on_member.status().current().provisional_erasures[0];
    assert_eq!(theirs.change, shred);
    assert!(!theirs.can_undo, "a writer may not take it back");
    assert!(
        held(&member, space).is_some(),
        "a provisional shred keeps the person key"
    );
    assert!(!relay.person_keys(space).await.is_empty(), "and the record");
}

#[tokio::test(flavor = "multi_thread")]
async fn no_erasure_no_entry() {
    let relay = LocalRelay::start().await;
    let (_admin, _member, project) = shared(&relay).await;
    assert!(project.status().current().provisional_erasures.is_empty());
}

/// How far the relay's clock of the finalize test is ahead of the real one.
static RELAY_AHEAD: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn relay_now() -> u64 {
    crate::clock::now_millis() + RELAY_AHEAD.load(std::sync::atomic::Ordering::SeqCst)
}

const SEVEN_DAYS: u64 = 7 * 24 * 60 * 60 * 1000;

/// An admin of the performer's account writes the finalize once its device
/// held the shred for 7 days, here at once (RFC 0008, "Who writes the
/// finalize"). The relay refuses it as `too_early` until 7 days by its own
/// clock passed: the SDK keeps it unconfirmed, never refused, and pushes it
/// again later; once the relay accepts it, the member destroys the key and
/// the relay deletes the record (pm, NOC-234).
#[tokio::test(flavor = "multi_thread")]
async fn the_admin_finalizes_and_retries_a_too_early_finalize() {
    let relay = LocalRelay::start_with_clock(relay_now).await;
    let (admin, member, project) = shared(&relay).await;
    let key = ncdb_core::keys::PersonKey::generate();
    person_key(&admin, &project, &key, &key);
    synced(&admin).await;
    let space = project.id().0;
    stored(&relay, space).await;
    relay.proxy.cut();
    relay.proxy.restore();
    eventually("the member holds the person key", || {
        held(&member, space).is_some()
    })
    .await;
    crate::sync::set_finalize_after(&admin, Duration::ZERO, Duration::from_millis(200));

    let shred = erase(&admin, &project).0;
    eventually("the admin writes the finalize", || {
        let inner = lock(&admin.inner);
        let key = inner.spaces[&space].checks.resolved().person_key(PERSON);
        key.is_some_and(|key| key.finalized.is_some())
    })
    .await;
    tokio::time::sleep(Duration::from_secs(1)).await;
    let refused = project.status().current().refused;
    assert!(refused.is_empty(), "never refused: {refused:?}");
    assert!(admin.sync().current().unconfirmed > 0, "kept unconfirmed");
    assert!(
        held(&member, space).is_some(),
        "the relay has no finalize yet"
    );

    RELAY_AHEAD.store(SEVEN_DAYS, std::sync::atomic::Ordering::SeqCst);
    eventually("the relay takes the finalize", || {
        admin.sync().current().unconfirmed == 0
    })
    .await;
    eventually("the member destroys the key", || {
        held(&member, space).is_none()
    })
    .await;
    // The admin folded its own finalize, so it acknowledges the shred as
    // deleted too (RFC 0008, "Acknowledgement"; NOC-238).
    eventually("the admin acknowledges seen and deleted", || {
        acknowledgements(&admin, space).contains(&(shred, true))
    })
    .await;
    assert!(
        relay.person_keys(space).await.is_empty(),
        "the record is gone"
    );
    assert!(project.status().current().provisional_erasures.is_empty());
}
