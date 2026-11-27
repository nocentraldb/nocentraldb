//! `ncdb/richText` meets RFC 0004's plugin contract C1 to C7 (RFC 0013,
//! "How rich text meets the plugin contract"), checked by the convergence
//! harness: members type, delete, format, split and join blocks, delete
//! blocks and set attributes, and sync now and then; copies receive it all
//! in random orders, with duplicates and malformed operations. The broken
//! fixtures, a fold that lets client ids collide and one that accepts any
//! client id, must be caught (C6). A member restored from a backup, which
//! writes again under ids it used before, is checked on its own: the
//! harness's C4 asks that a deletion name only what its author had seen,
//! and a deletion of a reused id names both writes of it.
#![cfg(ncdb_rich_text)]

mod rich_text_support;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::BTreeSet;

use ncdb_harness::{
    Author, Config, Generator, Member, Mergeable, Op, OpId, Outcome, check, digest,
};
use proptest::prelude::*;
use rich_text_support::harness::{Body, Copy, Edits, Raw, body, client, edit, member_doc};
use yrs::updates::decoder::Decode;
use yrs::{ReadTxn, StateVector, Transact, Update};

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
fn rich_text_meets_the_contract() {
    for seed in SEEDS {
        let result = check::<Copy, _>(&config(seed), &Edits);
        assert!(result.is_ok(), "{}", result.unwrap_err());
    }
}

#[test]
fn rich_text_meets_the_contract_with_more_members_copies_and_duplicates() {
    for seed in SEEDS {
        let result = check::<Copy, _>(
            &Config {
                members: 4,
                copies: 5,
                max_duplicates: 3,
                max_learned: 4,
                cases: 32,
                ..config(seed)
            },
            &Edits,
        );
        assert!(result.is_ok(), "{}", result.unwrap_err());
    }
}

/// C3: the same cases end in the same state bytes natively and on wasm32.
/// The digest was computed natively; the wasm32 run must give it too.
#[test]
fn c3_the_same_cases_end_in_the_same_state_bytes_on_every_target() {
    let digest = digest::<Copy, _>(
        &Config {
            cases: 16,
            ..config(0)
        },
        &[0, 1, 2],
        &Edits,
    );
    assert_eq!(
        digest,
        "9db184d19bd648c47690f542b422cf2f8ff0178803b0d123916af00daf3cc94b"
    );
}

/// Broken on purpose: a fold that applies every update as it arrives,
/// with no check of its client ids and no rule for the same id twice.
#[derive(Clone, Debug, Default)]
struct Unchecked {
    received: BTreeSet<OpId>,
    document: Vec<u8>,
}

impl Mergeable for Unchecked {
    type Change = Body;
    type Read = Vec<u8>;

    fn apply(&mut self, op: &Op<Body>) {
        if !self.received.insert(op.id) {
            return;
        }
        let doc = member_doc(1, &self.document);
        if let Ok(update) = Update::decode_v1(&op.body.update) {
            let _ = doc.transact_mut().apply_update(update);
        }
        self.document = doc
            .transact()
            .encode_state_as_update_v1(&StateVector::default());
    }

    fn state_bytes(&self) -> Vec<u8> {
        self.document.clone()
    }

    fn read(&self) -> Vec<u8> {
        self.document.clone()
    }

    fn outcome(&self, _: OpId) -> Outcome {
        Outcome::Visible
    }
}

/// Members whose client ids are drawn at random, from few values, as Yjs
/// draws them, so that two members now and then share one.
struct RandomIds;

impl Generator<Unchecked> for RandomIds {
    type Raw = (Raw, u8);

    fn raw(&self, _: Member) -> BoxedStrategy<(Raw, u8)> {
        (any::<Raw>(), 1u8..4).boxed()
    }

    fn make(&self, author: &Author<'_, Unchecked>, (raw, id): &(Raw, u8)) -> Body {
        let client = u64::from(*id);
        let update = edit(&member_doc(client, &author.copy.document), raw);
        body(author, update, client, [0; 8])
    }
}

/// Members that now and then write under another member's client id,
/// which a fold that accepts any client id lets through.
struct Forged;

impl Generator<Unchecked> for Forged {
    type Raw = (Raw, Option<u8>);

    fn raw(&self, _: Member) -> BoxedStrategy<(Raw, Option<u8>)> {
        (any::<Raw>(), proptest::option::weighted(0.3, 0u8..3)).boxed()
    }

