//! A late removal across islands, played end to end through the SDK and
//! the local relay (NOC-381; RFC 0008, "The cutoff", "Why it converges",
//! "What a new change can change" and "A member removed while offline who
//! keeps writing"; RFC 0006, "Control changes and changes that wait for
//! them"). Six members share one space: member 2 is its admin, member 1 a
//! writer, members 3 to 5 writers and member 6 a reader. Member 1 writes
//! three changes on an island, member 2 removes it with a cutoff below
//! them, and members 5 and 6 come back only after everything else. In
//! either order of arrival, every copy ends with the same membership
//! verdict for every change it holds, and the same view.
//!
//! An island is a database whose sync is paused (RFC 0009, "Sync and
//! status"): it writes and folds locally and sends the relay nothing.

mod order_a;
mod order_b;

use std::collections::BTreeMap;
use std::time::Duration;

use ncdb_core::ids::ChangeId as CoreChangeId;
use ncdb_core::log::{Position, RejectReason, State};
use ncdb_core::view::Row;

use super::support::*;
use crate::db::{Db, lock};
use crate::id::{MemberId, ObjectId};
use crate::members::{AddMember, Role};
use crate::space::Space;
use crate::status::RejectReason as PublicReject;
use crate::value::Value;

/// The member that is removed.
const REMOVED: Who = 1;
/// The admin, who creates the space and removes member 1.
const ADMIN: Who = 2;

/// The six databases, indexed by member number minus one, and the space
/// as each of them holds it.
pub(super) struct Six {
    pub dbs: Vec<Db>,
    pub spaces: Vec<Space>,
    /// The task the admin wrote before anyone went on an island.
    pub seed: ObjectId,
}

impl Six {
    /// The database of member `who`.
    pub fn db(&self, who: Who) -> &Db {
        &self.dbs[usize::from(who) - 1]
    }

    /// The space as member `who` holds it.
    pub fn space(&self, who: Who) -> &Space {
        &self.spaces[usize::from(who) - 1]
    }

    /// Pauses the sync of each of `members`: they go on an island.
    pub async fn island(&self, members: &[Who]) {
        for who in members {
            self.db(*who).sync().pause().await;
        }
    }

    /// Resumes the sync of each of `members`, and waits until each is
    /// connected and confirmed.
    pub async fn back(&self, members: &[Who]) {
        for who in members {
            self.db(*who).sync().resume().await;
        }
        for who in members {
            synced(self.db(*who)).await;
        }
    }

    /// Member 1's three changes on its island: #1 creates task A, #2
    /// creates task B, #3 sets A's title again. Returns A, B and the three
    /// change ids.
    pub fn removed_writes(&self) -> (ObjectId, ObjectId, Vec<CoreChangeId>) {
        let space = self.space(REMOVED);
        let one = now(space.create("tasks", [("title", Value::from("A by member 1"))])).unwrap();
        let two = now(space.create("tasks", [("title", Value::from("B by member 1"))])).unwrap();
        let (a, b) = (one.ids[0], two.ids[0]);
        let three = now(space.update("tasks", a, [("title", Value::from("A again"))])).unwrap();
        let changes = [one, two, three].iter().map(|w| w.change.0).collect();
        (a, b, changes)
    }

    /// Member 2 removes member 1, with the highest sequence of member 1 it
    /// holds as the cutoff (RFC 0008, "The cutoff").
    pub fn remove_member_1(&self) {
        let id = MemberId(lock(&self.db(REMOVED).inner).member.key().id());
        now(self.space(ADMIN).members().remove(id)).unwrap();
    }

    /// Waits until every one of `members` holds every change of the
    /// relay's log of the space.
    pub async fn hold_the_relay_log(&self, relay: &LocalRelay, members: &[Who]) {
        let space = self.space(ADMIN).id().0;
        let log = relay.log(space).await;
        eventually("every member holds the relay's log", || {
            members.iter().all(|who| {
                let inner = lock(&self.db(*who).inner);
                let held = inner.view.log(space).expect("the space's log");
                log.iter().all(|id| held.state(*id).is_some())
            })
        })
        .await;
    }
}

