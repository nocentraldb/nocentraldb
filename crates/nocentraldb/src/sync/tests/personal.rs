//! Reading a personal field across devices (RFC 0009, "How a personal
//! field reads"; Carsten, 2026-11-12, NOC-244): another member reads a
//! personal value once it fetched the person's key. After an erasure,
//! provisional or final, the field reads as unset on every device and the
//! object names it in `erased()`.

use ncdb_core::field::FieldType;
use ncdb_core::personal::{PERSON_FIELD, Person, PersonField};
use ncdb_core::view::FieldState;

use super::provisional::erase_key;
use super::support::*;
use crate::Value;
use crate::db::{Db, lock};
use crate::id::ObjectId;
use crate::members::{AddMember, Role};
use crate::space::Space;

const ADMIN: Who = 1;
const MEMBER: Who = 2;

/// The admin and a writer of one account, both on the personal test model,
/// sharing a space.
async fn shared(relay: &LocalRelay) -> (Db, Db, Space) {
    let lock_json = crate::personal_write_tests::lock();
    let admin = open_with(ADMIN, &relay.url(), &lock_json).await;
    let member = open_with(MEMBER, &relay.url(), &lock_json).await;
    let project = now(admin.spaces().create()).unwrap();
    let options = AddMember {
        role: Role::Writer,
        fingerprint: None,
    };
    now(project.members().add(&now(member.invitation()), options)).unwrap();
    synced(&admin).await;
    (admin, member, project)
}

fn person(db: &Db, space: &Space, object: ObjectId) -> Person {
    let inner = lock(&db.inner);
    let row = inner
        .view
        .get(space.id().0, "projects", object.0)
        .unwrap()
        .unwrap();
    let Some(FieldState::Basic(state)) = row.fields.get(PERSON_FIELD) else {
        panic!("$person is set");
    };
    Person::from_value(&PersonField.read(state).value.unwrap().value).unwrap()
}

/// The name of `object` and the object's erased fields, once the copy
/// holds the object.
fn name(space: &Space, object: ObjectId) -> Option<(Option<String>, Vec<String>)> {
    let object = now(space.get("projects", object)).unwrap()?;
    let name = object.get::<&str>("name").unwrap().map(str::to_owned);
    Some((name, object.erased().to_vec()))
}

#[tokio::test(flavor = "multi_thread")]
async fn the_writer_reads_its_personal_value_at_once() {
    let relay = LocalRelay::start().await;
    let (_admin, _member, project) = shared(&relay).await;
    let id = now(project.write(|tx| {
        tx.create("projects", [("name", Value::from("Anna Schmidt"))])?;
        Ok(())
    }))
    .unwrap()
    .ids[0];
    assert_eq!(
        name(&project, id).unwrap(),
        (Some("Anna Schmidt".to_owned()), Vec::new())
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn another_member_reads_it_once_it_holds_the_key_and_an_erasure_empties_it() {
    let relay = LocalRelay::start().await;
    let (admin, member, project) = shared(&relay).await;
    let id = now(project.write(|tx| {
        tx.create("projects", [("name", Value::from("Anna Schmidt"))])?;
        Ok(())
    }))
    .unwrap()
    .ids[0];
    synced(&admin).await;
    let on_member = member.spaces().get(project.id()).await.unwrap();
    eventually("the member reads the name", || {
        name(&on_member, id).is_some_and(|(name, _)| name.as_deref() == Some("Anna Schmidt"))
    })
    .await;

    // A provisional erasure: the key is kept, but the field reads as
    // erased on every device (RFC 0008, "A provisional erasure in a space
    // with one account").
    let anna = person(&admin, &project, id);
    erase_key(&admin, &project, anna.person_key);
    synced(&admin).await;
    assert_eq!(name(&project, id).unwrap(), (None, vec!["name".to_owned()]));
    eventually("the member reads it as erased", || {
        name(&on_member, id) == Some((None, vec!["name".to_owned()]))
    })
    .await;
}
