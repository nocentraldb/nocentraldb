//! What each command does, through the public Rust SDK (RFC 0009).

use std::time::Duration;

use nocentraldb::{
    AddMember, Db, ErrorCode, Invitation, ObjectId, Page, Role, Space, SpaceId, Value,
};

use crate::args::Command;
use crate::wait;

/// The collection and index of the lock (model.ts).
const TASKS: &str = "tasks";
const BY_STATUS: &str = "byStatus";

/// How many tasks `list` and `watch` show.
const PAGE: usize = 200;

/// Runs one command on `db`. `syncs` says whether a relay was given, so
/// that a write waits until the relay holds it.
pub async fn run(db: &Db, command: Command, syncs: bool) -> Result<(), String> {
    match command {
        Command::Help => unreachable!("main prints the usage"),
        Command::Invitation => {
            let invitation = db.invitation().await;
            println!("{invitation}");
            println!("fingerprint: {}", invitation.fingerprint());
        }
        Command::Create => {
            let space = db.spaces().create().await.map_err(text)?;
            settle(db, syncs).await?;
            println!("{}", space.id());
        }
        Command::Spaces => {
            for info in db.spaces().list().await.map_err(text)? {
                let role = info.role.map_or("none", role_name);
                println!("{}  {role}", info.id);
            }
        }
        Command::Invite {
            space,
            invitation,
            fingerprint,
        } => invite(db, &space, &invitation, fingerprint, syncs).await?,
        Command::Add { space, title } => {
            let space = space_of(db, &space).await?;
            let written = space
                .write(|tx| {
                    tx.create(
                        TASKS,
                        [
                            ("title", Value::from(title.as_str())),
                            ("status", "open".into()),
                        ],
                    )?;
                    Ok(())
                })
                .await
                .map_err(text)?;
            settle(db, syncs).await?;
            println!("{}", written.ids[0]);
        }
        Command::Done { space, task } => {
            let space = space_of(db, &space).await?;
            let task: ObjectId = task.parse().map_err(text)?;
            space
                .write(|tx| tx.update(TASKS, task, [("status", "done".into())]))
                .await
                .map_err(text)?;
            settle(db, syncs).await?;
        }
        Command::List => {
            let query = db.index(TASKS, BY_STATUS).map_err(text)?.take(PAGE);
            print!("{}", lines(&db.find(query).await.map_err(text)?)?);
        }
        Command::Watch { until, timeout } => watch(db, until, timeout).await?,
        Command::Status { space } => {
            let space = space_of(db, &space).await?;
            // Each run opens the database anew: with a relay, report the
            // status after this run's first sync, not before it.
            settle(db, syncs).await?;
            status(&space);
        }
        Command::Sync { space, timeout } => {
            let space = space
                .map(|s| s.parse::<SpaceId>().map_err(text))
                .transpose()?;
            wait::synced(db, space, timeout).await?;
            let now = db.sync().current();
            println!(
                "connection: {:?}, unconfirmed: {}, refused: {}",
                now.connection, now.unconfirmed, now.refused
            );
        }
    }
    Ok(())
}

/// Adds the device of `invitation` to `space` as a writer. An account new
/// to this database needs its fingerprint confirmed: the person compares
/// the one this prints with the one `tasks invitation` shows on the other
/// device, and passes it with `--fingerprint`.
async fn invite(
    db: &Db,
    space: &str,
    invitation: &str,
    fingerprint: Option<String>,
    syncs: bool,
) -> Result<(), String> {
    let space = space_of(db, space).await?;
    let invitation: Invitation = invitation.trim().parse().map_err(text)?;
    let shown = invitation.fingerprint();
    let confirmed = match fingerprint {
        None => None,
        Some(typed) if digits(&typed) == digits(&shown.to_string()) => Some(shown),
        Some(_) => {
            return Err(format!(
                "the fingerprint does not match the invitation's: {shown}. \
                 Do not add this device unless the other device shows this one."
            ));
        }
    };
    let options = AddMember {
        role: Role::Writer,
        fingerprint: confirmed,
    };
    match space.members().add(&invitation, options).await {
        Ok(notice) => println!("{}", notice.text),
        Err(e) if e.code() == ErrorCode::UnknownAccount => {
            return Err(format!(
                "the invitation is from an account this database does not know yet. \
                 Check that `tasks invitation` on the other device shows the fingerprint\n  \
                 {shown}\nand run this command again with --fingerprint \"{shown}\"."
            ));
        }
        Err(e) => return Err(text(e)),
    }
    settle(db, syncs).await
}

