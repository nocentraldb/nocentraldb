//! Who may write content into a space (RFC 0008, "Roles and rights"): a
//! reader's write is refused with `NotAllowed` and a removed member's with
//! `NotMember` (RFC 0009, "Errors"), from the membership the member's copy
//! folded, before any change is made.

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so these tests run on both targets (NOC-175).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ed25519_dalek::SigningKey;

use super::Role;
use super::tests::{copy, joined, now, remove};
use crate::db::Db;
use crate::error::{Error, ErrorCode};
use crate::space::Space;
use crate::value::Value;

/// The number of changes the member's copy holds of `space`.
fn changes(db: &Db, space: &Space) -> usize {
    let inner = db.lock();
    let log = inner.view.log(space.id().0).expect("the space's log");
    log.after(ncdb_core::log::Position::START).count()
}

/// A write of one task into `space`.
fn write_a_task(space: &Space) -> Result<crate::space::Written, Error> {
    now(space.write(|tx| {
        tx.create("tasks", [("title", Value::from("a task"))])?;
        Ok(())
    }))
}

#[test]
fn a_reader_may_not_write() {
    let account = SigningKey::from_bytes(&[9; 32]);
    let (_, project, member, _) = joined(&account, Role::Reader);
    let on_member = now(member.spaces().get(project.id())).unwrap();
    let before = changes(&member, &project);

    let refused = write_a_task(&on_member).unwrap_err();

    assert_eq!(refused.code(), ErrorCode::NotAllowed);
    assert_eq!(changes(&member, &project), before);
}

#[test]
fn a_writer_and_an_admin_may_write() {
    let account = SigningKey::from_bytes(&[9; 32]);
    for role in [Role::Writer, Role::Admin] {
        let (_, project, member, _) = joined(&account, role);
        let on_member = now(member.spaces().get(project.id())).unwrap();

        write_a_task(&on_member).unwrap();
    }
}

#[test]
fn a_member_whose_copy_folded_its_removal_may_not_write() {
    let account = SigningKey::from_bytes(&[9; 32]);
    let (admin, project, member, invitation) = joined(&account, Role::Writer);
    let on_member = now(member.spaces().get(project.id())).unwrap();
    remove(&admin, &project, &invitation);
    copy(&admin, &member, &project);
    let before = changes(&member, &project);

    let refused = write_a_task(&on_member).unwrap_err();

    assert_eq!(refused.code(), ErrorCode::NotMember);
    assert_eq!(changes(&member, &project), before);
}
