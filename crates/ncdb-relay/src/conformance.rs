//! The one backend test suite of RFC 0007 ("The backend interface"):
//! written against [`RelayStore`] alone, run against every backend. A
//! backend is interchangeable when it passes it.
//!
//! A backend's tests implement [`Backend`] and call
//! [`conformance_suite!`](crate::conformance_suite), which writes one test
//! per rule:
//!
//! ```text
//! ncdb_relay::conformance_suite!(fresh_sqlite_backend);
//! ```
//!
//! Each test gets a fresh, empty backend. Tests that run appends at the same
//! time open one handle per thread with [`Backend::connect`], the way two
//! relay processes share one storage.

use std::future::Future;
use std::pin::pin;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use std::thread::{self, Thread};

use ncdb_core::change::Kind;
use ncdb_core::ids::{ChangeId, MemberId, SpaceId};

use crate::store::{
    AppendOutcome, Effect, MemberRecord, NewEntry, PersonKeyId, PersonKeyRecord, Position,
    RelayStore,
};

/// A backend under test: storage that several handles can open at once.
pub trait Backend: Sync {
    /// A handle on the storage.
    type Store: RelayStore + 'static;

    /// A new handle on this backend's storage, as another relay process
    /// would open it.
    fn connect(&self) -> Self::Store;
}

/// Runs a future to completion on the current thread. The suite needs no
/// runtime: a backend's futures do their work when polled.
pub fn block_on<F: Future>(future: F) -> F::Output {
    struct Unpark(Thread);
    impl Wake for Unpark {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = Waker::from(Arc::new(Unpark(thread::current())));
    let mut context = Context::from_waker(&waker);
    let mut future = pin!(future);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => return output,
            Poll::Pending => thread::park(),
        }
    }
}

/// Writes one `#[test]` per rule of the suite, each on a fresh backend from
/// `$fresh`, a function that returns a [`Backend`].
#[macro_export]
macro_rules! conformance_suite {
    ($fresh:path) => {
        $crate::conformance_suite!(@tests $fresh;
            an_unknown_space_is_empty,
            an_append_at_head_0_creates_the_space,
            positions_are_dense_and_frames_exact,
            a_read_starts_after_the_cursor_and_stops_at_the_limit,
            an_append_on_a_stale_head_appends_nothing,
            a_held_change_id_is_a_duplicate_and_appends_nothing,
            spaces_do_not_share_positions_or_change_ids,
            concurrent_appends_to_one_space_stay_dense,
            concurrent_appends_of_one_change_give_it_one_position,
            effects_write_the_membership_table,
            effects_of_a_refused_append_are_not_applied,
            person_key_records_are_stored_replaced_and_deleted,
            the_epoch_is_shared_and_a_new_one_differs,
        );
    };
    (@tests $fresh:path; $($name:ident),* $(,)?) => {
        $(
            #[test]
            fn $name() {
                $crate::conformance::$name(&$fresh());
            }
        )*
    };
}

/// A space of the suite.
pub fn space(n: u8) -> SpaceId {
    SpaceId([n; 16])
}

/// A change id of the suite.
fn change(n: u32) -> ChangeId {
    let mut id = [0xC0; 32];
    id[..4].copy_from_slice(&n.to_be_bytes());
    ChangeId(id)
}

/// A new entry with change id `n` and a frame that tells it apart, of a
/// length that grows with `n`.
fn entry(n: u32) -> NewEntry {
    let mut frame = vec![0x83, 0x01];
    frame.extend(n.to_be_bytes());
    frame.extend(std::iter::repeat_n(0xA5, n as usize % 300));
    NewEntry {
        change: change(n),
        accepted_at: 1_700_000_000_000 + u64::from(n),
        author: MemberId([n as u8; 32]),
        kind: if n.is_multiple_of(5) {
            Kind::CONTROL
        } else {
            Kind::SEALED
        },
        frame,
    }
}

fn append<S: RelayStore>(
    store: &S,
    space: SpaceId,
    head: Position,
    entries: Vec<NewEntry>,
) -> AppendOutcome {
    block_on(store.append(space, head, entries, Vec::new())).expect("the backend appends")
}

