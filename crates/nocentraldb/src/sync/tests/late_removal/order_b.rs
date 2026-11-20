//! Order B: member 1's changes reach the relay first. Member 2 removes
//! member 1 on its own island, with a cutoff below #1, while member 1
//! brings #1 to #3 to the relay, and members 3 and 4 fold them; member 3
//! then edits task B, which #2 created. When member 2 comes back with the
//! removal, the relay and members 3 and 4 reject #1 to #3 and un-fold
//! them (RFC 0008, "On members that received them directly"; RFC 0006,
//! "Un-folding"). Members 5 and 6 come back last.
//!
//! Member 3's edit stays accepted: its author is a member. Task B then
//! holds a field write and no create, which RFC 0004 ("Creating and
//! deleting an object") keeps, as it keeps any write to an object whose
//! create has not arrived, and RFC 0006 makes the view equal that of a
//! copy that never folded #2. The test pins how every copy reads it.

use ncdb_core::object::Object as CoreObject;

use super::*;
use crate::error::ErrorCode;

#[tokio::test(flavor = "multi_thread")]
async fn a_removal_that_reaches_the_relay_last_un_folds_the_late_changes_on_every_copy() {
    let relay = LocalRelay::start().await;
    let six = six(&relay).await;
    let space = six.space(ADMIN).id().0;

    six.island(&[REMOVED, ADMIN, 5, 6]).await;
    let (a, b, written) = six.removed_writes();
    six.remove_member_1();

    // Member 1 comes back first; the relay accepts #1 to #3, and members
    // 3 and 4 fold them.
    six.back(&[REMOVED]).await;
    six.hold_the_relay_log(&relay, &[3, 4]).await;
    for who in [3, 4] {
        let held = verdicts(&six, who);
        for id in &written {
            assert!(
                matches!(held[id], State::Folded { .. }),
                "member {who}: {:?}",
                held[id]
            );
        }
        assert!(now(six.space(who).get("tasks", b)).unwrap().is_some());
    }

    // Member 3 edits task B, which #2 created.
    let edit = now(six
        .space(3)
        .update("tasks", b, [("title", Value::from("B by member 3"))]))
    .unwrap()
    .change
    .0;
    synced(six.db(3)).await;
    six.hold_the_relay_log(&relay, &[4]).await;

    // Member 2 comes back with the removal.
    six.back(&[ADMIN]).await;
    six.hold_the_relay_log(&relay, &[2, 3, 4]).await;
    let log = relay.log(space).await;
    assert!(
        written.iter().all(|id| log.contains(id)),
        "the relay never removes an entry from its log"
    );
    for who in [2, 3, 4] {
        let held = verdicts(&six, who);
        for id in &written {
            assert_eq!(
                held[id],
                State::Rejected(RejectReason::NotMember),
                "member {who}"
            );
        }
        assert!(matches!(held[&edit], State::Folded { .. }), "member {who}");
        // The SDK reports each change that moved.
        let status = six.space(who).status().current();
        assert_eq!(
            status.rejected.get(&PublicReject::NotMember),
            Some(&3),
            "member {who}"
        );
    }

    // Member 1 learns of its removal.
    eventually("member 1 hears of its removal", || {
        six.db(REMOVED).sync().current().error.is_some()
    })
    .await;
    assert_eq!(
        six.db(REMOVED).sync().current().error.as_deref(),
        Some("removed_member")
    );

    // The latecomers.
    six.back(&[5, 6]).await;
    six.hold_the_relay_log(&relay, &[2, 3, 4, 5, 6]).await;

    let objects = [six.seed, a, b];
    same_fold(&six, ADMIN, &[3, 4, 5, 6], &objects);
    for who in 2..=6 {
        let status = six.space(who).status().current();
        assert_eq!(
            status.rejected.get(&PublicReject::NotMember),
            Some(&3),
            "member {who}"
        );
        // Task A is gone. Task B keeps member 3's write to its title and
        // holds no create, so the view keeps the write and the SDK reads
        // no object.
        let read = view(&six, who, &[a, b]);
        assert_eq!(read[0], (None, None), "member {who}");
        let (row, object) = &read[1];
        let row = row.as_ref().expect("the view keeps member 3's write");
        assert_eq!(row.object, CoreObject::default(), "member {who}: no create");
        assert!(!row.object.exists());
        assert_eq!(row.fields.keys().collect::<Vec<_>>(), ["title"]);
        assert_eq!(object, &None, "member {who}");
    }
    // An app on member 3 that writes to task B again is told it is gone
    // (RFC 0009, "Writing": `notFound`).
    let refused = now(six
        .space(3)
        .update("tasks", b, [("title", Value::from("again"))]))
    .unwrap_err();
    assert_eq!(refused.code(), ErrorCode::NotFound);
    removed_agrees(&six, ADMIN, &written, &objects);
}
