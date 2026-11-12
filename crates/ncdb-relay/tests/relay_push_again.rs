//! RFC 0007, "Push": a push is idempotent. A member whose connection drops
//! before `pushed` arrives pushes the same frames again and gets
//! `duplicate` for the ones that made it, even when one of them ended its
//! membership, such as its own leave (NOC-276). That answer goes only to
//! the change's own author: a former member learns nothing of a change it
//! did not write.

mod support;

use ncdb_core::membership::Role;
use ncdb_relay::{ErrorCode, Pushed};
use support::space::Space;
use support::{push, relay};

#[test]
fn a_leave_pushed_again_after_it_was_appended_is_a_duplicate() {
    let (relay, _dir) = relay();
    let mut space = Space::new(0);
    let genesis = space.genesis.clone();
    let add = space.add(0, 1, Role::Writer, &[&genesis]);
    push(&relay, 0, space.space, &[&genesis, &add]);
    let cutoff = space.sequence(1);
    let leave = space.remove(1, 1, cutoff, &[&add]);
    assert_eq!(
        push(&relay, 1, space.space, &[&leave]),
        [Pushed::Accepted(3)]
    );

    // The answer was lost: the former member pushes its leave again.
    assert_eq!(
        push(&relay, 1, space.space, &[&leave]),
        [Pushed::Duplicate(3)]
    );
}

#[test]
fn a_former_member_s_new_change_is_still_refused() {
    let (relay, _dir) = relay();
    let mut space = Space::new(0);
    let genesis = space.genesis.clone();
    let add = space.add(0, 1, Role::Writer, &[&genesis]);
    push(&relay, 0, space.space, &[&genesis, &add]);
    let cutoff = space.sequence(1);
    let leave = space.remove(1, 1, cutoff, &[&add]);
    push(&relay, 1, space.space, &[&leave]);

    let after = space.sealed(1, Vec::new());
    assert_eq!(
        push(&relay, 1, space.space, &[&after]),
        [Pushed::Rejected(ErrorCode::NotMember)]
    );
}

#[test]
fn a_former_member_learns_nothing_of_a_change_it_did_not_write() {
    let (relay, _dir) = relay();
    let mut space = Space::new(0);
    let genesis = space.genesis.clone();
    let add = space.add(0, 1, Role::Writer, &[&genesis]);
    let by_admin = space.sealed(0, Vec::new());
    push(&relay, 0, space.space, &[&genesis, &add, &by_admin]);
    let cutoff = space.sequence(1);
    let leave = space.remove(1, 1, cutoff, &[&add]);
    push(&relay, 1, space.space, &[&leave]);

    // The admin's change is in the log, but its position is not the
    // former member's to learn.
    assert_eq!(
        push(&relay, 1, space.space, &[&by_admin]),
        [Pushed::Rejected(ErrorCode::NotMember)]
    );
}
