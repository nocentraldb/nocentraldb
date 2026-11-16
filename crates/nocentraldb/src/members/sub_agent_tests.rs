//! A sub-agent's identity (NOC-318; RFC 0003, "Sub-agents get an ad-hoc
//! space"; RFC 0009, "Sub-agents and ad-hoc spaces"): a database opened with
//! `OpenOptions::sub_agent` invites with a version 3 invitation, holds no
//! account and creates no space; only an agent adds it, signing its
//! certificate with its own key (Carsten, 2026-11-15, NOC-320 Q1, Q2, Q11).

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ed25519_dalek::SigningKey;

use super::tests::{database, now, of_account, space, writer};
use super::{MemberKind, NoticeCode};
use crate::db::{Db, OpenOptions};
use crate::error::ErrorCode;

const LOCK: &str = include_str!("../../tests/model/model.lock.json");

/// A sub-agent's database in memory.
fn sub_agent() -> Db {
    now(Db::open(OpenOptions::new(LOCK).in_memory().sub_agent())).expect("the test lock opens")
}

/// An agent's database in memory, of `account`.
fn agent(who: u8, account: &SigningKey) -> Db {
    let options = OpenOptions::new(LOCK).in_memory().agent();
    now(Db::open_as(
        options,
        [who; 32],
        [who ^ 0x80; 32],
        account.clone(),
    ))
    .expect("the test lock opens")
}

#[test]
fn a_sub_agent_invites_with_version_3_and_no_certificate() {
    let invitation = now(sub_agent().invitation());
    assert!(invitation.certificate().is_none());
    assert!(matches!(invitation.form, super::invitation::Form::SubAgent));
    assert!(invitation.to_string().starts_with("ncdb-invite-1:A"));
}

#[test]
fn a_sub_agent_creates_no_space() {
    let refused = now(sub_agent().spaces().create()).unwrap_err();
    assert_eq!(refused.code(), ErrorCode::NotAllowed);
}

#[test]
fn a_sub_agent_is_neither_an_agent_nor_paired_nor_recovered() {
    for (options, why) in [
        (OpenOptions::new(LOCK).in_memory().sub_agent().agent(), "with agent()"),
        (OpenOptions::new(LOCK).in_memory().sub_agent().pairing(), "with pairing()"),
        (
            OpenOptions::new(LOCK)
                .in_memory()
                .sub_agent()
                .recover("abandon ability able about above absent absorb abstract absurd abuse access accident"),
            "with recover()",
        ),
    ] {
        let Err(refused) = now(Db::open(options)) else {
            panic!("opened {why}");
        };
        assert_eq!(refused.code(), ErrorCode::InvalidValue, "{why}");
    }
}

#[test]
fn a_device_does_not_add_a_sub_agent() {
    let device = database();
    let project = space(&device);
    let invitation = now(sub_agent().invitation());
    let refused = now(project.members().add(&invitation, writer())).unwrap_err();
    assert_eq!(refused.code(), ErrorCode::NotAllowed);
}

#[test]
fn an_agent_adds_a_sub_agent_and_signs_its_certificate() {
    let account = SigningKey::from_bytes(&[0x5a; 32]);
    let parent = agent(7, &account);
    let project = space(&parent);
    let child = sub_agent();
    let invitation = now(child.invitation());

    let notice = now(project.members().add(&invitation, writer())).unwrap();

    assert_eq!(notice.code, NoticeCode::MemberAdded);
    let members = now(project.members().list()).unwrap();
    let added = members
        .iter()
        .find(|m| m.id.0 == invitation.key.id())
        .expect("the sub-agent is a member");
    let me = members.iter().find(|m| m.is_self).unwrap();
    assert_eq!(added.kind, MemberKind::SubAgent);
    assert_eq!(added.role, Some(super::Role::Writer));
    // The sub-agent is of its parent's account.
    assert_eq!(added.account, me.account);
    // Another device of the same account adds no sub-agent either.
    let device = of_account(8, &account);
    let theirs = space(&device);
    let refused = now(theirs.members().add(&invitation, writer())).unwrap_err();
    assert_eq!(refused.code(), ErrorCode::NotAllowed);
}
