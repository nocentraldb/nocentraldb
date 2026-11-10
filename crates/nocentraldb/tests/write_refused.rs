//! What a write refuses before anything is written, with an error and no
//! change (RFC 0009, "What the SDK refuses"): a value that does not fit
//! the field's kind, an update or delete of an object the copy does not
//! hold. A collection or field the model does not have is refused too.

mod support;

use nocentraldb::{ErrorCode, ObjectId, Value};
use support::{now, open};

#[test]
fn a_value_of_another_kind_is_refused_and_nothing_is_written() {
    let db = open();
    let project = now(db.spaces().create()).unwrap();
    let mut created = None;

    let error = now(project.write(|tx| {
        created = Some(tx.create("tasks", [("title", Value::from("Kept out"))])?);
        tx.create("tasks", [("priority", Value::from("high"))])?;
        Ok(())
    }))
    .unwrap_err();

    assert_eq!(error.code(), ErrorCode::InvalidValue);
    assert!(
        now(project.get("tasks", created.unwrap()))
            .unwrap()
            .is_none()
    );
}

#[test]
fn an_option_the_lock_does_not_list_is_refused() {
    let db = open();
    let project = now(db.spaces().create()).unwrap();

    let error = now(project.write(|tx| {
        tx.create("tasks", [("status", Value::from("blocked"))])?;
        Ok(())
    }))
    .unwrap_err();

    assert_eq!(error.code(), ErrorCode::InvalidValue);
}

#[test]
fn an_update_or_delete_of_an_object_the_copy_does_not_hold_is_refused() {
    let db = open();
    let project = now(db.spaces().create()).unwrap();
    let unknown: ObjectId = "00112233445566778899aabbccddeeff".parse().unwrap();

    let update =
        now(project.write(|tx| tx.update("tasks", unknown, [("priority", Value::from(1))])));
    let delete = now(project.write(|tx| tx.delete("tasks", unknown)));

    assert_eq!(update.unwrap_err().code(), ErrorCode::NotFound);
    assert_eq!(delete.unwrap_err().code(), ErrorCode::NotFound);
}

#[test]
fn a_collection_or_field_the_model_does_not_have_is_refused() {
    let db = open();
    let project = now(db.spaces().create()).unwrap();

    let collection = now(project.write(|tx| {
        tx.create("people", [("name", Value::from("Anna"))])?;
        Ok(())
    }));
    let field = now(project.write(|tx| {
        tx.create("tasks", [("owner", Value::from("Anna"))])?;
        Ok(())
    }));

    assert_eq!(collection.unwrap_err().code(), ErrorCode::NotFound);
    assert_eq!(field.unwrap_err().code(), ErrorCode::NotFound);
}
