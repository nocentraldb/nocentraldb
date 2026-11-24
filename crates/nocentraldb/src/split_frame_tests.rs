//! The room a frame takes besides its payload's operations (NOC-421): a
//! part filled to its budget seals into a frame within the relay's limit,
//! and one byte more is refused up front.

use std::collections::BTreeSet;

use ncdb_core::cbor::Value as Cbor;
use ncdb_core::change::{Operation, WriterModel};
use ncdb_core::clock::Hlc;
use ncdb_core::ids::{ChangeId, ObjectId, SpaceId};
use ncdb_core::keys::{MemberKey, SpaceKey};

use super::*;

const WRITER: WriterModel = WriterModel {
    id: [0xee; 16],
    version: u32::MAX as u64,
};

/// An operation whose value is a text of `n` bytes.
fn operation(n: usize) -> Operation {
    Operation::new(
        ObjectId([7; 16]),
        "tasks".into(),
        Some("title".into()),
        1,
        Some(Cbor::Text("x".repeat(n))),
    )
}

/// The one operation that takes exactly the budget of a part that
/// depends on `heads` changes.
fn filling(heads: usize) -> Operation {
    let budget = budget(base_length(heads, WRITER));
    let mut scratch = Payload::new(Hlc(0), BTreeSet::new(), Vec::new());
    let empty = scratch.unpadded_length();
    let mut n = budget;
    loop {
        let (op, length) = measured(&mut scratch, empty, operation(n));
        match length.cmp(&budget) {
            std::cmp::Ordering::Equal => return op,
            std::cmp::Ordering::Greater => n -= length - budget,
            std::cmp::Ordering::Less => n += budget - length,
        }
    }
}

fn heads(n: usize) -> BTreeSet<ChangeId> {
    (0..n as u8).map(|i| ChangeId([i; 32])).collect()
}

#[test]
fn a_part_filled_to_its_budget_seals_within_the_relay_limit() {
    let member = MemberKey::generate();
    let key = SpaceKey::generate();
    for count in [1, 3] {
        let parts = plan(vec![filling(count)], base_length(count, WRITER)).unwrap();
        assert_eq!(parts.len(), 1);
        let sealing = Seal {
            member: &member,
            key: &key,
            space: SpaceId([1; 16]),
            generation: u64::MAX,
            sequence: u64::MAX,
            heads: heads(count),
            writer: WRITER,
        };
        let changes = seal_parts(parts, sealing, || Ok(Hlc(u64::MAX))).unwrap();
        let frame = changes[0].bytes().len();
        assert!(frame <= CHANGE_LIMIT, "{frame} bytes for {count} heads");
    }
}

#[test]
fn one_byte_above_the_budget_is_too_large() {
    let mut over = filling(1);
    let Some(Cbor::Text(text)) = over.value() else {
        unreachable!("a text value");
    };
    over = Operation::new(
        over.object,
        over.collection,
        over.field,
        over.op,
        Some(Cbor::Text(format!("{text}x"))),
    );
    let refused = plan(vec![over], base_length(1, WRITER)).unwrap_err();
    assert_eq!(refused.code(), ErrorCode::TooLarge);
}
