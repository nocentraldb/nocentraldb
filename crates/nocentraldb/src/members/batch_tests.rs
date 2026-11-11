//! The changes of one membership call are folded in one transaction
//! (NOC-204): a batch that cannot be received whole leaves neither the
//! log nor the space's checks changed, so nothing half-written survives.

use ed25519_dalek::SigningKey;
use ncdb_core::membership::{self, Action, ControlBody, Entry, MembershipEntry};

use super::Role;
use super::tests::{joined, now, of_account, writer};
use crate::db::Db;
use crate::space::Space;

/// The number of changes the admin's log of `space` holds.
fn changes(admin: &Db, space: &Space) -> usize {
    let inner = admin.lock();
    let log = inner.view.log(space.id().0).expect("the space's log");
    log.after(ncdb_core::log::Position::START).count()
}

#[test]
fn a_batch_that_is_not_received_whole_changes_nothing() {
    let account = SigningKey::from_bytes(&[9; 32]);
    let (admin, project, _, invitation) = joined(&account, Role::Writer);
    let before = changes(&admin, &project);
    let id = project.id().0;

    let refused = {
        let mut inner = admin.lock();
        let member = inner.member.key();
        let crate::db::Inner {
            view,
            spaces,
            member: secrets,
            ..
        } = &mut *inner;
        let state = spaces.get_mut(&id).expect("the admin holds the space");
        let next = crate::next::next(view, id, &state.checks, member.id());
        let removal = membership::write(
            &member,
            id,
            next.sequence,
            next.generation.expect("a generation"),
            &ControlBody::new(
                super::control_heads(view, id),
                Entry::Membership(MembershipEntry {
                    subject: invitation.key.id(),
                    action: Action::Remove { cutoff: 0 },
                }),
            ),
        );
        let frames: [&[u8]; 2] = [removal.bytes(), b"garbled"];
        super::batch::commit(view, state, secrets, id, &frames)
    };

    assert!(refused.is_err());
    assert_eq!(changes(&admin, &project), before);
    let role = admin.lock().spaces[&id]
        .checks
        .resolved()
        .role(invitation.key.id());
    assert_eq!(role, Some(Role::Writer.into()));
    let other = now(of_account(3, &account).invitation());
    now(project.members().add(&other, writer())).unwrap();
}
