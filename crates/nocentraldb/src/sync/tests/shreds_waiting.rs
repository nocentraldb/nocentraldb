//! The shreds of a space that are not stable yet (RFC 0009, "Sync and
//! status": `shredsWaiting`; RFC 0008, "Effects that wait until a change
//! is stable"): each with its age and, on an admin device after 7 days,
//! the accounts of the admin devices whose runs block it.
//!
//! Control changes carry no clock, so the age counts from when this device
//! first held the shred, which the view keeps (owner, 2026-11-11, NOC-218).
//! The changes of a simulated space are
//! folded into the database through the crate's internals, as syncing
//! folds what it pulls, and once through a relay.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ncdb_core::change::Change;
use ncdb_core::membership::Role as CoreRole;

use super::rotation::model_id;
use super::support::*;
use crate::db::{Db, OpenOptions, SpaceState};
use crate::id::ChangeId;
use crate::members::Fingerprint;
use crate::status::{ShredWaiting, SpaceStatus};

/// The senior admin, who creates the space.
const X: Who = 0;
/// The member whose database this is.
const ME: Who = 1;
/// The writer who shreds.
const W: Who = 2;
/// An admin of a second account, who signs the shred off.
const Y: Who = 3;

/// Eight days, past the 7 days after which an admin device names the
/// blocking accounts.
const EIGHT_DAYS: Duration = Duration::from_secs(8 * 24 * 60 * 60);

/// `ME`'s database, in memory, without a relay.
async fn mine() -> Db {
    let options = OpenOptions::new(LOCK).in_memory();
    Db::open_as(options, [ME; 32], [ME ^ 0x80; 32], account())
        .await
        .expect("the test lock opens")
}

/// Folds `change` into `db`, as syncing folds a change it pulls.
fn fold(db: &Db, change: &Change) {
    let mut inner = crate::db::lock(&db.inner);
    let space = change.header().space;
    let me = inner.member.key();
    let crate::db::Inner { view, spaces, .. } = &mut *inner;
    let state = spaces
        .entry(space)
        .or_insert_with(|| SpaceState::new(space, me));
    view.receive(change.bytes(), &mut state.checks)
        .expect("the change is appended");
    crate::status::note_shreds(view, &state.checks).expect("the view stores it");
}

/// Now, in milliseconds since the Unix epoch.
fn now_millis() -> u64 {
    let since = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    u64::try_from(since.as_millis()).unwrap()
}

/// A space `X` created, with `ME` in `role`, `Y` an admin of a second
/// account and `W` a writer, who announces a person key and shreds it
/// with `Y`'s account's sign-off. Returns the space, every change but the
/// shred, and the shred.
fn shredded(role: CoreRole) -> (space::Space, Vec<Change>, Change) {
    let mut space = space::Space::with_model(X, [5; 16], model_id());
    let genesis = space.genesis.clone();
    let own = space.envelope(X, X, &[&genesis]);
    let me = space.add(X, ME, role, &[&own]);
    let to_me = space.envelope(X, ME, &[&me]);
    space.certify_by(Y, space::other_account(1));
    let y = space.add(X, Y, CoreRole::Admin, &[&to_me]);
    let w = space.add(X, W, CoreRole::Writer, &[&y]);
    let entry = space.person_key(W, 1, 0xc1, &[&w]);
    let shred = space.shred(W, 1, &space::other_account(1), &[&entry]);
    let held = vec![genesis, own, me, to_me, y, w, entry];
    (space, held, shred)
}

/// Folds `changes` into `db`.
fn fold_all(db: &Db, changes: &[&Change]) {
    for change in changes {
        fold(db, change);
    }
}

fn status(db: &Db, space: &space::Space) -> SpaceStatus {
    handle(db, space.space).status().current()
}

/// Records that `db` first held `shred` `ago` before now.
fn held_since(db: &Db, shred: &Change, ago: Duration) {
    let at = now_millis() - u64::try_from(ago.as_millis()).unwrap();
    let mut inner = crate::db::lock(&db.inner);
    inner
        .view
        .note_shred_held(shred.header().space, shred.id(), at)
        .unwrap();
}

/// The fingerprint of an account.
fn fingerprint(account: &ed25519_dalek::SigningKey) -> Fingerprint {
    Fingerprint(account.verifying_key().to_bytes())
}

#[tokio::test]
async fn a_space_without_a_shred_has_none_waiting() {
    let db = mine().await;
    let (space, held, _) = shredded(CoreRole::Admin);
    fold_all(&db, &held.iter().collect::<Vec<_>>());

    assert_eq!(status(&db, &space).shreds_waiting, []);
}

