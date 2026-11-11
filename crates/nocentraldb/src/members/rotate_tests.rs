//! What a rotation of the SDK refuses (RFC 0003, "Rotating a space key",
//! step 3): it cannot re-wrap person-key records, which the SDK does not
//! hold, so a space with a person key is not rotated, and nothing is
//! written.

use ed25519_dalek::SigningKey;
use ncdb_core::membership::{self, ControlBody, Entry, PersonKeyEntry};

use super::Role;
use super::tests::{joined, now, remove, writer};
use crate::db::Db;
use crate::error::ErrorCode;
use crate::space::Space;

/// The admin announces a person key in `space`, through the core, as
/// another client could.
fn person_key(admin: &Db, space: &Space) {
    let mut inner = admin.lock();
    let member = inner.member.key();
    let id = space.id().0;
    let crate::db::Inner { view, spaces, .. } = &mut *inner;
    let state = spaces.get_mut(&id).expect("the admin holds the space");
    let next = crate::next::next(view, id, &state.checks, member.id());
    let generation = next.generation.expect("a generation");
    let entry = Entry::PersonKey(PersonKeyEntry {
        person_key: [0x0f; 16],
        commitment: [0xc0; 32],
        generation,
        record_hash: membership::record_hash(b"a wrapped record"),
    });
    let change = membership::write(
        &member,
        id,
        next.sequence,
        generation,
        &ControlBody::new(super::control_heads(view, id), entry),
    );
    crate::space::fold(view, &mut state.checks, &change).unwrap();
}

/// The number of changes the admin's log of `space` holds.
fn changes(admin: &Db, space: &Space) -> usize {
    let inner = admin.lock();
    let log = inner.view.log(space.id().0).expect("the space's log");
    log.after(ncdb_core::log::Position::START).count()
}

#[test]
fn a_former_member_of_a_space_with_a_person_key_is_not_added_again() {
    let account = SigningKey::from_bytes(&[9; 32]);
    let (admin, project, _, invitation) = joined(&account, Role::Writer);
    person_key(&admin, &project);
    remove(&admin, &project, &invitation);
    let before = changes(&admin, &project);

    let refused = now(project.members().add(&invitation, writer())).unwrap_err();

    assert_eq!(refused.code(), ErrorCode::InvalidValue);
    assert_eq!(changes(&admin, &project), before);
}

/// `db` rotates `space` on its own, through the SDK's rotation.
fn rotate_on(db: &Db, space: &Space) -> u64 {
    let mut inner = db.lock();
    let member = inner.member.key();
    let crate::db::Inner { view, spaces, .. } = &mut *inner;
    let state = spaces.get_mut(&space.id().0).expect("the database holds the space");
    super::rotate::rotate(view, state, &member, space.id().0, Default::default()).unwrap()
}

/// The heads of the generation graph in `db`'s copy of `space`.
fn heads(db: &Db, space: &Space) -> std::collections::BTreeSet<u64> {
    db.lock().spaces[&space.id().0].checks.resolved().heads()
}

/// Two admins rotate at once, so the generation graph has two heads; the
/// next rotation lists both as previous, as a merge rotation does (RFC
/// 0008, "Two removals that rotate the key at once", step 5), and leaves
/// one head.
#[test]
fn a_rotation_chains_every_head_of_the_generation_graph() {
    let account = SigningKey::from_bytes(&[9; 32]);
    let (admin, project, other, _) = joined(&account, Role::Admin);
    let guest = super::tests::of_account(3, &account);
    let invitation = now(guest.invitation());
    now(project.members().add(&invitation, writer())).unwrap();
    super::tests::copy(&admin, &other, &project);
    let ours = rotate_on(&admin, &project);
    let theirs = rotate_on(&other, &project);
    super::tests::copy(&other, &admin, &project);
    assert_eq!(heads(&admin, &project), [ours, theirs].into());
    remove(&admin, &project, &invitation);

    now(project.members().add(&invitation, writer())).unwrap();

    let after = heads(&admin, &project);
    assert_eq!(after.len(), 1, "{after:?}");
    let current = *after.first().unwrap();
    let inner = admin.lock();
    let resolved = inner.spaces[&project.id().0].checks.resolved();
    assert!(resolved.descends(current, ours) && resolved.descends(current, theirs));
}

/// Gives `to` the control changes of `space` that `from` holds and that
/// `keep` keeps, as a partial pull would.
fn copy_some(from: &Db, to: &Db, space: &Space, keep: impl Fn(&Entry) -> bool) {
    let changes: Vec<Vec<u8>> = {
        let inner = from.lock();
        let log = inner.view.log(space.id().0).expect("the space's log");
        log.after(ncdb_core::log::Position::START)
            .filter(|(_, change)| {
                ControlBody::decode(&change.header().payload)
                    .is_ok_and(|body| keep(&body.entry))
            })
            .map(|(_, change)| change.bytes().to_vec())
            .collect()
    };
    let mut inner = to.lock();
    let crate::db::Inner { view, spaces, .. } = &mut *inner;
    let state = spaces.get_mut(&space.id().0).expect("the database holds the space");
    for bytes in changes {
        view.receive(&bytes, &mut state.checks).expect("the change is received");
    }
}

/// An admin that holds no key of one head cannot chain it: the rotation
/// is refused before anything is written. The admin rotated twice, so its
/// own generation is current, and it holds only the other admin's key
/// chain entry, not its envelope.
#[test]
fn a_rotation_is_refused_when_the_key_of_a_head_is_not_held() {
    let account = SigningKey::from_bytes(&[9; 32]);
    let (admin, project, other, _) = joined(&account, Role::Admin);
    let guest = super::tests::of_account(3, &account);
    let invitation = now(guest.invitation());
    now(project.members().add(&invitation, writer())).unwrap();
    super::tests::copy(&admin, &other, &project);
    rotate_on(&other, &project);
    rotate_on(&admin, &project);
    rotate_on(&admin, &project);
    copy_some(&other, &admin, &project, |entry| matches!(entry, Entry::KeyChain(_)));
    assert_eq!(heads(&admin, &project).len(), 2);
    remove(&admin, &project, &invitation);
    let before = changes(&admin, &project);

    let refused = now(project.members().add(&invitation, writer())).unwrap_err();

    assert_eq!(refused.code(), ErrorCode::InvalidValue);
    assert_eq!(changes(&admin, &project), before);
}
