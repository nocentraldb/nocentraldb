//! RFC 0007, "Pull: read from a cursor" and "What the relay checks": the
//! reader is a current member, the cursor belongs to the relay's epoch and
//! lies at or below the head.

mod support;

use ncdb_core::membership::Role;
use ncdb_relay::conformance::block_on;
use ncdb_relay::store::{Epoch, RelayStore};
use ncdb_relay::{ErrorCode, Pushed, Relay};
use support::space::{Space, id};
use support::{config, push, relay};

#[test]
fn a_member_pulls_the_entries_after_its_cursor_with_the_head() {
    let (relay, _dir) = relay();
    let mut space = Space::new(0);
    let genesis = space.genesis.clone();
    let add = space.add(0, 1, Role::Reader, &[&genesis]);
    let one = space.sealed(0, Vec::new());
    let two = space.sealed(0, Vec::new());
    let changes = [&genesis, &add, &one, &two];
    push(&relay, 0, space.space, &changes);
    let epoch = block_on(relay.store().epoch()).unwrap();
    let pulled = block_on(relay.pull(id(1), space.space, epoch, 1, 2)).unwrap();
    assert_eq!(pulled.head, 4);
    let positions: Vec<u64> = pulled.entries.iter().map(|e| e.position).collect();
    assert_eq!(positions, [2, 3]);
    assert_eq!(pulled.entries[1].frame, one.bytes());
    assert_eq!(pulled.entries[1].change, one.id());
    let rest = block_on(relay.pull(id(1), space.space, epoch, 4, 10)).unwrap();
    assert!(rest.entries.is_empty());
    assert_eq!(rest.head, 4);
}

#[test]
fn a_reader_that_is_not_a_current_member_is_refused() {
    let (relay, _dir) = relay();
    let mut space = Space::new(0);
    let genesis = space.genesis.clone();
    let add = space.add(0, 1, Role::Reader, &[&genesis]);
    let remove = space.remove(0, 1, 0, &[&add]);
    push(&relay, 0, space.space, &[&genesis, &add, &remove]);
    let epoch = block_on(relay.store().epoch()).unwrap();
    for who in [1, 5] {
        let pulled = block_on(relay.pull(id(who), space.space, epoch, 0, 10));
        assert_eq!(pulled, Err(ErrorCode::NotMember), "member {who}");
    }
}

#[test]
fn a_space_the_relay_does_not_hold_is_unknown() {
    let (relay, _dir) = relay();
    let space = Space::new(0);
    let epoch = block_on(relay.store().epoch()).unwrap();
    let pulled = block_on(relay.pull(id(0), space.space, epoch, 0, 10));
    assert_eq!(pulled, Err(ErrorCode::UnknownSpace));
}

#[test]
fn a_cursor_of_another_epoch_is_refused() {
    let (relay, _dir) = relay();
    let space = Space::new(0);
    push(&relay, 0, space.space, &[&space.genesis]);
    let old = block_on(relay.store().epoch()).unwrap();
    let new = block_on(relay.store().new_epoch()).unwrap();
    let pulled = block_on(relay.pull(id(0), space.space, old, 0, 10));
    assert_eq!(pulled, Err(ErrorCode::EpochChanged));
    let pulled = block_on(relay.pull(id(0), space.space, Epoch([0; 16]), 0, 10));
    assert_eq!(pulled, Err(ErrorCode::EpochChanged));
    assert!(block_on(relay.pull(id(0), space.space, new, 0, 10)).is_ok());
}

#[test]
fn a_cursor_above_the_head_is_refused() {
    let (relay, _dir) = relay();
    let space = Space::new(0);
    push(&relay, 0, space.space, &[&space.genesis]);
    let epoch = block_on(relay.store().epoch()).unwrap();
    let pulled = block_on(relay.pull(id(0), space.space, epoch, 2, 10));
    assert_eq!(pulled, Err(ErrorCode::BadCursor));
}

/// Two relay processes on one database push to one space at the same
/// time: the core re-checks and retries, so every change gets one position
/// and the log stays dense.
#[test]
fn two_relays_on_one_backend_push_to_one_space_at_once() {
    let dir = support::TempDir::new();
    let mut space = Space::new(0);
    let genesis = space.genesis.clone();
    let add = space.add(0, 1, Role::Writer, &[&genesis]);
    let first = Relay::new(dir.store(), config());
    push(&first, 0, space.space, &[&genesis, &add]);
    let changes: Vec<Vec<_>> = (0..2)
        .map(|_| (0..15).map(|_| space.sealed(1, Vec::new())).collect())
        .collect();
    let space_id = space.space;
    std::thread::scope(|scope| {
        for batch in &changes {
            let relay = Relay::new(dir.store(), config());
            scope.spawn(move || {
                for change in batch {
                    let pushed = push(&relay, 1, space_id, &[change]);
                    assert!(matches!(pushed[..], [Pushed::Accepted(_)]), "{pushed:?}");
                }
            });
        }
    });
    let epoch = block_on(first.store().epoch()).unwrap();
    let pulled = block_on(first.pull(id(0), space_id, epoch, 0, 100)).unwrap();
    assert_eq!(pulled.head, 32);
    let positions: Vec<u64> = pulled.entries.iter().map(|e| e.position).collect();
    assert_eq!(positions, (1..=32).collect::<Vec<_>>());
}
