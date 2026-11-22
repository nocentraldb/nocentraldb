//! Erasing a person through the bridge (NOC-406; RFC 0009, "The Rust SDK",
//! "Erasing a person"; Carsten, 2026-11-15, NOC-326): `erasureRequest`,
//! `account.signOff`, `erase` and `erasures.takeBack`, the Rust SDK's
//! `Db::erasure_request`, `Account::sign_off`, `Db::erase` and
//! `Erasures::take_back`. On one database in memory, which signs off its
//! own request in a space whose members all belong to its account, so the
//! erasure is provisional and can be taken back.

use ncdb_core::cbor::Value;

use super::{Bridge, as_text, code, field, map, ok, send, text};

/// A database in memory on the test lock with `projects.name` personal.
fn opened() -> Bridge {
    let mut bridge = Bridge::new();
    let lock = crate::personal_write_tests::lock();
    send(
        &mut bridge,
        1,
        "open",
        map(&[("lock", text(&lock)), ("inMemory", Value::Bool(true))]),
    );
    bridge
}

/// A space with Anna in it: the space and Anna's object id.
fn anna(bridge: &mut Bridge) -> (Value, Value) {
    let space = ok(bridge, "spaces.create", map(&[]));
    let op = map(&[
        ("op", text("create")),
        ("collection", text("projects")),
        ("values", map(&[("name", text("Anna Schmidt"))])),
    ]);
    let written = ok(
        bridge,
        "write",
        map(&[("space", space.clone()), ("ops", Value::Array(vec![op]))]),
    );
    let Some(Value::Array(ids)) = field(&written, "ids") else {
        panic!("ids: {written:?}")
    };
    (space, ids[0].clone())
}

/// Anna as `get` reads her.
fn read(bridge: &mut Bridge, space: &Value, id: &Value) -> Value {
    ok(
        bridge,
        "get",
        map(&[
            ("space", space.clone()),
            ("collection", text("projects")),
            ("id", id.clone()),
        ]),
    )
}

/// The request to erase Anna, as its text.
fn request(bridge: &mut Bridge, space: &Value, id: &Value) -> Value {
    ok(
        bridge,
        "erasureRequest",
        map(&[
            ("space", space.clone()),
            ("collection", text("projects")),
            ("id", id.clone()),
        ]),
    )
}

#[test]
fn the_request_to_erase_a_person_is_its_text() {
    let mut bridge = opened();
    let (space, id) = anna(&mut bridge);
    let request = request(&mut bridge, &space, &id);
    assert!(
        as_text(&request).starts_with("ncdb-erase-1:"),
        "{request:?}"
    );
}

#[test]
fn an_object_of_no_person_is_refused_with_no_person() {
    let mut bridge = opened();
    let space = ok(&mut bridge, "spaces.create", map(&[]));
    let op = map(&[
        ("op", text("create")),
        ("collection", text("tasks")),
        ("values", map(&[("priority", Value::Unsigned(2))])),
    ]);
    let written = ok(
        &mut bridge,
        "write",
        map(&[("space", space.clone()), ("ops", Value::Array(vec![op]))]),
    );
    let Some(Value::Array(ids)) = field(&written, "ids") else {
        panic!("ids: {written:?}")
    };
    let args = map(&[
        ("space", space),
        ("collection", text("tasks")),
        ("id", ids[0].clone()),
    ]);
    assert_eq!(code(&mut bridge, "erasureRequest", args), "noPerson");
}

#[test]
fn an_object_that_does_not_exist_is_not_found() {
    let mut bridge = opened();
    let space = ok(&mut bridge, "spaces.create", map(&[]));
    let args = map(&[
        ("space", space),
        ("collection", text("projects")),
        ("id", text(&"ab".repeat(16))),
    ]);
    assert_eq!(code(&mut bridge, "erasureRequest", args), "notFound");
}

