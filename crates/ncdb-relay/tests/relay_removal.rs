//! RFC 0007, "Subscribe" and "The errors": the relay hands a removed
//! member the control change that removes it, inside the error that ends
//! its pull or subscription, and nothing else of the space (Carsten,
//! 2026-11-20, NOC-383, option A).

mod support;

use std::collections::BTreeMap;

use ncdb_core::cbor::{self, Key, Value};
use ncdb_core::change::Change;
use ncdb_core::membership::Role;
use ncdb_relay::conformance::block_on;
use ncdb_relay::page;
use ncdb_relay::{ErrorCode, Pushed};
use support::space::{Space, id};
use support::{push, relay};

/// A space where 0 created it, added 1 as a writer, wrote a sealed change
/// and removed 1: the genesis, the add, the sealed change and the removal.
fn removed() -> (
    ncdb_relay::Relay<ncdb_relay::sqlite::SqliteStore>,
    support::TempDir,
    Space,
    Vec<Change>,
) {
    let (relay, dir) = relay();
    let mut space = Space::new(0);
    let genesis = space.genesis.clone();
    let add = space.add(0, 1, Role::Writer, &[&genesis]);
    let sealed = space.sealed(0, Vec::new());
    let remove = space.remove(0, 1, 0, &[&add]);
    let pushed = push(&relay, 0, space.space, &[&genesis, &add, &sealed, &remove]);
    assert!(pushed.iter().all(|p| matches!(p, Pushed::Accepted(_))));
    (relay, dir, space, vec![genesis, add, sealed, remove])
}

fn fields(bytes: &[u8]) -> BTreeMap<Key, Value> {
    match cbor::decode(bytes).expect("CBOR") {
        Value::Map(map) => map,
        other => panic!("not a map: {other:?}"),
    }
}

fn at(map: &BTreeMap<Key, Value>, key: u64) -> Option<&Value> {
    map.get(&Key::Unsigned(key))
}

#[test]
fn the_removal_of_a_former_member_is_the_control_change_that_removed_it() {
    let (relay, _dir, space, changes) = removed();
    let removal = block_on(relay.removal(id(1), space.space)).unwrap();
    let removal = removal.expect("member 1 was removed");
    assert_eq!(removal.position, 4);
    assert_eq!(removal.change, changes[3].id());
    assert_eq!(removal.frame, changes[3].bytes());
}

#[test]
fn a_current_member_or_a_stranger_has_no_removal() {
    let (relay, _dir, space, _) = removed();
    assert_eq!(block_on(relay.removal(id(0), space.space)).unwrap(), None);
    assert_eq!(block_on(relay.removal(id(5), space.space)).unwrap(), None);
}

#[test]
fn a_member_removed_twice_gets_the_removal_that_holds() {
    let (relay, _dir, mut space, changes) = removed();
    // A re-grant names a fresh generation (RFC 0008, "Validity in the
    // causal past").
    let first = space.first_generation;
    let (rotation, next) = space.rotate(0, &[first], &[&changes[3]]);
    let again = space.add_at(0, 1, Role::Writer, next, &[&rotation]);
    let remove = space.remove(0, 1, 0, &[&again]);
    let pushed = push(&relay, 0, space.space, &[&rotation, &again, &remove]);
    assert_eq!(
        pushed,
        [
            Pushed::Accepted(5),
            Pushed::Accepted(6),
            Pushed::Accepted(7)
        ]
    );
    let removal = block_on(relay.removal(id(1), space.space)).unwrap();
    assert_eq!(removal.expect("removed").change, remove.id());
}

#[test]
fn the_error_to_a_removed_member_carries_its_removal_and_nothing_else() {
    let (relay, _dir, space, changes) = removed();
    for code in [ErrorCode::NotMember, ErrorCode::RemovedMember] {
        let bytes = block_on(page::refusal(&relay, id(1), Some(7), space.space, code));
        let map = fields(&bytes);
        assert_eq!(at(&map, 1), Some(&Value::Text("error".into())));
        assert_eq!(at(&map, 2), Some(&Value::Unsigned(7)));
        assert_eq!(at(&map, 3), Some(&Value::Text(code.code().into())));
        assert_eq!(at(&map, 4), None);
        let Some(Value::Map(entry)) = at(&map, 5) else {
            panic!("no removal: {map:?}");
        };
        assert_eq!(at(entry, 1), Some(&Value::Unsigned(4)));
        assert_eq!(at(entry, 2), Some(&Value::Unsigned(1_700_000_000_000)));
        assert_eq!(
            at(entry, 3),
            Some(&Value::Bytes(changes[3].bytes().to_vec()))
        );
        assert_eq!(entry.len(), 3);
        assert_eq!(map.len(), 4);
    }
}

#[test]
fn the_error_to_a_stranger_or_of_another_code_carries_no_removal() {
    let (relay, _dir, space, _) = removed();
    let stranger = block_on(page::refusal(
        &relay,
        id(5),
        Some(1),
        space.space,
        ErrorCode::NotMember,
    ));
    assert_eq!(
        stranger,
        ncdb_relay::wire::error(Some(1), ErrorCode::NotMember)
    );
    let other = block_on(page::refusal(
        &relay,
        id(1),
        Some(2),
        space.space,
        ErrorCode::EpochChanged,
    ));
    assert_eq!(
        other,
        ncdb_relay::wire::error(Some(2), ErrorCode::EpochChanged)
    );
}

#[test]
fn a_removal_too_large_for_one_message_is_left_out() {
    let (_relay, _dir, _, changes) = removed();
    let entry = |frame: Vec<u8>| ncdb_relay::store::Entry {
        position: 4,
        change: changes[3].id(),
        accepted_at: 1,
        author: id(0),
        kind: changes[3].header().kind,
        frame,
    };
    let code = ErrorCode::RemovedMember;
    let fits = ncdb_relay::wire::removed(Some(1), code, &entry(changes[3].bytes().to_vec()));
    assert!(fits.is_some_and(|bytes| bytes.len() <= ncdb_relay::wire::MAX_MESSAGE));
    let large = vec![0; ncdb_relay::wire::MAX_INLINE + 1];
    assert_eq!(
        ncdb_relay::wire::removed(Some(1), code, &entry(large)),
        None
    );
}
