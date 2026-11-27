//! A person-key record written while the relay was out of reach, then the
//! database closed (NOC-547): the device kept the record, so the reopened
//! database puts it once it connects, and another member reads the
//! personal value.

use super::support::*;
use crate::Value;
use crate::db::OpenOptions;
use crate::keys::{Keys, Secrets};
use crate::members::{AddMember, Role};

const ADMIN: Who = 1;
const MEMBER: Who = 2;

/// The database of `ADMIN` at `dir` on the personal test model, syncing
/// with the relay at `url`.
async fn open_at(dir: &TempDir, url: &str) -> crate::Db {
    let key_file = dir.0.join("member.key");
    if !key_file.exists() {
        let keys = Keys::of_account(
            Secrets::new([ADMIN; 32], [ADMIN ^ 0x80; 32]),
            account(),
        );
        std::fs::write(&key_file, keys.encode()).unwrap();
    }
    let options = OpenOptions::new(&crate::personal_write_tests::lock())
        .path(dir.0.join("data"))
        .key_file(key_file)
        .relay(url);
    crate::Db::open(options).await.expect("the personal lock opens")
}

#[tokio::test(flavor = "multi_thread")]
async fn a_record_written_offline_is_put_after_a_reopen_and_another_member_reads_the_value() {
    let relay = LocalRelay::start().await;
    let dir = TempDir::new();
    let admin = open_at(&dir, &relay.url()).await;
    let member = open_with(MEMBER, &relay.url(), &crate::personal_write_tests::lock()).await;
    let project = admin.spaces().create().await.unwrap();
    let options = AddMember {
        role: Role::Writer,
        fingerprint: None,
    };
    let invitation = member.invitation().await;
    project.members().add(&invitation, options).await.unwrap();
    synced(&admin).await;
    let space = project.id();

    relay.proxy.cut();
    let anna = project
        .create("projects", [("name", Value::from("Anna"))])
        .await
        .unwrap()
        .ids[0];
    drop((project, admin));
    relay.proxy.restore();

    let admin = open_at(&dir, &relay.url()).await;
    synced(&admin).await;
    eventually("the member reads the name the admin wrote offline", || {
        let Ok(project) = now(member.spaces().get(space)) else {
            return false;
        };
        let object = now(project.get("projects", anna)).unwrap();
        object.is_some_and(|object| object.get::<&str>("name").unwrap() == Some("Anna"))
    })
    .await;
}