/// Opens the six databases on `relay`, has member 2 create the space with
/// one task and add the others, and waits until all six hold the whole
/// log.
pub(super) async fn six(relay: &LocalRelay) -> Six {
    let mut dbs = Vec::new();
    for who in 1..=6 {
        dbs.push(open(who, &relay.url()).await);
    }
    let admin = &dbs[usize::from(ADMIN) - 1];
    // The removal rotates the key, so the admin's SDK writes no merge
    // rotation; the delay keeps it so if a test ever leaves a generation
    // exposed (RFC 0008, "Two removals that rotate the key at once").
    crate::sync::set_merge_delay(admin, Duration::from_secs(3600));
    let on_admin = now(admin.spaces().create()).unwrap();
    let seed = now(on_admin.create("tasks", [("title", Value::from("seed"))]))
        .unwrap()
        .ids[0];
    for (who, db) in (1..=6).zip(&dbs) {
        let role = match who {
            ADMIN => continue,
            6 => Role::Reader,
            _ => Role::Writer,
        };
        let invitation = now(db.invitation());
        let options = AddMember {
            role,
            fingerprint: None,
        };
        now(on_admin.members().add(&invitation, options)).unwrap();
    }
    synced(admin).await;
    let space = on_admin.id();
    let mut spaces = Vec::new();
    for db in &dbs {
        let mut held = None;
        eventually("each member holds the space and the seed", || {
            held = now(db.spaces().get(space)).ok();
            held.as_ref()
                .is_some_and(|s| now(s.get("tasks", seed)).unwrap().is_some())
        })
        .await;
        spaces.push(held.unwrap());
    }
    let six = Six { dbs, spaces, seed };
    six.hold_the_relay_log(relay, &[1, 2, 3, 4, 5, 6]).await;
    for db in &six.dbs {
        // A member whose subscription has not caught up yet would hear
        // of a removal as `not_member` rather than `removed_member`
        // (NOC-184).
        eventually("the subscription caught up", || {
            lock(&db.inner).sync_live(space.0)
        })
        .await;
    }
    six
}

/// The membership verdict of every change member `who` holds, by change
/// id.
pub(super) fn verdicts(six: &Six, who: Who) -> BTreeMap<CoreChangeId, State> {
    let space = six.space(ADMIN).id().0;
    let inner = lock(&six.db(who).inner);
    let log = inner.view.log(space).expect("the space's log");
    log.after_in(Position::START, &mut inner.view.stored_frames())
        .map(|(_, change)| {
            let id = change.id();
            (id, log.state(id).expect("a held change").clone())
        })
        .collect()
}

/// What member `who`'s view holds of each of `objects` in `tasks`: the
/// core's row, with the merge state of every field, and the SDK's read.
pub(super) fn view(
    six: &Six,
    who: Who,
    objects: &[ObjectId],
) -> Vec<(Option<Row>, Option<crate::object::Object>)> {
    let space = six.space(who);
    objects
        .iter()
        .map(|id| {
            let row = lock(&six.db(who).inner)
                .view
                .get(space.id().0, "tasks", id.0)
                .unwrap();
            (row, now(space.get("tasks", *id)).unwrap())
        })
        .collect()
}

/// Checks that members `others` hold exactly the changes of `reference`,
/// with the same verdict for each, and the same view of `objects`.
pub(super) fn same_fold(six: &Six, reference: Who, others: &[Who], objects: &[ObjectId]) {
    let verdicts_of_reference = verdicts(six, reference);
    let view_of_reference = view(six, reference, objects);
    for who in others {
        assert_eq!(
            verdicts(six, *who),
            verdicts_of_reference,
            "member {who}'s verdicts differ from member {reference}'s"
        );
        assert_eq!(
            view(six, *who, objects),
            view_of_reference,
            "member {who}'s view differs from member {reference}'s"
        );
    }
}

/// Checks what member 1's copy holds against member `reference`'s. The
/// relay hands member 1 its own removal in the error that ends its pull or
/// subscription (RFC 0007, "The errors"; Carsten, 2026-11-20, NOC-383), so
/// member 1 folds it like any member: its own changes `written` above the
/// cutoff become rejected on its own copy, and the SDK reports them as
/// `rejected` with `notMember` (RFC 0008, "A member removed while offline
/// who keeps writing", "On the laptop itself"). On every change both hold,
/// the verdict is the same, and so is the view of every object.
pub(super) fn removed_agrees(
    six: &Six,
    reference: Who,
    written: &[CoreChangeId],
    objects: &[ObjectId],
) {
    let theirs = verdicts(six, reference);
    let ours = verdicts(six, REMOVED);
    for id in written {
        assert_eq!(
            ours[id],
            State::Rejected(RejectReason::NotMember),
            "member 1 rejects its own {id:?}"
        );
    }
    for (id, state) in ours.iter().filter(|(id, _)| !written.contains(id)) {
        if let Some(other) = theirs.get(id) {
            assert_eq!(state, other, "member 1's verdict on {id:?}");
        }
    }
    assert_eq!(view(six, REMOVED, objects), view(six, reference, objects));
    let status = six.space(REMOVED).status().current();
    assert_eq!(
        status.rejected.get(&PublicReject::NotMember),
        Some(&3),
        "member 1 is told which of its writes no member accepts"
    );
}
