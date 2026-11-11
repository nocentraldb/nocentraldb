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
