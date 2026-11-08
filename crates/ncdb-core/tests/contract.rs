//! The convergence harness checks the plugin contract C1 to C7 of RFC 0004
//! (`docs/rfc/0004-merge-rules-basic-types.md`, "The plugin contract"). A toy
//! register meets the contract and is the positive control; for each checked
//! property a broken variant of it violates exactly that property, and the
//! harness must report it (C6).

mod support;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use proptest::prelude::*;
use support::convergence::{self, Author, Config, Generator, Member, Mergeable, Op, OpId, Outcome};

/// One write to the register: its stamp, its value and the writes its author
/// had seen, which it replaces.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Write {
    stamp: u64,
    value: u8,
    seen: BTreeSet<OpId>,
}

/// The positive control: a register that keeps every write, as RFC 0004's
/// basic values do. A write is live until a write whose author had seen it
/// arrives; the live write last in stamp order (stamp, author, id) wins and
/// the other live writes are conflicts. Its state is a pure function of the
/// set of writes received.
#[derive(Clone, Debug, Default)]
struct Register {
    writes: BTreeMap<OpId, (Member, Write)>,
}

impl Register {
    /// The first write, in id order, that replaces `id`.
    fn replacer(&self, id: OpId) -> Option<OpId> {
        self.writes
            .iter()
            .find(|(other, (_, write))| **other != id && write.seen.contains(&id))
            .map(|(other, _)| *other)
    }

    /// The live writes, in stamp order.
    fn live(&self) -> Vec<OpId> {
        let mut live: Vec<OpId> = self
            .writes
            .keys()
            .copied()
            .filter(|&id| self.replacer(id).is_none())
            .collect();
        live.sort_by_key(|id| {
            let (author, write) = &self.writes[id];
            (write.stamp, *author, *id)
        });
        live
    }
}

impl Mergeable for Register {
    type Change = Write;
    /// The winning value, then the conflicts' values, in stamp order.
    type Read = (Option<u8>, Vec<u8>);

    fn apply(&mut self, op: &Op<Write>) {
        self.writes
            .entry(op.id)
            .or_insert_with(|| (op.author, op.body.clone()));
    }

    fn state_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        for (id, (author, write)) in &self.writes {
            bytes.extend(id.to_le_bytes());
            bytes.push(*author);
            bytes.extend(write.stamp.to_le_bytes());
            bytes.push(write.value);
            bytes.extend((write.seen.len() as u32).to_le_bytes());
            for seen in &write.seen {
                bytes.extend(seen.to_le_bytes());
            }
        }
        bytes
    }

    fn read(&self) -> (Option<u8>, Vec<u8>) {
        let mut values: Vec<u8> = self
            .live()
            .iter()
            .map(|id| self.writes[id].1.value)
            .collect();
        let winner = values.pop();
        (winner, values)
    }

    fn outcome(&self, id: OpId) -> Outcome {
        if let Some(by) = self.replacer(id) {
            Outcome::Replaced(by)
        } else if self.live().last() == Some(&id) {
            Outcome::Visible
        } else {
            Outcome::Lost
        }
    }
}

/// Generates writes the way a member makes them: its stamp is above every
/// stamp its author had seen (O3 of RFC 0004), and it replaces exactly what
/// its author had seen.
struct Writes;

impl Generator<Register> for Writes {
    /// How far the stamp moves past the author's highest seen stamp, and the
    /// value.
    type Raw = (u8, u8);

    fn raw(&self, _: Member) -> BoxedStrategy<(u8, u8)> {
        (0u8..3, any::<u8>()).boxed()
    }

    fn make(&self, author: &Author<'_, Register>, &(step, value): &(u8, u8)) -> Write {
        let highest = author.seen.iter().map(|op| op.body.stamp).max();
        Write {
            stamp: highest.map_or(0, |stamp| stamp + 1) + u64::from(step),
            value,
            seen: author.seen.iter().map(|op| op.id).collect(),
        }
    }
}

#[test]
fn the_register_meets_the_contract() {
    let result = convergence::check::<Register, _>(&Config::default(), &Writes);
    assert!(result.is_ok(), "{}", result.unwrap_err());
}

thread_local! {
    /// Per operation the harness had made, on this test's thread: its id,
    /// its author and the ids of what its author had seen.
    static MADE: RefCell<Vec<(OpId, Member, Vec<OpId>)>> = const { RefCell::new(Vec::new()) };
}

/// Records what each author had seen, and checks that its copy holds exactly
/// that.
struct RecordingWrites;

impl Generator<Register> for RecordingWrites {
    type Raw = (u8, u8);

    fn raw(&self, member: Member) -> BoxedStrategy<(u8, u8)> {
        Writes.raw(member)
    }

    fn make(&self, author: &Author<'_, Register>, raw: &(u8, u8)) -> Write {
        let mut folded = Register::default();
        for op in author.seen {
            folded.apply(op);
        }
        assert_eq!(
            author.copy.state_bytes(),
            folded.state_bytes(),
            "the author's copy is not what it had seen"
        );
        let seen = author.seen.iter().map(|op| op.id).collect();
        MADE.with(|made| made.borrow_mut().push((author.id, author.member, seen)));
        Writes.make(author, raw)
    }
}

#[test]
fn generators_know_what_each_member_had_seen() {
    MADE.with(|made| made.borrow_mut().clear());
    convergence::check::<Register, _>(&Config::default(), &RecordingWrites)
        .expect("the register meets the contract");
    let made = MADE.with(|made| made.take());

    // The harness makes a case's operations in id order, from 0.
    let mut cases: Vec<Vec<(OpId, Member, Vec<OpId>)>> = Vec::new();
    for op in made {
        if op.0 == 0 {
            cases.push(Vec::new());
        }
        cases.last_mut().expect("a case starts at id 0").push(op);
    }

    let (mut learned, mut concurrent) = (false, false);
    for case in &cases {
        for (id, member, seen) in case {
            // An author sees only operations made before its own.
            assert!(seen.iter().all(|s| s < id), "{case:?}");
            for (other, other_member, _) in &case[..*id as usize] {
                if other_member == member {
                    // An author has seen its own earlier operations.
                    assert!(seen.contains(other), "{case:?}");
                } else if seen.contains(other) {
                    learned = true;
                } else {
                    concurrent = true;
                }
            }
        }
    }
    assert!(learned, "no member ever saw another member's operation");
    assert!(concurrent, "no two operations were ever concurrent");
}