/// Appends with retries until it lands, as the core does, and returns its
/// outcome.
fn append_retrying<S: RelayStore>(store: &S, space: SpaceId, entry: NewEntry) -> AppendOutcome {
    let mut head = block_on(store.head(space)).expect("a head").unwrap_or(0);
    loop {
        match append(store, space, head, vec![entry.clone()]) {
            AppendOutcome::HeadMoved(actual) => head = actual,
            outcome => return outcome,
        }
    }
}

fn read_all<S: RelayStore>(store: &S, space: SpaceId) -> Vec<crate::store::Entry> {
    block_on(store.read(space, 0, u32::MAX)).expect("the backend reads")
}

fn record(member: u8, current: bool, state: &[u8]) -> MemberRecord {
    MemberRecord {
        member: MemberId([member; 32]),
        current,
        state: state.to_vec(),
    }
}

fn person_key(id: u8, generation: u64) -> PersonKeyRecord {
    PersonKeyRecord {
        id: PersonKeyId([id; 16]),
        generation,
        writer: MemberId([id ^ 0x0F; 32]),
        wrapped: vec![id; 48 + id as usize],
        signature: vec![id ^ 0xFF; 64],
    }
}

/// The relay holds nothing of a space nobody appended to.
pub fn an_unknown_space_is_empty<B: Backend>(backend: &B) {
    let store = backend.connect();
    let space = space(1);
    assert_eq!(block_on(store.head(space)).unwrap(), None);
    assert!(read_all(&store, space).is_empty());
    assert_eq!(block_on(store.find(space, change(1))).unwrap(), None);
    let member = MemberId([1; 32]);
    assert_eq!(block_on(store.member(space, member)).unwrap(), None);
    assert!(block_on(store.members_of(member)).unwrap().is_empty());
    assert!(block_on(store.person_keys(space)).unwrap().is_empty());
}

/// An append at head 0 creates the space; the first change sits at 1.
pub fn an_append_at_head_0_creates_the_space<B: Backend>(backend: &B) {
    let store = backend.connect();
    let space = space(1);
    let outcome = append(&store, space, 0, vec![entry(1)]);
    assert_eq!(outcome, AppendOutcome::Appended { head: 1 });
    assert_eq!(block_on(store.head(space)).unwrap(), Some(1));
    assert_eq!(block_on(store.find(space, change(1))).unwrap(), Some(1));
}

/// Entries take the positions after the head in the order given, and come
/// back with every field, the frame byte for byte.
pub fn positions_are_dense_and_frames_exact<B: Backend>(backend: &B) {
    let store = backend.connect();
    let space = space(1);
    append(&store, space, 0, vec![entry(1), entry(2)]);
    let outcome = append(&store, space, 2, (3..=7).map(entry).collect());
    assert_eq!(outcome, AppendOutcome::Appended { head: 7 });
    let entries = read_all(&store, space);
    assert_eq!(entries.len(), 7);
    for (i, got) in entries.iter().enumerate() {
        let n = i as u32 + 1;
        let want = entry(n);
        assert_eq!(got.position, u64::from(n));
        assert_eq!(got.change, want.change);
        assert_eq!(got.accepted_at, want.accepted_at);
        assert_eq!(got.author, want.author);
        assert_eq!(got.kind, want.kind);
        assert_eq!(got.frame, want.frame, "the frame of position {n}");
        let found = block_on(store.find(space, want.change)).unwrap();
        assert_eq!(found, Some(u64::from(n)));
    }
}

/// A read returns the entries after its cursor, at most `limit`.
pub fn a_read_starts_after_the_cursor_and_stops_at_the_limit<B: Backend>(backend: &B) {
    let store = backend.connect();
    let space = space(1);
    append(&store, space, 0, (1..=10).map(entry).collect());
    let page = block_on(store.read(space, 3, 4)).unwrap();
    let positions: Vec<Position> = page.iter().map(|e| e.position).collect();
    assert_eq!(positions, [4, 5, 6, 7]);
    let tail = block_on(store.read(space, 8, 100)).unwrap();
    assert_eq!(tail.iter().map(|e| e.position).collect::<Vec<_>>(), [9, 10]);
    assert!(block_on(store.read(space, 10, 100)).unwrap().is_empty());
    assert!(block_on(store.read(space, 0, 0)).unwrap().is_empty());
}

