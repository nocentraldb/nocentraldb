//! The log of RFC 0002, in memory: it appends and never rewrites, holds
//! each change once, and hands out its entries from a cursor in order.

#[path = "support/props.rs"]
mod props;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::change::{Change, Header, Kind};
use ncdb_core::ids::{MemberId, SpaceId};
use ncdb_core::log::{AppendError, Appended, Log, Position};
use proptest::prelude::*;

const SPACE: SpaceId = SpaceId([1; 16]);

fn change(space: SpaceId, sequence: u64) -> Change {
    let header = Header::new(
        space,
        MemberId([2; 32]),
        sequence,
        0,
        Kind::SEALED,
        vec![3],
        1,
    );
    Change::new(header, vec![4])
}

#[test]
fn appends_at_dense_positions_from_1() {
    let mut log = Log::new(SPACE);
    assert!(log.is_empty());
    assert_eq!(log.head(), Position::START);
    for sequence in 1..=3 {
        let appended = log.append(change(SPACE, sequence));
        assert_eq!(appended, Ok(Appended::New(Position(sequence))));
    }
    assert_eq!(log.len(), 3);
    assert!(!log.is_empty());
    assert_eq!(log.head(), Position(3));
    assert_eq!(log.get(Position(2)), Some(&change(SPACE, 2)));
    assert_eq!(log.get(Position::START), None);
    assert_eq!(log.get(Position(4)), None);
    assert_eq!(log.position_of(change(SPACE, 3).id()), Some(Position(3)));
    assert_eq!(log.position_of(change(SPACE, 9).id()), None);
}

#[test]
fn a_change_it_holds_is_not_appended_again() {
    let mut log = Log::new(SPACE);
    log.append(change(SPACE, 1)).unwrap();
    log.append(change(SPACE, 2)).unwrap();
    assert_eq!(
        log.append(change(SPACE, 1)),
        Ok(Appended::Duplicate(Position(1)))
    );
    assert_eq!(log.len(), 2);
    // A second copy with another signature has the same id: still the same
    // change, and the log keeps the bytes it appended first.
    let header = change(SPACE, 2).header().clone();
    let resigned = Change::new(header, vec![9, 9]);
    assert_eq!(log.append(resigned), Ok(Appended::Duplicate(Position(2))));
    assert_eq!(log.get(Position(2)), Some(&change(SPACE, 2)));
}

#[test]
fn a_change_of_another_space_is_refused() {
    let mut log = Log::new(SPACE);
    let other = SpaceId([9; 16]);
    assert_eq!(log.append(change(other, 1)), Err(AppendError::WrongSpace));
    assert!(log.is_empty());
    assert_eq!(log.space(), SPACE);
    assert_eq!(
        AppendError::WrongSpace.to_string(),
        "the change belongs to another space"
    );
}

#[test]
fn hands_out_the_entries_after_a_cursor_in_order() {
    let mut log = Log::new(SPACE);
    for sequence in 1..=5 {
        log.append(change(SPACE, sequence)).unwrap();
    }
    let after = |cursor| {
        log.after(Position(cursor))
            .map(|(position, change)| (position.0, change.header().sequence))
            .collect::<Vec<_>>()
    };
    assert_eq!(after(0), vec![(1, 1), (2, 2), (3, 3), (4, 4), (5, 5)]);
    assert_eq!(after(3), vec![(4, 4), (5, 5)]);
    assert_eq!(after(5), vec![]);
    assert_eq!(after(99), vec![]);
}

#[test]
fn the_log_appends_and_never_rewrites() {
    // Appends of changes, some of them again, some of another space; and
    // cursors that page through the log.
    let appends = proptest::collection::vec((1u64..20, any::<bool>()), 0..60);
    let pages = proptest::collection::vec(1usize..5, 1..20);
    props::check(256, (appends, pages), |(appends, pages)| {
        let mut log = Log::new(SPACE);
        let mut seen: Vec<Change> = Vec::new();
        for (sequence, other_space) in appends {
            let space = if other_space { SpaceId([9; 16]) } else { SPACE };
            let before: Vec<Vec<u8>> = log
                .after(Position::START)
                .map(|(_, c)| c.bytes().to_vec())
                .collect();
            let next = change(space, sequence);
            let result = log.append(next.clone());
            match seen.iter().position(|c| c.id() == next.id()) {
                _ if other_space => prop_assert_eq!(result, Err(AppendError::WrongSpace)),
                Some(at) => {
                    prop_assert_eq!(result, Ok(Appended::Duplicate(Position(at as u64 + 1))))
                }
                None => {
                    seen.push(next);
                    prop_assert_eq!(result, Ok(Appended::New(Position(seen.len() as u64))));
                }
            }
            // Every entry that was there keeps its position and its bytes.
            let after: Vec<Vec<u8>> = log
                .after(Position::START)
                .map(|(_, c)| c.bytes().to_vec())
                .collect();
            prop_assert_eq!(&after[..before.len()], &before[..]);
            prop_assert_eq!(log.len(), seen.len());
        }
        // Paging from a cursor hands out every entry once, in order.
        let mut cursor = Position::START;
        let mut paged = Vec::new();
        for size in pages.into_iter().cycle().take(seen.len() + 1) {
            for (position, change) in log.after(cursor).take(size) {
                prop_assert_eq!(position.0, cursor.0 + 1);
                cursor = position;
                paged.push(change.clone());
            }
        }
        prop_assert_eq!(paged, seen);
        prop_assert_eq!(cursor, log.head());
        Ok(())
    });
}
