//! An object id shows as 32 lowercase hex digits, its 16 bytes in order,
//! and reads back from them (RFC 0009, "Get by id"). A space id has the
//! same form. A member id shows as 64, its 32 bytes in order (Carsten,
//! 2026-11-13, NOC-298).

mod support;

use nocentraldb::{ErrorCode, MemberId, ObjectId, SpaceId, Value};
use support::{now, open};

fn is_lowercase_hex(text: &str, digits: usize) -> bool {
    text.len() == digits && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

#[test]
fn an_object_id_shows_as_32_lowercase_hex_digits_and_reads_back() {
    let db = open();
    let project = now(db.spaces().create()).unwrap();
    let written = now(project.write(|tx| {
        tx.create("projects", [("name", Value::from("A"))])?;
        Ok(())
    }))
    .unwrap();
    let id = written.ids[0];

    let text = id.to_string();
    assert!(is_lowercase_hex(&text, 32), "{text}");
    assert_eq!(text.parse::<ObjectId>().unwrap(), id);
    let read = now(project.get("projects", text.parse().unwrap())).unwrap();
    assert_eq!(read.unwrap().id(), id);
}

#[test]
fn a_space_id_shows_as_32_lowercase_hex_digits_and_reads_back() {
    let db = open();
    let project = now(db.spaces().create()).unwrap();

    let text = project.id().to_string();
    assert!(is_lowercase_hex(&text, 32), "{text}");
    assert_eq!(text.parse::<SpaceId>().unwrap(), project.id());
}

#[test]
fn an_id_in_another_form_is_refused() {
    let upper = "0123456789ABCDEF0123456789ABCDEF";
    let short = "0123456789abcdef";
    let not_hex = "0123456789abcdef0123456789abcdeg";
    for text in [upper, short, not_hex, ""] {
        let error = text.parse::<ObjectId>().unwrap_err();
        assert_eq!(error.code(), ErrorCode::InvalidValue, "{text}");
    }
}

#[test]
fn a_member_id_shows_as_64_lowercase_hex_digits_and_reads_back() {
    let db = open();
    let project = now(db.spaces().create()).unwrap();
    let me = now(project.members().list()).unwrap()[0].id;

    let text = me.to_string();
    assert!(is_lowercase_hex(&text, 64), "{text}");
    assert_eq!(text.parse::<MemberId>().unwrap(), me);
    assert_eq!(format!("{me:?}"), format!("MemberId({text})"));
}

#[test]
fn a_member_id_in_another_form_is_refused() {
    let digits = "0123456789abcdef".repeat(4);
    let upper = digits.to_uppercase();
    let short = &digits[..32];
    let not_hex = format!("{}g", &digits[..63]);
    for text in [upper.as_str(), short, not_hex.as_str(), ""] {
        let error = text.parse::<MemberId>().unwrap_err();
        assert_eq!(error.code(), ErrorCode::InvalidValue, "{text}");
    }
}