    fn make(&self, author: &Author<'_, Unchecked>, (raw, other): &(Raw, Option<u8>)) -> Body {
        let client = client(other.unwrap_or(author.member));
        let update = edit(&member_doc(client, &author.copy.document), raw);
        body(author, update, client, [0; 8])
    }
}

fn expect_c1<G: Generator<Unchecked>>(generator: &G) {
    for seed in SEEDS {
        let violation = check::<Unchecked, _>(&config(seed), generator)
            .expect_err("the harness passed a broken fold");
        let report = violation.to_string();
        assert!(report.starts_with("C1 "), "expected C1, got: {report}");
    }
}

#[test]
fn c6_a_fold_that_lets_random_client_ids_collide_is_caught() {
    expect_c1(&RandomIds);
}

#[test]
fn c6_a_fold_that_accepts_any_client_id_is_caught() {
    expect_c1(&Forged);
}

#[test]
fn the_same_generated_edits_are_refused_under_another_members_client_id() {
    // What the broken fold lets through, the core parks: an edit under
    // another member's client id does not decode.
    let doc = member_doc(client(1), &[]);
    let raw: Raw = (0, 0, 0, 0);
    let op = Op {
        id: 0,
        author: 0,
        body: Body {
            stamp: 0,
            update: edit(&doc, &raw),
            clients: vec![(client(1), rich_text_support::harness::NONCE)],
        },
    };
    let mut copy = Copy::default();
    copy.apply(&op);
    assert_eq!(copy.outcome(0), Outcome::Parked);
}

/// Every order of `ops`, by Heap's algorithm.
fn orders(n: usize) -> Vec<Vec<usize>> {
    fn heap(k: usize, a: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
        if k <= 1 {
            out.push(a.clone());
            return;
        }
        for i in 0..k {
            heap(k - 1, a, out);
            let j = if k.is_multiple_of(2) { i } else { 0 };
            a.swap(j, k - 1);
        }
    }
    let mut out = Vec::new();
    heap(n, &mut (0..n).collect(), &mut out);
    out
}

#[test]
fn a_member_restored_from_a_backup_converges_in_every_order() {
    use rich_text_support::harness::{NONCE, edit_text};
    use rich_text_support::{apply, type_text};
    let alice = client(0);
    let bob = client(1);
    let a = member_doc(alice, &[]);
    let first = edit_text(&a, "Alice first.");
    let typed = type_text(&a, 0, 12, " More.");
    // Bob sees Alice's first write and types after it.
    let b = member_doc(bob, &first);
    let by_bob = type_text(&b, 0, 0, "Bob: ");
    // Alice, restored from a backup taken before she wrote, writes again
    // under the same client id and clocks; Carol sees only that write and
    // deletes part of it.
    let restored = member_doc(alice, &[]);
    let again = edit_text(&restored, "Alice again.");
    let c = member_doc(client(2), &again);
    let by_carol = rich_text_support::delete_text(&c, 0, 0, 6);
    apply(&c, &by_bob);
    let ops: Vec<Op<Body>> = [
        (0, 0, first, Some(alice)),
        (0, 1, typed, Some(alice)),
        (1, 2, by_bob, Some(bob)),
        (0, 3, again, Some(alice)),
        (2, 4, by_carol, None),
    ]
    .into_iter()
    .map(|(author, stamp, update, client)| Op {
        id: stamp as OpId,
        author,
        body: Body {
            stamp,
            update,
            clients: client.map(|c| vec![(c, NONCE)]).unwrap_or_default(),
        },
    })
    .collect();
    let mut ends = Vec::new();
    for order in orders(ops.len()) {
        let mut copy = Copy::default();
        for i in order {
            copy.apply(&ops[i]);
            copy.apply(&ops[i]);
        }
        let outcomes: Vec<Outcome> = ops.iter().map(|op| copy.outcome(op.id)).collect();
        ends.push((copy.state_bytes(), outcomes, copy.read()));
    }
    assert!(ends.iter().all(|end| *end == ends[0]));
    // The earlier write keeps the ids, and Carol's deletion of them takes
    // its characters; the restored write parks.
    assert_eq!(ends[0].1[0], Outcome::Replaced(4));
    assert_eq!(ends[0].1[3], Outcome::Parked);
}
