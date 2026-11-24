//! A split write through a relay (NOC-421; NOC-431, B′): a write above the
//! relay's change limit reaches another member as several changes, none of
//! which the relay refuses, and the other member ends with every object of
//! the write.

use crate::members::{AddMember, Role};
use crate::sync::tests::support::*;
use crate::value::Value;

/// 72 tasks of 256 KiB titles: about 18 MiB in one write, above the
/// relay's 16 MiB.
const TASKS: usize = 72;

fn title(n: usize) -> String {
    format!("{n:08}{}", "x".repeat(256 * 1024 - 8))
}

#[tokio::test(flavor = "multi_thread")]
async fn a_split_write_reaches_another_member_whole() {
    let relay = LocalRelay::start().await;
    let admin = open(1, &relay.url()).await;
    let member = open(2, &relay.url()).await;
    synced(&member).await;
    let project = now(admin.spaces().create()).unwrap();
    let writer = AddMember {
        role: Role::Writer,
        fingerprint: None,
    };
    now(project.members().add(&now(member.invitation()), writer)).unwrap();
    synced(&admin).await;

    let ids = now(project.write(|tx| {
        for n in 0..TASKS {
            tx.create("tasks", [("title", Value::from(title(n).as_str()))])?;
        }
        Ok(())
    }))
    .unwrap()
    .ids;

    let status = synced(&admin).await;
    assert_eq!(status.refused, 0, "the relay refused a part of the write");
    eventually("the member holds every task of the write", || {
        let Ok(space) = now(member.spaces().get(project.id())) else {
            return false;
        };
        ids.iter().enumerate().all(|(n, id)| {
            now(space.get("tasks", *id))
                .ok()
                .flatten()
                .and_then(|task| task.get::<&str>("title").ok().flatten().map(str::to_owned))
                .as_deref()
                == Some(title(n).as_str())
        })
    })
    .await;
}