/// Prints the list each time it changes, until a task titled `until`
/// is listed or `timeout` passes.
async fn watch(db: &Db, until: Option<String>, timeout: Option<Duration>) -> Result<(), String> {
    let query = db.index(TASKS, BY_STATUS).map_err(text)?.take(PAGE);
    let mut pages = db.subscribe(query);
    let follow = async {
        while let Some(page) = wait::next(&mut pages).await {
            let page = page.map_err(text)?;
            println!("--");
            print!("{}", lines(&page)?);
            if let Some(title) = &until
                && titles(&page)?.iter().any(|t| t == title)
            {
                return Ok(());
            }
        }
        Err("the subscription ended".to_owned())
    };
    match timeout {
        Some(timeout) => tokio::time::timeout(timeout, follow)
            .await
            .map_err(|_| "no such task arrived in time".to_owned())?,
        None => follow.await,
    }
}

/// Waits until the relay holds what this device wrote, when it syncs.
async fn settle(db: &Db, syncs: bool) -> Result<(), String> {
    match syncs {
        true => wait::synced(db, None, crate::args::WAIT).await,
        false => Ok(()),
    }
}

/// The space with the id `text`, which this device holds.
async fn space_of(db: &Db, id: &str) -> Result<Space, String> {
    let id: SpaceId = id.parse().map_err(text)?;
    db.spaces().get(id).await.map_err(text)
}

/// One line per task: `[ ]` or `[x]`, its title, its id and its space.
/// The index orders by status as text, so "done" before "open"; the open
/// tasks come first here, each group still by title.
fn lines(page: &Page) -> Result<String, String> {
    let mut out = String::new();
    let mut tasks: Vec<_> = page.items.iter().collect();
    tasks.sort_by_key(|task| task.get::<&str>("status").ok().flatten() == Some("done"));
    for task in tasks {
        let title: &str = task.get("title").map_err(text)?.unwrap_or("");
        let done = task.get::<&str>("status").map_err(text)? == Some("done");
        let mark = if done { "[x]" } else { "[ ]" };
        out += &format!(
            "{mark} {title}  (task {}, space {})\n",
            task.id(),
            task.space()
        );
    }
    Ok(out)
}

fn titles(page: &Page) -> Result<Vec<String>, String> {
    page.items
        .iter()
        .map(|task| {
            Ok(task
                .get::<&str>("title")
                .map_err(text)?
                .unwrap_or("")
                .to_owned())
        })
        .collect()
}

fn status(space: &Space) {
    let s = space.status().current();
    println!("role: {}", s.role.map_or("none", role_name));
    println!("initial sync complete: {}", s.initial_sync_complete);
    println!("behind: {}", s.behind);
    println!("missing changes: {}", s.gaps.len());
    println!("parked: {:?}", s.parked);
    println!("rejected: {:?}", s.rejected);
    println!("refused by the relay: {}", s.refused.len());
    println!("key exposed to a former member: {}", s.exposed);
    println!("waiting for a key: {}", s.waiting_for_key);
}

fn role_name(role: Role) -> &'static str {
    match role {
        Role::Reader => "reader",
        Role::Writer => "writer",
        Role::Admin => "admin",
    }
}

/// The digits of a fingerprint as a person types it, blocks or not.
fn digits(text: &str) -> String {
    text.chars().filter(char::is_ascii_digit).collect()
}

fn text(error: impl std::fmt::Display) -> String {
    error.to_string()
}
