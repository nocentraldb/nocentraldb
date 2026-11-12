//! The merge rotation (RFC 0008, "Two removals that rotate the key at
//! once", step 5; NOC-220): when the current generation is exposed, as
//! after a member left, an admin's SDK writes a key chain entry that lists
//! every head, with envelopes for the current members only, after a random
//! delay of up to a minute, unless another admin's merge rotation ends the
//! exposure first. A writer never rotates. The tests set each database's
//! longest delay (`set_merge_delay`).

use std::time::Duration;

use super::support::*;
use crate::Db;
use crate::members::{AddMember, Role};
use crate::space::Space;

/// A delay longer than any test: an admin with it does not rotate while
/// the test runs.
const AN_HOUR: Duration = Duration::from_secs(3600);

/// The current generation of `space` as `db` resolved it, and whether it
/// is exposed.
fn current(db: &Db, space: &Space) -> Option<(u64, bool)> {
    let inner = crate::db::lock(&db.inner);
    let resolved = inner.spaces.get(&space.id().0)?.checks.resolved();
    let generation = resolved.current_generation()?;
    Some((generation, resolved.exposed(generation)))
}

/// Whether `db` has a merge rotation of `space` waiting for its delay.
fn pending(db: &Db, space: &Space) -> bool {
    let inner = crate::db::lock(&db.inner);
    let sync = inner.sync.as_ref().expect("a database that syncs");
    sync.merges.contains_key(&space.id().0)
}

/// Each database's sync status, and the current generation of `space` it
/// holds with whether it is exposed and whether a merge rotation waits: the
/// diagnosis of a wait that runs out (NOC-276).
fn report(dbs: &[(&str, &Db)], space: Option<crate::SpaceId>) -> String {
    dbs.iter()
        .map(|(name, db)| {
            let status = db.sync().current();
            let inner = crate::db::lock(&db.inner);
            let state = space.and_then(|space| inner.spaces.get(&space.0));
            let generation = state.and_then(|state| {
                let resolved = state.checks.resolved();
                let generation = resolved.current_generation()?;
                Some((generation, resolved.exposed(generation)))
            });
            let merge = space.is_some_and(|space| {
                inner
                    .sync
                    .as_ref()
                    .is_some_and(|sync| sync.merges.contains_key(&space.0))
            });
            format!(
                "{name}: {:?}, unconfirmed {}, refused {}, error {:?}; generation (id, exposed) {generation:?}, merge waiting {merge}",
                status.connection, status.unconfirmed, status.refused, status.error
            )
        })
        .collect::<Vec<_>>()
        .join("\n  ")
}

