//! Ids without a coordinator (RFC 0002): two members that generate ids
//! independently, offline, never produce the same one.

#[path = "support/props.rs"]
mod props;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::{BTreeMap, BTreeSet};

use ncdb_core::change::{Change, Header, Kind};
use ncdb_core::ids::{ChangeId, MemberId, ObjectId, SpaceId};
use proptest::prelude::*;

/// One member: generates its ids on its own, from the platform's secure
/// generator, as a device or an agent does offline.
fn generate(objects: usize, spaces: usize) -> (Vec<ObjectId>, Vec<SpaceId>) {
    let objects = (0..objects).map(|_| ObjectId::random()).collect();
    let spaces = (0..spaces).map(|_| SpaceId::random()).collect();
    (objects, spaces)
}

#[test]
fn two_members_generating_object_and_space_ids_do_not_collide() {
    let counts = (0usize..200, 0usize..20, 0usize..200, 0usize..20);
    props::check(64, counts, |(objects_a, spaces_a, objects_b, spaces_b)| {
        let (objects, spaces) = generate(objects_a, spaces_a);
        let (more_objects, more_spaces) = generate(objects_b, spaces_b);
        let objects: BTreeSet<ObjectId> = objects.into_iter().chain(more_objects).collect();
        let spaces: BTreeSet<SpaceId> = spaces.into_iter().chain(more_spaces).collect();
        prop_assert_eq!(objects.len(), objects_a + objects_b);
        prop_assert_eq!(spaces.len(), spaces_a + spaces_b);
        Ok(())
    });
}

#[test]
fn random_ids_use_all_their_bits() {
    // A generator that left a byte fixed would make collisions far likelier
    // than the RFC's birthday bound: over 256 ids, every byte position
    // takes more than one value.
    let ids: Vec<ObjectId> = (0..256).map(|_| ObjectId::random()).collect();
    for position in 0..16 {
        let values: BTreeSet<u8> = ids.iter().map(|id| id.0[position]).collect();
        assert!(values.len() > 1, "byte {position} never changes");
    }
}

/// A change one member writes: its sequence number and payload.
fn written() -> impl Strategy<Value = Vec<(u64, Vec<u8>)>> {
    proptest::collection::vec(
        (1u64..6, proptest::collection::vec(any::<u8>(), 0..4)),
        1..12,
    )
}

#[test]
fn two_members_writing_changes_do_not_collide() {
    // Member B also stands for a member restored from a backup: it reuses
    // sequence numbers, and even payloads, of member A.
    let strategy = (props::space_id(), written(), written());
    props::check(256, strategy, |(space, by_a, by_b)| {
        let a = MemberId([0xaa; 32]);
        let b = MemberId([0xbb; 32]);
        let header = |author, (sequence, payload): (u64, Vec<u8>)| {
            Header::new(space, author, sequence, 0, Kind::SEALED, payload, 1)
        };
        let headers = by_a
            .into_iter()
            .map(|w| header(a, w))
            .chain(by_b.into_iter().map(|w| header(b, w)));
        let mut ids: BTreeMap<ChangeId, Vec<u8>> = BTreeMap::new();
        for header in headers {
            let change = Change::new(header, vec![]);
            // An id repeats only for the very same header bytes.
            if let Some(bytes) = ids.insert(change.id(), change.header_bytes().to_vec()) {
                prop_assert_eq!(bytes, change.header_bytes().to_vec());
            }
        }
        Ok(())
    });
}
