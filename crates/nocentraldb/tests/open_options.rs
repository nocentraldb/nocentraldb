//! Where a database lives (RFC 0009, "The Rust SDK", decisions S19 and
//! S27): the app names a path or memory, and opening with neither is
//! refused with `invalidValue`, so no data lands where the app did not say.

mod support;

use nocentraldb::{Db, ErrorCode, OpenOptions};
use support::{LOCK, now};

#[test]
fn opening_with_neither_a_path_nor_memory_is_invalid_value() {
    let error = now(Db::open(OpenOptions::new(LOCK))).unwrap_err();

    assert_eq!(error.code(), ErrorCode::InvalidValue);
    assert!(error.to_string().contains("path"), "{error}");
    assert!(error.to_string().contains("in_memory"), "{error}");
}

#[test]
fn a_database_in_memory_opens_and_holds_its_spaces() {
    let db = now(Db::open(OpenOptions::new(LOCK).in_memory())).unwrap();

    let created = now(db.spaces().create()).unwrap();

    assert_eq!(now(db.spaces().get(created.id())).unwrap().id(), created.id());
}