/// An append whose expected head is not the head appends nothing and names
/// the head.
pub fn an_append_on_a_stale_head_appends_nothing<B: Backend>(backend: &B) {
    let store = backend.connect();
    let space = space(1);
    append(&store, space, 0, vec![entry(1), entry(2)]);
    assert_eq!(
        append(&store, space, 1, vec![entry(3)]),
        AppendOutcome::HeadMoved(2)
    );
    assert_eq!(
        append(&store, space, 0, vec![entry(3)]),
        AppendOutcome::HeadMoved(2)
    );
    let other = crate::conformance::space(2);
    assert_eq!(
        append(&store, other, 4, vec![entry(3)]),
        AppendOutcome::HeadMoved(0)
    );
    assert_eq!(block_on(store.head(other)).unwrap(), None);
    assert_eq!(block_on(store.head(space)).unwrap(), Some(2));
    assert_eq!(block_on(store.find(space, change(3))).unwrap(), None);
}

/// A change id the space's log holds is never appended again: the append
/// names its position and appends nothing, not even the other entries.
pub fn a_held_change_id_is_a_duplicate_and_appends_nothing<B: Backend>(backend: &B) {
    let store = backend.connect();
    let space = space(1);
    append(&store, space, 0, vec![entry(1), entry(2)]);
    let outcome = append(&store, space, 2, vec![entry(3), entry(1)]);
    assert_eq!(
        outcome,
        AppendOutcome::Duplicate {
            change: change(1),
            position: 1
        }
    );
    assert_eq!(block_on(store.head(space)).unwrap(), Some(2));
    assert_eq!(block_on(store.find(space, change(3))).unwrap(), None);
    assert_eq!(read_all(&store, space).len(), 2);
}

/// Each space has its own positions, and the same change id may sit in two
/// spaces.
pub fn spaces_do_not_share_positions_or_change_ids<B: Backend>(backend: &B) {
    let store = backend.connect();
    let (a, b) = (space(1), space(2));
    append(&store, a, 0, vec![entry(1), entry(2)]);
    assert_eq!(
        append(&store, b, 0, vec![entry(2)]),
        AppendOutcome::Appended { head: 1 }
    );
    assert_eq!(block_on(store.find(b, change(2))).unwrap(), Some(1));
    assert_eq!(block_on(store.find(a, change(2))).unwrap(), Some(2));
    assert_eq!(read_all(&store, b).len(), 1);
}

/// Handles that append to one space at the same time, each retrying on a
/// moved head, leave a log without a gap and without a change twice, which
/// every handle reads.
pub fn concurrent_appends_to_one_space_stay_dense<B: Backend>(backend: &B) {
    const HANDLES: u32 = 6;
    const EACH: u32 = 20;
    let space = space(1);
    thread::scope(|scope| {
        for handle in 0..HANDLES {
            scope.spawn(move || {
                let store = backend.connect();
                for i in 0..EACH {
                    let outcome = append_retrying(&store, space, entry(handle * EACH + i + 1));
                    assert!(matches!(outcome, AppendOutcome::Appended { .. }));
                }
            });
        }
    });
    for _ in 0..2 {
        let store = backend.connect();
        let entries = read_all(&store, space);
        let total = u64::from(HANDLES * EACH);
        assert_eq!(block_on(store.head(space)).unwrap(), Some(total));
        let positions: Vec<Position> = entries.iter().map(|e| e.position).collect();
        assert_eq!(positions, (1..=total).collect::<Vec<_>>());
        let mut changes: Vec<ChangeId> = entries.iter().map(|e| e.change).collect();
        changes.sort();
        changes.dedup();
        assert_eq!(changes.len() as u64, total);
    }
}

