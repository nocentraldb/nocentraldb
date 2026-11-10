//! Every space of the member (RFC 0009, "Spaces"): `db.spaces.list()`
//! returns each space the copy holds, with the member's role and whether
//! the space is of the lock's model.

mod support;

use nocentraldb::{Role, SpaceInfo, SpaceModel};
use support::{now, open};

#[test]
fn a_database_without_spaces_lists_none() {
    let db = open();

    assert_eq!(now(db.spaces().list()).unwrap(), Vec::new());
}

#[test]
fn every_space_the_member_created_is_listed_with_the_admin_role_and_our_model() {
    let db = open();
    let first = now(db.spaces().create()).unwrap();
    let second = now(db.spaces().create()).unwrap();

    let mut listed = now(db.spaces().list()).unwrap();

    listed.sort_by_key(|info| info.id);
    let mut expected: Vec<SpaceInfo> = [first.id(), second.id()]
        .into_iter()
        .map(|id| SpaceInfo {
            id,
            role: Some(Role::Admin),
            model: SpaceModel::Ours,
        })
        .collect();
    expected.sort_by_key(|info| info.id);
    assert_eq!(listed, expected);
}
