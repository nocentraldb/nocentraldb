//! The bridge's membership calls (NOC-176, slice 4b; RFC 0009,
//! "Invitations and members"): a member's invitation, and adding, listing,
//! re-roling and removing the members of a space. A member id is its 64
//! lowercase hex digits (Carsten, 2026-11-13, NOC-298).

use ncdb_core::cbor::Value;

use super::{Bridge, as_text, code, field, map, ok, opened, send, text};

/// The invitation of a second database: its text and its fingerprint.
fn invitation() -> (Bridge, Value, Value) {
    let mut other = opened();
    let answer = ok(&mut other, "invitation", map(&[]));
    let invitation = field(&answer, "text").expect("a text").clone();
    let fingerprint = field(&answer, "fingerprint")
        .expect("a fingerprint")
        .clone();
    (other, invitation, fingerprint)
}

fn notice_code(answer: &Value) -> &str {
    as_text(field(field(answer, "notice").expect("a notice"), "code").expect("a code"))
}

/// The members of `space`, as `members.list` answers them.
fn members(bridge: &mut Bridge, space: &Value) -> Vec<Value> {
    match ok(bridge, "members.list", map(&[("space", space.clone())])) {
        Value::Array(members) => members,
        other => panic!("not a list: {other:?}"),
    }
}

#[test]
fn an_invitation_is_its_text_and_its_accounts_fingerprint() {
    let (_other, invitation, fingerprint) = invitation();
    assert!(as_text(&invitation).starts_with("ncdb-invite-1:"));
    let digits: String = as_text(&fingerprint)
        .chars()
        .filter(char::is_ascii_digit)
        .collect();
    assert_eq!(digits.len(), 60, "{fingerprint:?}");
}

#[test]
fn a_new_space_lists_its_creator_as_its_admin() {
    let mut bridge = opened();
    let space = ok(&mut bridge, "spaces.create", map(&[]));
    let listed = members(&mut bridge, &space);
    assert_eq!(listed.len(), 1);
    let me = &listed[0];
    let id = as_text(field(me, "id").expect("an id"));
    assert!(
        id.len() == 64 && id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')),
        "{id}"
    );
    assert_eq!(field(me, "role"), Some(&text("admin")));
    assert_eq!(field(me, "kind"), Some(&text("device")));
    assert_eq!(field(me, "self"), Some(&Value::Bool(true)));
    assert!(field(me, "account").is_some());
    // An account linked into no other has no `linkedTo` (the owner's
    // NOC-307 A1).
    assert_eq!(field(me, "linkedTo"), Some(&Value::Null));
}

#[test]
fn an_account_new_to_this_device_is_added_once_a_person_confirmed_its_fingerprint() {
    let mut bridge = opened();
    let space = ok(&mut bridge, "spaces.create", map(&[]));
    let (_other, invitation, fingerprint) = invitation();

    let args = |fingerprint: Option<Value>| {
        let mut entries = vec![
            ("space", space.clone()),
            ("invitations", Value::Array(vec![invitation.clone()])),
            ("role", text("writer")),
        ];
        if let Some(fingerprint) = fingerprint {
            entries.push(("fingerprint", fingerprint));
        }
        map(&entries)
    };

    // Unconfirmed: refused, with the fingerprint for a person to compare.
    let answer = send(&mut bridge, 3, "members.add", args(None));
    let error = field(&answer, "error").expect("refused");
    assert_eq!(field(error, "code"), Some(&text("unknownAccount")));
    assert_eq!(field(error, "fingerprint"), Some(&fingerprint));

    // Another fingerprint confirms nothing.
    let wrong = text(&"1".repeat(60));
    assert_eq!(
        code(&mut bridge, "members.add", args(Some(wrong))),
        "unknownAccount"
    );

    // The digits a person compared, spaced or not.
    let digits: String = as_text(&fingerprint)
        .chars()
        .filter(char::is_ascii_digit)
        .collect();
    let added = ok(&mut bridge, "members.add", args(Some(text(&digits))));
    assert_eq!(notice_code(&added), "memberAdded");
    assert_eq!(members(&mut bridge, &space).len(), 2);
    assert_eq!(
        code(&mut bridge, "members.add", args(None)),
        "alreadyMember"
    );
}

#[test]
fn a_member_is_re_roled_and_removed_by_its_id() {
    let mut bridge = opened();
    let space = ok(&mut bridge, "spaces.create", map(&[]));
    let (_other, invitation, fingerprint) = invitation();
    ok(
        &mut bridge,
        "members.add",
        map(&[
            ("space", space.clone()),
            ("invitations", Value::Array(vec![invitation])),
            ("role", text("writer")),
            ("fingerprint", fingerprint),
        ]),
    );
    let them = members(&mut bridge, &space)
        .into_iter()
        .find(|member| field(member, "self") == Some(&Value::Bool(false)))
        .expect("the added member");
    let id = field(&them, "id").expect("an id").clone();

    let lowered = ok(
        &mut bridge,
        "members.setRole",
        map(&[
            ("space", space.clone()),
            ("member", id.clone()),
            ("role", text("reader")),
        ]),
    );
    assert_eq!(notice_code(&lowered), "roleSet");
    let role_of = |bridge: &mut Bridge| {
        members(bridge, &space)
            .into_iter()
            .find(|member| field(member, "id") == Some(&id))
            .and_then(|member| field(&member, "role").cloned())
    };
    assert_eq!(role_of(&mut bridge), Some(text("reader")));

    let removed = ok(
        &mut bridge,
        "members.remove",
        map(&[("space", space.clone()), ("member", id.clone())]),
    );
    assert_eq!(notice_code(&removed), "memberRemoved");
    assert_eq!(role_of(&mut bridge), None);
}

#[test]
fn a_member_id_that_is_not_one_is_refused() {
    let mut bridge = opened();
    let space = ok(&mut bridge, "spaces.create", map(&[]));
    for id in ["", "ABCD", &"g".repeat(64)] {
        assert_eq!(
            code(
                &mut bridge,
                "members.remove",
                map(&[("space", space.clone()), ("member", text(id))]),
            ),
            "invalidValue",
            "{id}"
        );
    }
}

#[test]
fn an_invitation_that_is_not_one_is_refused() {
    let mut bridge = opened();
    let space = ok(&mut bridge, "spaces.create", map(&[]));
    assert_eq!(
        code(
            &mut bridge,
            "members.add",
            map(&[
                ("space", space),
                (
                    "invitations",
                    Value::Array(vec![text("ncdb-invite-1:AAAA")])
                ),
                ("role", text("writer")),
            ]),
        ),
        "invalidValue"
    );
}