/// [`synced`] of `db`, which names the call and reports every database of
/// `dbs` when it runs out.
async fn synced_named(what: &str, db: &Db, dbs: &[(&str, &Db)], space: Option<crate::SpaceId>) {
    let start = std::time::Instant::now();
    loop {
        let status = db.sync().current();
        if status.connection == crate::Connection::Connected && status.unconfirmed == 0 {
            return;
        }
        assert!(
            start.elapsed() < Duration::from_secs(20),
            "{what}: not synced in time\n  {}",
            report(dbs, space)
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

async fn add(admin: &Space, member: &Db, role: Role) {
    let options = AddMember {
        role,
        fingerprint: None,
    };
    now(admin.members().add(&now(member.invitation()), options)).unwrap();
}

/// `member`'s handle on `space`, once it joined it through the relay.
async fn joined(member: &Db, space: &Space) -> Space {
    let mut handle = Err(());
    eventually("the member joined the space", || {
        handle = now(member.spaces().get(space.id())).map_err(|_| ());
        handle.is_ok()
    })
    .await;
    handle.unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn after_a_leave_the_admin_rotates_and_the_generation_is_not_exposed() {
    let relay = LocalRelay::start().await;
    let admin = open(1, &relay.url()).await;
    let member = open(2, &relay.url()).await;
    crate::sync::set_merge_delay(&admin, Duration::ZERO);
    synced(&member).await;
    let project = now(admin.spaces().create()).unwrap();
    add(&project, &member, Role::Writer).await;
    synced(&admin).await;
    let on_member = joined(&member, &project).await;
    let (before, _) = current(&admin, &project).unwrap();

    now(on_member.leave()).unwrap();
    synced(&member).await;

    eventually("the admin rotated away from the exposed generation", || {
        current(&admin, &project).is_some_and(|(now, exposed)| now != before && !exposed)
    })
    .await;
    assert!(!project.status().current().exposed);
    // The leaver holds no key of the new generation.
    let (after, _) = current(&admin, &project).unwrap();
    eventually("the relay holds the rotation", || {
        now(relay.log(project.id().0)).len() > 4
    })
    .await;
    let inner = crate::db::lock(&member.inner);
    let state = &inner.spaces[&project.id().0];
    assert!(state.checks.space_key(after).is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn two_admins_converge_on_one_generation_that_is_not_exposed() {
    let relay = LocalRelay::start().await;
    let first = open(1, &relay.url()).await;
    let second = open(2, &relay.url()).await;
    let leaver = open(3, &relay.url()).await;
    crate::sync::set_merge_delay(&first, Duration::ZERO);
    crate::sync::set_merge_delay(&second, Duration::ZERO);
    let dbs = [("first", &first), ("second", &second), ("leaver", &leaver)];
    synced_named("second before the space", &second, &dbs, None).await;
    synced_named("leaver before the space", &leaver, &dbs, None).await;
    let project = now(first.spaces().create()).unwrap();
    let space = Some(project.id());
    add(&project, &second, Role::Admin).await;
    add(&project, &leaver, Role::Writer).await;
    synced_named("first after the adds", &first, &dbs, space).await;
    let on_second = joined(&second, &project).await;
    let on_leaver = joined(&leaver, &project).await;
    let (before, _) = current(&first, &project).unwrap();

    now(on_leaver.leave()).unwrap();
    synced_named("leaver after its leave", &leaver, &dbs, space).await;

    let start = std::time::Instant::now();
    loop {
        let (a, b) = (current(&first, &project), current(&second, &on_second));
        if a == b && a.is_some_and(|(now, exposed)| now != before && !exposed) {
            break;
        }
        assert!(
            start.elapsed() < Duration::from_secs(20),
            "both admins hold one generation that is not exposed: not in time (before {before})\n  {}",
            report(&dbs, space)
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// How long until `db`'s merge rotation of `space` is due, if one waits.
fn due_in(db: &Db, space: &Space) -> Option<Duration> {
    let inner = crate::db::lock(&db.inner);
    let deadline = *inner.sync.as_ref()?.merges.get(&space.id().0)?;
    Some(deadline.saturating_duration_since(crate::sync::time::Instant::now()))
}

/// The delay a test sets is the deadline itself, not the longest of a
/// random draw (NOC-284): an admin a test gives an hour never rotates while
/// the test runs. Drawn at random up to an hour, its deadline fell within
/// the two seconds `a_writer_never_rotates` waits about once in 1,800 runs,
/// and the writer then held the admin's new generation.
#[tokio::test(flavor = "multi_thread")]
async fn a_delay_a_test_sets_is_the_deadline() {
    let relay = LocalRelay::start().await;
    let admin = open(1, &relay.url()).await;
    let leaver = open(3, &relay.url()).await;
    crate::sync::set_merge_delay(&admin, AN_HOUR);
    synced(&leaver).await;
    let project = now(admin.spaces().create()).unwrap();
    add(&project, &leaver, Role::Writer).await;
    synced(&admin).await;
    let on_leaver = joined(&leaver, &project).await;

    now(on_leaver.leave()).unwrap();
    synced(&leaver).await;
    eventually("the admin has a merge rotation pending", || {
        pending(&admin, &project)
    })
    .await;

    let due = due_in(&admin, &project).expect("a deadline");
    assert!(
        due > AN_HOUR - Duration::from_secs(60),
        "the deadline is {due:?} away, not an hour"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_writer_never_rotates() {
    let relay = LocalRelay::start().await;
    let admin = open(1, &relay.url()).await;
    let writer = open(2, &relay.url()).await;
    let leaver = open(3, &relay.url()).await;
    // The admin waits longer than the test; the writer would not wait.
    crate::sync::set_merge_delay(&admin, AN_HOUR);
    crate::sync::set_merge_delay(&writer, Duration::ZERO);
    synced(&writer).await;
    synced(&leaver).await;
    let project = now(admin.spaces().create()).unwrap();
    add(&project, &writer, Role::Writer).await;
    add(&project, &leaver, Role::Writer).await;
    synced(&admin).await;
    let on_writer = joined(&writer, &project).await;
    let on_leaver = joined(&leaver, &project).await;
    let (before, _) = current(&admin, &project).unwrap();

    now(on_leaver.leave()).unwrap();
    synced(&leaver).await;
    eventually("the writer sees the exposure", || {
        current(&writer, &on_writer) == Some((before, true))
    })
    .await;
    tokio::time::sleep(Duration::from_secs(2)).await;
    assert_eq!(current(&writer, &on_writer), Some((before, true)));
    assert!(!pending(&writer, &on_writer));
    assert_eq!(current(&admin, &project), Some((before, true)));
}

#[tokio::test(flavor = "multi_thread")]
async fn another_admins_rotation_cancels_a_pending_one() {
    let relay = LocalRelay::start().await;
    let waiting = open(1, &relay.url()).await;
    let quick = open(2, &relay.url()).await;
    let leaver = open(3, &relay.url()).await;
    crate::sync::set_merge_delay(&waiting, AN_HOUR);
    crate::sync::set_merge_delay(&quick, AN_HOUR);
    synced(&quick).await;
    synced(&leaver).await;
    let project = now(waiting.spaces().create()).unwrap();
    add(&project, &quick, Role::Admin).await;
    add(&project, &leaver, Role::Writer).await;
    synced(&waiting).await;
    let on_quick = joined(&quick, &project).await;
    let on_leaver = joined(&leaver, &project).await;
    let (before, _) = current(&waiting, &project).unwrap();

    // Both admins see the exposure and wait.
    now(on_leaver.leave()).unwrap();
    synced(&leaver).await;
    eventually("both admins have a merge rotation pending", || {
        pending(&waiting, &project) && pending(&quick, &on_quick)
    })
    .await;

    // Then one rotates at once.
    crate::sync::set_merge_delay(&quick, Duration::ZERO);
    eventually(
        "the quick admin's rotation reached the waiting admin",
        || current(&waiting, &project).is_some_and(|(now, exposed)| now != before && !exposed),
    )
    .await;
    eventually("the waiting admin dropped its pending rotation", || {
        !pending(&waiting, &project)
    })
    .await;
    assert_eq!(current(&waiting, &project), current(&quick, &on_quick));
}
