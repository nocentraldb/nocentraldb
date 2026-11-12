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

use super::person_keys::{PERSON, fetched, held, person_key, shared};
use super::support::*;
use crate::db::{Db, lock};
use crate::members::{MemberId, NoticeCode};
use crate::space::Space;

/// `admin` erases `PERSON` in `space`, signed off by its own account,
/// folds the shred and wakes syncing. Returns the shred's change id.
pub(super) fn erase(admin: &Db, space: &Space) -> crate::ChangeId {
    let id = space.id().0;
    let mut inner = lock(&admin.inner);
    let member = inner.member.key();
    let crate::db::Inner { view, spaces, .. } = &mut *inner;
    let state = spaces.get_mut(&id).expect("the admin holds the space");
    let next = crate::next::next(view, id, &state.checks, member.id());
    let generation = next.generation.expect("a generation");
    let entry = Entry::Shred(membership::ShredEntry {
        person_key: PERSON,
        sign_off: membership::SignOff::sign(&account(), id, PERSON, member.id()),
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
    eventually("the member holds the person key", || {
        // A fetch that ran before the record was put finds nothing; a new
        // connection fetches again.
        if fetched(&member, space) && held(&member, space).is_none() {
            relay.proxy.cut();
            relay.proxy.restore();
        }
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
