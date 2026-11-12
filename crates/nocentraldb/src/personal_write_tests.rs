//! Writing a personal field (RFC 0009, "Personal fields"; RFC 0005,
//! "Identifying the person"; Carsten, 2026-11-12, NOC-244): the SDK seals
//! the value under the key of the person it is about, found from the
//! schema's `person`. The first personal value of a person draws the
//! person's key, writes its person-key entry before the content, and sets
//! `$person` on the person's object in the same change as the value. A
//! person link that is unset is refused with `NoPerson`.

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so these tests run on both targets (NOC-175).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::cbor;
use ncdb_core::field::FieldType;
use ncdb_core::model_lock::{ModelLock, Person as Whom};
use ncdb_core::personal::{self, PERSON_FIELD, Person, PersonField, Place, Sealed};
use ncdb_core::view::FieldState;

use crate::id::ObjectId;
use crate::space::Space;
use crate::tx::Create;
use crate::{Db, ErrorCode, OpenOptions, Value};

const LOCK: &str = include_str!("../tests/model/model.lock.json");

fn now<F: std::future::Future>(future: F) -> F::Output {
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    match std::pin::pin!(future).poll(&mut cx) {
        std::task::Poll::Ready(output) => output,
        std::task::Poll::Pending => panic!("the SDK never waits on the device"),
    }
}

/// The test model with `projects.name` personal to the project itself, and
/// `tasks.title` personal to the task's `project`.
pub(crate) fn lock() -> String {
    let mut lock = ModelLock::from_json(LOCK).unwrap();
    lock.collections
        .get_mut("projects")
        .unwrap()
        .fields
        .get_mut("name")
        .unwrap()
        .personal = Some(Whom::Itself);
    lock.collections
        .get_mut("tasks")
        .unwrap()
        .fields
        .get_mut("title")
        .unwrap()
        .personal = Some(Whom::Link("project".into()));
    lock.model.hash = lock.compute_hash();
    lock.to_json().to_string()
}

fn open() -> Db {
    now(Db::open(OpenOptions::new(&lock()).in_memory())).unwrap()
}

/// The winning value of `field` of `object`, as the view keeps it.
fn stored(db: &Db, space: &Space, collection: &str, object: ObjectId, field: &str) -> cbor::Value {
    let inner = db.lock();
    let row = inner
        .view
        .get(space.id().0, collection, object.0)
        .unwrap()
        .unwrap();
    let Some(FieldState::Basic(state)) = row.fields.get(field) else {
        panic!("{field} is written");
    };
    PersonField.read(state).value.unwrap().value
}

fn person(db: &Db, space: &Space, collection: &str, object: ObjectId) -> Person {
    Person::from_value(&stored(db, space, collection, object, PERSON_FIELD)).expect("$person")
}

/// Opens the sealed `field` of `object` with the key the writer holds.
fn opened(db: &Db, space: &Space, collection: &str, object: ObjectId, field: &str) -> cbor::Value {
    let sealed = Sealed::from_value(&stored(db, space, collection, object, field)).unwrap();
    let inner = db.lock();
    let key = &inner.spaces[&space.id().0].person_keys[&sealed.person_key];
    let place = Place {
        space: space.id().0,
        object: object.0,
        person_key: sealed.person_key,
        collection,
        field,
    };
    personal::open(key, &place, &sealed.bytes).unwrap()
}

fn anna() -> cbor::Value {
    cbor::Value::Text("Anna Schmidt".into())
}

#[test]
fn the_first_value_of_a_person_draws_its_key_and_sets_person() {
    let db = open();
    let space = now(db.spaces().create()).unwrap();
    let written = now(space.write(|tx| {
        tx.create("projects", [("name", Value::from("Anna Schmidt"))])?;
        Ok(())
    }))
    .unwrap();
    let project = written.ids[0];

    let anna_is = person(&db, &space, "projects", project);
    let sealed = Sealed::from_value(&stored(&db, &space, "projects", project, "name")).unwrap();
    assert_eq!(
        sealed.person_key, anna_is.person_key,
        "sealed under its key"
    );
    assert_eq!(opened(&db, &space, "projects", project, "name"), anna());
    let inner = db.lock();
    let resolved = inner.spaces[&space.id().0].checks.resolved();
    assert!(
        resolved.person_key(anna_is.person_key).is_some(),
        "the person-key entry is in the log"
    );
}

#[test]
fn a_later_value_of_the_same_person_keeps_its_key() {
    let db = open();
    let space = now(db.spaces().create()).unwrap();
    let project = now(space.write(|tx| {
        tx.create("projects", [("name", Value::from("Anna"))])?;
        Ok(())
    }))
    .unwrap()
    .ids[0];
    let first = person(&db, &space, "projects", project);
    now(space.write(|tx| tx.update("projects", project, [("name", Value::from("Anna Schmidt"))])))
        .unwrap();
    assert_eq!(person(&db, &space, "projects", project), first);
    assert_eq!(opened(&db, &space, "projects", project, "name"), anna());
    assert_eq!(db.lock().spaces[&space.id().0].person_keys.len(), 1);
}