#[test]
fn the_account_signs_off_erases_and_takes_the_erasure_back() {
    let mut bridge = opened();
    let (space, id) = anna(&mut bridge);
    let request = request(&mut bridge, &space, &id);

    let signed = ok(
        &mut bridge,
        "account.signOff",
        map(&[("request", request.clone())]),
    );
    let sign_offs = field(&signed, "signOffs").expect("signOffs").clone();
    assert!(
        as_text(&sign_offs).starts_with("ncdb-sign-off-1:"),
        "{signed:?}"
    );
    assert_eq!(field(&signed, "notSigned"), Some(&Value::Array(vec![])));

    let erased = ok(
        &mut bridge,
        "erase",
        map(&[("request", request), ("signOffs", sign_offs)]),
    );
    let Some(Value::Array(shreds)) = field(&erased, "shreds") else {
        panic!("shreds: {erased:?}")
    };
    assert_eq!(shreds.len(), 1, "{erased:?}");
    assert_eq!(field(&shreds[0], "space"), Some(&space));
    assert_eq!(field(&shreds[0], "provisional"), Some(&Value::Bool(true)));
    let shred = field(&shreds[0], "shred").expect("a shred").clone();
    assert_eq!(field(&erased, "skipped"), Some(&Value::Array(vec![])));
    let notice = field(&erased, "notice").expect("a notice");
    assert_eq!(field(notice, "code"), Some(&text("erased")));
    let fields = field(&read(&mut bridge, &space, &id), "fields").cloned();
    assert_eq!(field(fields.as_ref().unwrap(), "name"), Some(&Value::Null));

    let audit = ok(
        &mut bridge,
        "erasures.list",
        map(&[("space", space.clone())]),
    );
    let Value::Array(audit) = audit else {
        panic!("audit: {audit:?}")
    };
    assert_eq!(field(&audit[0], "shred"), Some(&shred));

    let taken_back = ok(
        &mut bridge,
        "erasures.takeBack",
        map(&[("space", space.clone()), ("shred", shred)]),
    );
    assert_eq!(field(&taken_back, "code"), Some(&text("takenBack")));
    let fields = field(&read(&mut bridge, &space, &id), "fields").cloned();
    assert_eq!(
        field(fields.as_ref().unwrap(), "name"),
        Some(&text("Anna Schmidt"))
    );
}

#[test]
fn a_text_that_is_no_request_or_no_sign_off_is_an_invalid_value() {
    let mut bridge = opened();
    let (space, id) = anna(&mut bridge);
    let request = request(&mut bridge, &space, &id);
    assert_eq!(
        code(
            &mut bridge,
            "account.signOff",
            map(&[("request", text("ncdb-pair-1:AAAA"))])
        ),
        "invalidValue"
    );
    assert_eq!(
        code(
            &mut bridge,
            "erase",
            map(&[("request", request), ("signOffs", text("not a sign-off"))])
        ),
        "invalidValue"
    );
}

#[test]
fn erasing_again_skips_the_space_where_the_person_has_no_key_left() {
    let mut bridge = opened();
    let (space, id) = anna(&mut bridge);
    let request = request(&mut bridge, &space, &id);
    let signed = ok(
        &mut bridge,
        "account.signOff",
        map(&[("request", request.clone())]),
    );
    let sign_offs = field(&signed, "signOffs").expect("signOffs").clone();
    // Erase once, then again with the same texts: the person has no key
    // left there, so the space is skipped with noKey.
    ok(
        &mut bridge,
        "erase",
        map(&[
            ("request", request.clone()),
            ("signOffs", sign_offs.clone()),
        ]),
    );
    let again = ok(
        &mut bridge,
        "erase",
        map(&[("request", request), ("signOffs", sign_offs)]),
    );
    assert_eq!(field(&again, "shreds"), Some(&Value::Array(vec![])));
    let Some(Value::Array(skipped)) = field(&again, "skipped") else {
        panic!("skipped: {again:?}")
    };
    assert_eq!(field(&skipped[0], "space"), Some(&space));
    assert_eq!(field(&skipped[0], "reason"), Some(&text("noKey")));
}

#[test]
fn take_back_of_a_shred_the_space_does_not_have_is_an_invalid_value() {
    let mut bridge = opened();
    let (space, _) = anna(&mut bridge);
    let args = map(&[("space", space), ("shred", text(&"aa".repeat(32)))]);
    assert_eq!(code(&mut bridge, "erasures.takeBack", args), "invalidValue");
}
