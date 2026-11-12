//! M-C13 of RFC 0008's convergence harness (Carsten, 2026-11-11, NOC-230):
//! a finalize wins over every concurrent take-back. Provisional shreds with
//! finalizes and take-backs from several devices of the account,
//! concurrent and in sequence, delivered in any order: every copy reaches
//! the same outcome, no copy destroys a key without an accepted finalize,
//! and none restores a person whose key a copy destroyed. Its broken
//! fixture for M-C12: a rule under which the decision that arrives last
//! wins, which diverges where `resolve` does not.

#[path = "support/space.rs"]
mod space;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::BTreeSet;

use ncdb_core::change::Change;
use ncdb_core::ids::ChangeId;
use ncdb_core::membership::{Entry, ErasureDecision, Resolved, Role, Verdict, resolve};

use space::{Space, Who, account, person};

/// Admins of the one account, each a device that may decide.
const ALICE: Who = 1;
const ERIN: Who = 5;
const FRANK: Who = 6;
/// A writer of the same account, who performs the erasure.
const BOB: Who = 2;
const DECIDERS: [Who; 3] = [ALICE, ERIN, FRANK];

/// How many sets of decisions, and how many orders of each, the test tries.
const CASES: u64 = 64;
const ORDERS: usize = 4;

/// SplitMix64, seeded from the case number, never from the OS, so a run
/// is the same on every machine and target.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// A one-account space with three admins and a writer; the writer holds
/// person 1's key and erases it, signed off by his own account. Returns
/// the space, its changes and the provisional shred.
fn provisional() -> (Space, Vec<Change>, Change) {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let bob = s.add(ALICE, BOB, Role::Writer, &[&g]);
    let erin = s.add(ALICE, ERIN, Role::Admin, &[&bob]);
    let frank = s.add(ALICE, FRANK, Role::Admin, &[&erin]);
    let key = s.person_key(BOB, 1, 0xc1, &[&frank]);
    let shred = s.shred(BOB, 1, &account(), &[&key]);
    (s, vec![g, bob, erin, frank, key, shred.clone()], shred)
}

/// Up to four decisions by random admins of the account, each a random
/// kind, each after the shred or after a decision made before it: so some
/// are concurrent and some in sequence. Returns them, and the ids of those
/// that finalize.
fn decisions(s: &mut Space, shred: &Change, rng: &mut Rng) -> (Vec<Change>, BTreeSet<ChangeId>) {
    let mut made: Vec<Change> = Vec::new();
    let mut finalizes = BTreeSet::new();
    for _ in 0..=rng.below(4) {
        let author = DECIDERS[rng.below(DECIDERS.len())];
        let finalize = rng.next().is_multiple_of(2);
        let after = match rng.below(made.len() + 1) {
            0 => shred.clone(),
            i => made[i - 1].clone(),
        };
        let entry = Entry::ErasureDecision(ErasureDecision {
            shred: shred.id(),
            finalize,
        });
        let decision = s.write(author, entry, &[&after]);
        if finalize {
            finalizes.insert(decision.id());
        }
        made.push(decision);
    }
    (made, finalizes)
}

fn shuffled(items: &[Change], rng: &mut Rng) -> Vec<Change> {
    let mut items = items.to_vec();
    for i in (1..items.len()).rev() {
        items.swap(i, rng.below(i + 1));
    }
    items
}

fn destroyed(state: &Resolved) -> bool {
    state
        .person_key(person(1))
        .is_some_and(|key| key.destroyed())
}

/// Whether the state accepts one of `finalizes` among `delivered`.
fn accepts_a_finalize(
    state: &Resolved,
    delivered: &[Change],
    finalizes: &BTreeSet<ChangeId>,
) -> bool {
    delivered
        .iter()
        .any(|d| finalizes.contains(&d.id()) && state.verdict(d.id()) == Some(&Verdict::Accepted))
}

#[test]
fn m_c13_every_copy_reaches_the_same_outcome_and_a_destroyed_key_stays_destroyed() {
    for seed in 0..CASES {
        let mut rng = Rng(seed);
        let (mut s, base, shred) = provisional();
        let (made, finalizes) = decisions(&mut s, &shred, &mut rng);
        let mut outcomes = BTreeSet::new();
        for _ in 0..ORDERS {
            let order = shuffled(&made, &mut rng);
            let mut was_destroyed = false;
            // Every prefix of the delivery is a copy at some moment.
            for n in 0..=order.len() {
                let state = resolve(s.space, base.iter().chain(&order[..n]));
                let now = destroyed(&state);
                assert!(
                    !now || accepts_a_finalize(&state, &order[..n], &finalizes),
                    "seed {seed}: a key destroyed without an accepted finalize"
                );
                let erased = state.person_key(person(1)).is_some_and(|key| key.erased());
                assert!(
                    !now || erased,
                    "seed {seed}: a copy shows a person whose key it destroyed"
                );
                assert!(
                    now || !was_destroyed,
                    "seed {seed}: a copy restored a person whose key it destroyed"
                );
                was_destroyed = now;
            }
            outcomes.insert(resolve(s.space, base.iter().chain(&order)).encode());
        }
        assert_eq!(outcomes.len(), 1, "seed {seed}: the copies differ");
    }
}

/// M-C12's broken fixture for M-C13: a rule under which the decision that
/// arrives last decides. For a finalize and a take-back that are
/// concurrent, the orders of arrival end in different outcomes, and one
/// order restores a person whose key the other destroyed; `resolve` gives
/// one outcome, final, in both.
#[test]
fn letting_the_last_decision_to_arrive_win_would_diverge() {
    let (mut s, base, shred) = provisional();
    let decide = |s: &mut Space, author, finalize| {
        let entry = Entry::ErasureDecision(ErasureDecision {
            shred: shred.id(),
            finalize,
        });
        s.write(author, entry, &[&shred])
    };
    let finalize = decide(&mut s, ALICE, true);
    let take_back = decide(&mut s, ERIN, false);
    let orders = [
        vec![finalize.clone(), take_back.clone()],
        vec![take_back.clone(), finalize.clone()],
    ];
    let last_wins: BTreeSet<bool> = orders
        .iter()
        .map(|order| order[1].id() == finalize.id())
        .collect();
    assert_eq!(last_wins.len(), 2, "the broken rule diverges");
    for order in &orders {
        assert!(destroyed(&resolve(s.space, base.iter().chain(order))));
    }
}