#[test]
fn a_value_about_a_linked_person_is_sealed_under_the_links_target() {
    let db = open();
    let space = now(db.spaces().create()).unwrap();
    // The project has no personal value yet, so no key: the task's title
    // draws it and sets `$person` on the project.
    let project = now(space.write(|tx| {
        tx.create("projects", [])?;
        Ok(())
    }))
    .unwrap()
    .ids[0];
    let task = now(space.write(|tx| {
        tx.create(
            "tasks",
            [
                ("project", Value::from(project)),
                ("title", Value::from("Anna Schmidt")),
            ],
        )?;
        Ok(())
    }))
    .unwrap()
    .ids[0];
    let theirs = person(&db, &space, "projects", project);
    let sealed = Sealed::from_value(&stored(&db, &space, "tasks", task, "title")).unwrap();
    assert_eq!(sealed.person_key, theirs.person_key);
    assert_eq!(opened(&db, &space, "tasks", task, "title"), anna());
}

#[test]
fn a_value_whose_person_link_is_unset_is_refused_with_no_person() {
    let db = open();
    let space = now(db.spaces().create()).unwrap();
    let error = now(space.write(|tx| {
        tx.create("tasks", [("title", Value::from("Anna Schmidt"))])?;
        Ok(())
    }))
    .unwrap_err();
    assert_eq!(error.code(), ErrorCode::NoPerson);
}

#[test]
fn a_value_must_fit_the_field() {
    let db = open();
    let space = now(db.spaces().create()).unwrap();
    let project = now(space.write(|tx| {
        tx.create("projects", [("name", Value::from("Anna"))])?;
        Ok(())
    }))
    .unwrap()
    .ids[0];
    let error = now(space.write(|tx| tx.update("projects", project, [("name", Value::from(7))])))
        .unwrap_err();
    assert_eq!(error.code(), ErrorCode::InvalidValue);
}

#[test]
fn the_app_cannot_write_person() {
    let db = open();
    let space = now(db.spaces().create()).unwrap();
    let error = now(space.write(|tx| {
        tx.create("projects", [(PERSON_FIELD, Value::from("x"))])?;
        Ok(())
    }))
    .unwrap_err();
    assert_eq!(error.code(), ErrorCode::NotFound);
}

#[test]
fn the_same_person_in_another_space_keeps_the_person_id_with_a_new_key() {
    let db = open();
    let sales = now(db.spaces().create()).unwrap();
    let support = now(db.spaces().create()).unwrap();
    let in_sales = now(sales.write(|tx| {
        tx.create("projects", [("name", Value::from("Anna Schmidt"))])?;
        Ok(())
    }))
    .unwrap()
    .ids[0];
    let anna_in_sales = now(sales.get("projects", in_sales)).unwrap().unwrap();
    let in_support = now(support.write(|tx| {
        tx.create_with(
            "projects",
            [("name", Value::from("Anna Schmidt"))],
            Create {
                same_person_as: Some(anna_in_sales),
            },
        )?;
        Ok(())
    }))
    .unwrap()
    .ids[0];
    let there = person(&db, &sales, "projects", in_sales);
    let here = person(&db, &support, "projects", in_support);
    assert_eq!(here.person, there.person, "the same person id");
    assert_ne!(here.person_key, there.person_key, "a key of its own");
    assert_eq!(
        opened(&db, &support, "projects", in_support, "name"),
        anna()
    );
}

#[test]
fn an_object_does_not_show_person() {
    let db = open();
    let space = now(db.spaces().create()).unwrap();
    let project = now(space.write(|tx| {
        tx.create("projects", [("name", Value::from("Anna"))])?;
        Ok(())
    }))
    .unwrap()
    .ids[0];
    let object = now(space.get("projects", project)).unwrap().unwrap();
    assert_eq!(
        object.get::<&str>(PERSON_FIELD).unwrap_err().code(),
        ErrorCode::NotFound
    );
}

/// A person whose key this device does not hold yet, as before a fetch
/// after a reconnect: the write is refused with `PersonKeyNotHeld`, and no
/// fresh key is drawn (Carsten, 2026-11-12, NOC-255).
#[test]
fn a_value_whose_person_key_is_not_held_yet_is_refused() {
    let db = open();
    let space = now(db.spaces().create()).unwrap();
    let project = now(space.write(|tx| {
        tx.create("projects", [("name", Value::from("Anna"))])?;
        Ok(())
    }))
    .unwrap()
    .ids[0];
    let anna = person(&db, &space, "projects", project);
    db.lock()
        .spaces
        .get_mut(&space.id().0)
        .unwrap()
        .person_keys
        .clear();
    let error = now(space.write(|tx| {
        tx.update("projects", project, [("name", Value::from("Anna Schmidt"))])
    }))
    .unwrap_err();
    assert_eq!(error.code(), ErrorCode::PersonKeyNotHeld);
    assert_eq!(person(&db, &space, "projects", project), anna, "no new key");
}
