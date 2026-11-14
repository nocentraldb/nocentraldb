//! The audit of a space's erasures through the bridge (NOC-315, slice 2;
//! RFC 0009, "Erasures"; the shapes the owner decided on NOC-246, Carsten,
//! 2026-11-12).

use ncdb_core::cbor::Value;

use super::{code, field, map, ok, opened, text};
use crate::erasures::{Acknowledged, Erasure, ErasureMember, ErasureState};
use crate::id::ChangeId;
use crate::members::{Fingerprint, MemberId};

#[test]
fn a_new_space_lists_no_erasures() {
    let mut bridge = opened();
    let space = ok(&mut bridge, "spaces.create", map(&[]));
    assert_eq!(
        ok(&mut bridge, "erasures.list", map(&[("space", space)])),
        Value::Array(vec![])
    );
}

#[test]
fn the_erasures_of_an_unknown_space_are_not_found() {
    let mut bridge = opened();
    assert_eq!(
        code(
            &mut bridge,
            "erasures.list",
            map(&[("space", text(&"ab".repeat(16)))])
        ),
        "notFound"
    );
}

#[test]
fn an_erasure_carries_rfc_0009s_names() {
    let performer = MemberId(ncdb_core::ids::MemberId([1; 32]));
    let other = MemberId(ncdb_core::ids::MemberId([2; 32]));
    let account = Fingerprint::account([3; 32]);
    let sign_off = Fingerprint::account([4; 32]);
    let shred = ChangeId(ncdb_core::ids::ChangeId([5; 32]));
    let erasure = Erasure {
        shred,
        state: ErasureState::TakenBack,
        performer,
        performer_account: account,
        sign_off,
        members: vec![
            ErasureMember {
                id: performer,
                account,
                current: true,
                acknowledged: Acknowledged::Deleted,
            },
            ErasureMember {
                id: other,
                account: sign_off,
                current: false,
                acknowledged: Acknowledged::Missing,
            },
        ],
    };
    let value = crate::bridge::erasures::erasure(&erasure);
    assert_eq!(field(&value, "shred"), Some(&text(&shred.to_string())));
    assert_eq!(field(&value, "state"), Some(&text("takenBack")));
    let by = field(&value, "performer").expect("a performer");
    assert_eq!(field(by, "id"), Some(&text(&performer.to_string())));
    assert_eq!(field(by, "account"), Some(&text(&account.to_string())));
    assert_eq!(field(&value, "signOff"), Some(&text(&sign_off.to_string())));
    let Some(Value::Array(members)) = field(&value, "members") else {
        panic!("members: {value:?}")
    };
    assert_eq!(members.len(), 2);
    assert_eq!(
        field(&members[0], "id"),
        Some(&text(&performer.to_string()))
    );
    assert_eq!(
        field(&members[0], "account"),
        Some(&text(&account.to_string()))
    );
    assert_eq!(field(&members[0], "current"), Some(&Value::Bool(true)));
    assert_eq!(field(&members[0], "acknowledged"), Some(&text("deleted")));
    assert_eq!(field(&members[1], "current"), Some(&Value::Bool(false)));
    assert_eq!(field(&members[1], "acknowledged"), Some(&text("missing")));
}
