//! The bridge's membership calls (NOC-176, slice 4b; RFC 0009,
//! "Invitations and members"), as the module docs of [`super`] list them.
//! A member id travels as its 64 lowercase hex digits (Carsten, 2026-11-13,
//! NOC-298).

use std::str::FromStr;

use ncdb_core::cbor::Value;
use ncdb_core::membership::MemberKind as CoreKind;

use super::{Failed, entries, get, invalid, lower_camel, role_name, space_id, text};
use crate::db::Db;
use crate::members::{AddMember, Invitation, MemberId, MemberKind, Notice, Role};

/// `invitation` `{}`: this member's invitation, `{"text": text,
/// "fingerprint": text}`.
pub(super) async fn invitation(db: &Db) -> Result<Value, Failed> {
    let invitation = db.invitation().await;
    Ok(entries([
        ("text", Value::Text(invitation.to_string())),
        (
            "fingerprint",
            Value::Text(invitation.fingerprint().to_string()),
        ),
    ]))
}

/// `members.add` `{"space": id, "invitations": [text], "role": text?,
/// "fingerprint": text?}`: adds each invitation's member, in order, and
/// answers `{"notice": notice}`. The fingerprint confirms an account new to
/// this device when its digits are the invitation's, however they are
/// spaced; any other confirms nothing. Without a role, an agent joins as a
/// writer, and a device is refused with `invalidValue`.
pub(super) async fn add(db: &Db, args: &Value) -> Result<Value, Failed> {
    let space = db.spaces().get(space_id(args)?).await?;
    let Some(Value::Array(texts)) = get(args, "invitations") else {
        return Err(invalid("members.add takes invitations"));
    };
    if texts.is_empty() {
        return Err(invalid("members.add takes at least one invitation"));
    }
    let role = match get(args, "role") {
        None | Some(Value::Null) => None,
        Some(Value::Text(name)) => Some(role_of(name)?),
        Some(_) => return Err(invalid("a role is text")),
    };
    let typed = match get(args, "fingerprint") {
        Some(Value::Text(typed)) => Some(digits(typed)),
        _ => None,
    };
    let mut notice = None;
    for value in texts {
        let Value::Text(text) = value else {
            return Err(invalid("an invitation is text"));
        };
        let invitation = Invitation::from_str(text)?;
        let shown = invitation.fingerprint();
        let fingerprint = typed
            .as_ref()
            .filter(|typed| **typed == digits(&shown.to_string()))
            .map(|_| shown);
        let role = match role {
            Some(role) => role,
            None if is_agent(&invitation) => Role::Writer,
            None => return Err(invalid("a device joins with a role")),
        };
        notice = Some(
            space
                .members()
                .add(&invitation, AddMember { role, fingerprint })
                .await?,
        );
    }
    Ok(entries([(
        "notice",
        notice_of(notice.expect("one invitation at least")),
    )]))
}

/// `members.list` `{"space": id}`: `[{"id": member, "account":
/// fingerprint, "linkedTo": fingerprint | null, "kind": "device" | "agent"
/// | "subAgent", "role": text | null, "self": bool}]`.
pub(super) async fn list(db: &Db, args: &Value) -> Result<Value, Failed> {
    let space = db.spaces().get(space_id(args)?).await?;
    let members = space.members().list().await?;
    Ok(Value::Array(
        members
            .into_iter()
            .map(|member| {
                let kind = match member.kind {
                    MemberKind::Device => "device",
                    MemberKind::Agent => "agent",
                    MemberKind::SubAgent => "subAgent",
                };
                entries([
                    ("id", Value::Text(member.id.to_string())),
                    ("account", Value::Text(member.account.to_string())),
                    (
                        "linkedTo",
                        member
                            .linked_to
                            .map_or(Value::Null, |into| Value::Text(into.to_string())),
                    ),
                    ("kind", Value::Text(kind.into())),
                    (
                        "role",
                        member
                            .role
                            .map_or(Value::Null, |role| Value::Text(role_name(role).into())),
                    ),
                    ("self", Value::Bool(member.is_self)),
                ])
            })
            .collect(),
    ))
}

/// `members.remove` `{"space": id, "member": member}`: answers
/// `{"notice": notice}`.
pub(super) async fn remove(db: &Db, args: &Value) -> Result<Value, Failed> {
    let space = db.spaces().get(space_id(args)?).await?;
    let notice = space.members().remove(member_id(args)?).await?;
    Ok(entries([("notice", notice_of(notice))]))
}

/// `members.setRole` `{"space": id, "member": member, "role": text}`:
/// answers `{"notice": notice}`.
pub(super) async fn set_role(db: &Db, args: &Value) -> Result<Value, Failed> {
    let space = db.spaces().get(space_id(args)?).await?;
    let role = role_of(text(args, "role")?)?;
    let notice = space.members().set_role(member_id(args)?, role).await?;
    Ok(entries([("notice", notice_of(notice))]))
}

/// A notice as the bridge sends it: `{"code": text, "text": text}`.
fn notice_of(notice: Notice) -> Value {
    entries([
        (
            "code",
            Value::Text(lower_camel(&format!("{:?}", notice.code))),
        ),
        ("text", Value::Text(notice.text.into())),
    ])
}

fn role_of(name: &str) -> Result<Role, Failed> {
    match name {
        "reader" => Ok(Role::Reader),
        "writer" => Ok(Role::Writer),
        "admin" => Ok(Role::Admin),
        _ => Err(invalid("a role is reader, writer or admin")),
    }
}

/// Whether the invitation's certificate makes its member an agent.
fn is_agent(invitation: &Invitation) -> bool {
    invitation
        .certificate()
        .is_some_and(|certificate| certificate.kind == CoreKind::Agent)
}

/// The digits of a fingerprint as a person types it, blocks or not.
fn digits(text: &str) -> String {
    text.chars().filter(char::is_ascii_digit).collect()
}

/// The member a call names by its 64 lowercase hex digits.
fn member_id(args: &Value) -> Result<MemberId, Failed> {
    MemberId::from_str(text(args, "member")?)
        .map_err(|_| invalid("a member id is 64 lowercase hex digits"))
}