#[tokio::test]
async fn a_new_shred_waits_with_its_age_and_names_no_blocking_account_yet() {
    let db = mine().await;
    let (space, held, shred) = shredded(CoreRole::Admin);
    fold_all(&db, &held.iter().chain([&shred]).collect::<Vec<_>>());

    let waiting = status(&db, &space).shreds_waiting;

    assert_eq!(waiting.len(), 1);
    assert_eq!(waiting[0].change, ChangeId(shred.id()));
    assert!(waiting[0].age < Duration::from_secs(60), "{waiting:?}");
    assert_eq!(waiting[0].blocking, []);
}

#[tokio::test]
async fn after_7_days_an_admin_device_names_each_blocking_account_once() {
    let db = mine().await;
    let (space, held, shred) = shredded(CoreRole::Admin);
    fold_all(&db, &held.iter().collect::<Vec<_>>());
    held_since(&db, &shred, EIGHT_DAYS);
    fold(&db, &shred);

    let waiting = status(&db, &space).shreds_waiting;

    // X, ME and Y block it: X and ME share one account, Y has its own.
    let mut accounts = vec![
        fingerprint(&space::account()),
        fingerprint(&space::other_account(1)),
    ];
    accounts.sort_by_key(|fingerprint| fingerprint.0);
    let [
        ShredWaiting {
            change,
            age,
            blocking,
            ..
        },
    ] = &waiting[..]
    else {
        panic!("one shred waits: {waiting:?}");
    };
    assert_eq!(*change, ChangeId(shred.id()));
    assert!(*age >= EIGHT_DAYS && *age < EIGHT_DAYS + Duration::from_secs(60));
    assert_eq!(*blocking, accounts);
}

#[tokio::test]
async fn a_writer_device_names_no_blocking_account_after_7_days() {
    let db = mine().await;
    let (space, held, shred) = shredded(CoreRole::Writer);
    fold_all(&db, &held.iter().collect::<Vec<_>>());
    held_since(&db, &shred, EIGHT_DAYS);
    fold(&db, &shred);

    let waiting = status(&db, &space).shreds_waiting;

    assert_eq!(waiting.len(), 1);
    assert!(waiting[0].age >= EIGHT_DAYS);
    assert_eq!(waiting[0].blocking, []);
}

#[tokio::test]
async fn the_age_counts_from_when_this_device_first_held_the_shred() {
    let db = mine().await;
    let (space, held, shred) = shredded(CoreRole::Admin);
    fold_all(&db, &held.iter().chain([&shred]).collect::<Vec<_>>());
    // Holding it again, as a later pull or a reopen does, keeps the age.
    held_since(&db, &shred, EIGHT_DAYS);
    fold(&db, &shred);

    assert!(status(&db, &space).shreds_waiting[0].age < Duration::from_secs(60));
}

#[tokio::test]
async fn a_shred_every_admin_wrote_after_is_stable_and_no_longer_waits() {
    let db = mine().await;
    let (mut space, held, shred) = shredded(CoreRole::Writer);
    let by_x = space.envelope(X, W, &[&shred]);
    let by_y = space.envelope(Y, W, &[&shred]);
    fold_all(
        &db,
        &held
            .iter()
            .chain([&shred, &by_x, &by_y])
            .collect::<Vec<_>>(),
    );

    assert_eq!(status(&db, &space).shreds_waiting, []);
}

#[tokio::test]
async fn an_admin_whose_run_the_device_does_not_hold_keeps_the_shred_waiting() {
    let db = mine().await;
    let (mut space, held, shred) = shredded(CoreRole::Writer);
    let by_x = space.envelope(X, W, &[&shred]);
    // Y's change after the shred follows one of Y's changes this device
    // never received.
    space.skip(Y, 1);
    let by_y = space.envelope(Y, W, &[&shred]);
    fold_all(
        &db,
        &held
            .iter()
            .chain([&shred, &by_x, &by_y])
            .collect::<Vec<_>>(),
    );

    let waiting = status(&db, &space).shreds_waiting;
    assert_eq!(waiting.len(), 1);
    assert_eq!(waiting[0].change, ChangeId(shred.id()));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_shred_pulled_from_the_relay_waits_from_when_it_arrived() {
    let relay = LocalRelay::start().await;
    let (space, held, shred) = shredded(CoreRole::Writer);
    let mut client = relay.client(X).await;
    push(
        &mut client,
        &held.iter().chain([&shred]).collect::<Vec<_>>(),
    )
    .await;
    let db = open(ME, &relay.url()).await;
    synced(&db).await;

    eventually("the shred waits", || {
        status(&db, &space)
            .shreds_waiting
            .iter()
            .any(|waiting| waiting.change == ChangeId(shred.id()))
    })
    .await;
    let waiting = status(&db, &space).shreds_waiting;
    assert!(waiting[0].age < Duration::from_secs(60));
    let at = crate::db::lock(&db.inner)
        .view
        .shred_held_at(space.space, shred.id())
        .unwrap();
    assert!(at.is_some_and(|at| at <= now_millis()));
}
