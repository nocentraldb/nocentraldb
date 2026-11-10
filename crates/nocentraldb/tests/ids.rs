//! An object id shows as 32 lowercase hex digits, its 16 bytes in order,
//! and reads back from them (RFC 0009, Amendment 3). A space id has the
//! same form.

mod support;

use nocentraldb::{ErrorCode, ObjectId, SpaceId, Value};
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
