//! Order A: the removal reaches the relay first. Member 2 is online and
//! removes member 1 while member 1 is on an island with changes #1 to #3.
//! Member 1 comes back, learns of its removal, and pushes anyway: the
//! relay refuses #1 to #3 (RFC 0008, "A member removed while offline who
//! keeps writing", "On the relay"), so no other copy ever holds them.
//! Members 5 and 6 come back last.

use ncdb_core::cbor::Value;
use ncdb_core::change::Change;

use super::super::relay_support::get;
use super::*;

#[tokio::test(flavor = "multi_thread")]
async fn a_removal_that_reaches_the_relay_first_keeps_the_late_changes_off_every_copy() {
    let relay = LocalRelay::start().await;
    let six = six(&relay).await;
    let space = six.space(ADMIN).id().0;

    six.island(&[REMOVED, 5, 6]).await;
    let (a, b, written) = six.removed_writes();
    six.remove_member_1();
    synced(six.db(ADMIN)).await;
    six.hold_the_relay_log(&relay, &[2, 3, 4]).await;

    // Member 1 comes back, and the SDK reports its removal.
    six.db(REMOVED).sync().resume().await;
    let status = synced(six.db(REMOVED)).await;
    assert_eq!(status.error.as_deref(), Some("not_member"));
    let listed = now(six.db(REMOVED).spaces().list()).unwrap();
    let listed = listed.iter().find(|s| s.id == six.space(ADMIN).id());
    assert_eq!(listed.expect("member 1 lists the space").role, None);

    // It pushes #1 to #3 anyway, through a raw connection of its member
    // key: the relay refuses each.
    let changes: Vec<Change> = {
        let inner = lock(&six.db(REMOVED).inner);
        let log = inner.view.log(space).unwrap();
        log.after_in(Position::START, &mut inner.view.stored_frames())
            .filter(|(_, change)| written.contains(&change.id()))
            .map(|(_, change)| change.into_owned())
            .collect()
    };
    assert_eq!(changes.len(), 3);
    let mut client = relay.client(REMOVED).await;
    let refs: Vec<&Change> = changes.iter().collect();
    for result in client.push_only(1, space, &refs).await {
        assert!(get(&result, 1).is_none(), "appended: {result:?}");
        // The uploader is no longer a member (RFC 0007, "The errors").
        assert_eq!(get(&result, 3), Some(&Value::Text("not_member".into())));
    }
    let log = relay.log(space).await;
    assert!(written.iter().all(|id| !log.contains(id)));

    // The latecomers.
    six.back(&[5, 6]).await;
    six.hold_the_relay_log(&relay, &[2, 3, 4, 5, 6]).await;

    let objects = [six.seed, a, b];
    same_fold(&six, ADMIN, &[3, 4, 5, 6], &objects);
    for who in 2..=6 {
        let held = verdicts(&six, who);
        assert!(
            written.iter().all(|id| !held.contains_key(id)),
            "member {who}"
        );
        assert_eq!(view(&six, who, &[a, b]), vec![(None, None), (None, None)]);
        let status = six.space(who).status().current();
        assert!(status.rejected.is_empty(), "member {who}: {status:?}");
    }
    removed_agrees(&six, ADMIN, &written, &objects);
}