/// Handles that append the same change at the same time give it one
/// position, which every one of them learns.
pub fn concurrent_appends_of_one_change_give_it_one_position<B: Backend>(backend: &B) {
    const HANDLES: usize = 6;
    let space = space(1);
    append(&backend.connect(), space, 0, vec![entry(1)]);
    let outcomes: Vec<AppendOutcome> = thread::scope(|scope| {
        let handles: Vec<_> = (0..HANDLES)
            .map(|_| scope.spawn(|| append_retrying(&backend.connect(), space, entry(2))))
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let appended = outcomes
        .iter()
        .filter(|o| **o == AppendOutcome::Appended { head: 2 })
        .count();
    assert_eq!(appended, 1, "{outcomes:?}");
    let duplicate = AppendOutcome::Duplicate {
        change: change(2),
        position: 2,
    };
    assert_eq!(
        outcomes.iter().filter(|o| **o == duplicate).count(),
        HANDLES - 1
    );
    assert_eq!(read_all(&backend.connect(), space).len(), 2);
}

/// An append's effects write the space's membership table: a row replaces
/// the member's row, and `members_of` lists the spaces it currently
/// belongs to.
pub fn effects_write_the_membership_table<B: Backend>(backend: &B) {
    let store = backend.connect();
    let (a, b) = (space(1), space(2));
    let effects = vec![
        Effect::Member(record(1, true, b"one")),
        Effect::Member(record(2, true, b"two")),
    ];
    block_on(store.append(a, 0, vec![entry(1)], effects)).unwrap();
    let effects = vec![Effect::Member(record(1, true, b"one in b"))];
    block_on(store.append(b, 0, vec![entry(1)], effects)).unwrap();
    let one = MemberId([1; 32]);
    let got = block_on(store.member(a, one)).unwrap();
    assert_eq!(got, Some(record(1, true, b"one")));
    assert_eq!(block_on(store.members_of(one)).unwrap(), [a, b]);

    let effects = vec![Effect::Member(record(1, false, b"one, removed"))];
    block_on(store.append(a, 1, vec![entry(2)], effects)).unwrap();
    let got = block_on(backend.connect().member(a, one)).unwrap();
    assert_eq!(got, Some(record(1, false, b"one, removed")));
    assert_eq!(block_on(store.members_of(one)).unwrap(), [b]);
    let two = MemberId([2; 32]);
    assert_eq!(block_on(store.members_of(two)).unwrap(), [a]);
}

/// An append that appends nothing, on a moved head or a duplicate, changes
/// nothing in the membership table either.
pub fn effects_of_a_refused_append_are_not_applied<B: Backend>(backend: &B) {
    let store = backend.connect();
    let space = space(1);
    let effects = vec![Effect::Member(record(1, true, b"first"))];
    block_on(store.append(space, 0, vec![entry(1)], effects)).unwrap();
    let late = vec![Effect::Member(record(1, false, b"late"))];
    let outcome = block_on(store.append(space, 0, vec![entry(2)], late.clone())).unwrap();
    assert_eq!(outcome, AppendOutcome::HeadMoved(1));
    let outcome = block_on(store.append(space, 1, vec![entry(1)], late)).unwrap();
    assert!(matches!(outcome, AppendOutcome::Duplicate { .. }));
    let got = block_on(store.member(space, MemberId([1; 32]))).unwrap();
    assert_eq!(got, Some(record(1, true, b"first")));
}

/// Person-key records are stored per space in id order, replaced by their
/// id, and go only through a deletion.
pub fn person_key_records_are_stored_replaced_and_deleted<B: Backend>(backend: &B) {
    let store = backend.connect();
    let (a, b) = (space(1), space(2));
    block_on(store.put_person_keys(a, vec![person_key(3, 1), person_key(1, 1)])).unwrap();
    block_on(store.put_person_keys(b, vec![person_key(2, 1)])).unwrap();
    let got = block_on(store.person_keys(a)).unwrap();
    assert_eq!(got, [person_key(1, 1), person_key(3, 1)]);

    block_on(store.put_person_keys(a, vec![person_key(3, 2)])).unwrap();
    let got = block_on(backend.connect().person_keys(a)).unwrap();
    assert_eq!(got, [person_key(1, 1), person_key(3, 2)]);

    block_on(store.delete_person_keys(a, vec![PersonKeyId([1; 16]), PersonKeyId([9; 16])]))
        .unwrap();
    assert_eq!(block_on(store.person_keys(a)).unwrap(), [person_key(3, 2)]);
    assert_eq!(block_on(store.person_keys(b)).unwrap(), [person_key(2, 1)]);
}

/// Every handle sees the same epoch; a new epoch differs from the old one
/// and every handle sees it.
pub fn the_epoch_is_shared_and_a_new_one_differs<B: Backend>(backend: &B) {
    let (one, two) = (backend.connect(), backend.connect());
    let first = block_on(one.epoch()).unwrap();
    assert_eq!(block_on(two.epoch()).unwrap(), first);
    let second = block_on(two.new_epoch()).unwrap();
    assert_ne!(second, first);
    assert_eq!(block_on(one.epoch()).unwrap(), second);
    assert_eq!(block_on(backend.connect().epoch()).unwrap(), second);
}
