//! `ncdb/richText` meets RFC 0004's plugin contract C1 to C7 with
//! un-folding (RFC 0013, "Un-folding a rejected change"): members edit as
//! in `rich_text_contract.rs` and now and then reject an earlier
//! operation, as a removal that arrives late rejects its author's change.
//! Copies receive edits and rejections in random orders, with duplicates
//! and malformed operations. Two broken folds must be caught (C6): one
//! that un-folds by leaving the rejected update out, and one that places a
//! rejected edit only when it had folded it before the rejection.
#![cfg(ncdb_rich_text)]

mod rich_text_support;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_harness::{Config, Property, check, digest};
use rich_text_support::unfolding::{
    EditsAndRejections, LEAVES_OUT, ONLY_WHAT_IT_FOLDED, Unfolding,
};

/// The seeds every check runs with.
const SEEDS: core::ops::Range<u64> = 0..8;

fn config(seed: u64) -> Config {
    Config {
        seed,
        cases: 64,
        ..Config::default()
    }
}

#[test]
fn rich_text_meets_the_contract_with_rejected_changes() {
    for seed in SEEDS {
        let result = check::<Unfolding, _>(&config(seed), &EditsAndRejections);
        assert!(result.is_ok(), "{}", result.unwrap_err());
    }
}

#[test]
fn rich_text_meets_the_contract_with_rejected_changes_more_members_copies_and_duplicates() {
    for seed in SEEDS {
        let result = check::<Unfolding, _>(
            &Config {
                members: 4,
                copies: 5,
                max_duplicates: 3,
                max_learned: 4,
                cases: 32,
                ..config(seed)
            },
            &EditsAndRejections,
        );
        assert!(result.is_ok(), "{}", result.unwrap_err());
    }
}

/// C3: the same cases end in the same state bytes natively and on wasm32.
/// The digest was computed natively; the wasm32 run must give it too.
#[test]
fn c3_with_rejected_changes_the_same_cases_end_in_the_same_state_bytes_on_every_target() {
    let digest = digest::<Unfolding, _>(
        &Config {
            cases: 16,
            ..config(0)
        },
        &[0, 1, 2],
        &EditsAndRejections,
    );
    assert_eq!(
        digest,
        "21ba9bbac19ba0c096d13e410dac0e09d61ad9c9dea3e7c7a10bce05abcf7a07"
    );
}

#[test]
fn c6_a_fold_that_leaves_the_rejected_update_out_is_caught() {
    let caught = SEEDS.into_iter().find_map(|seed| {
        check::<Unfolding<LEAVES_OUT>, _>(&config(seed), &EditsAndRejections).err()
    });
    let violation = caught.expect("the broken fold is caught");
    assert!(
        matches!(violation.property, Property::C1 | Property::C4),
        "{violation}"
    );
}

#[test]
fn c6_a_fold_that_places_only_what_it_folded_before_the_rejection_is_caught() {
    let caught = SEEDS.into_iter().find_map(|seed| {
        check::<Unfolding<ONLY_WHAT_IT_FOLDED>, _>(&config(seed), &EditsAndRejections).err()
    });
    let violation = caught.expect("the broken fold is caught");
    assert_eq!(violation.property, Property::C1, "{violation}");
}
