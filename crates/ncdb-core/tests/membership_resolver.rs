//! `resolve` with a memory (NOC-424): a [`Resolver`] computes RFC 0008's
//! `resolve` of every set of control changes it is given, and remembers
//! from one call to the next what depends on a single change or on a
//! node's causal past alone, so that a new control change does not judge
//! the earlier ones again. The state it returns is `resolve`'s, byte for
//! byte, in any order of arrival, with equivocations, gaps and shreds.

mod support;

#[path = "support/control.rs"]
mod control;
#[path = "support/space.rs"]
mod space;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::cell::RefCell;
use std::rc::Rc;

use ncdb_core::change::Change;
use ncdb_core::membership::{Resolver, resolve};
use support::convergence::{self, Author, Config, Generator, Mergeable, Op, OpId, Outcome};

use control::{Copy, Members, Raw, base};

/// A copy of the convergence harness whose state one [`Resolver`] also
/// resolves, shared by every copy of a run, so that it is given the sets
/// of all of them in turn. Every step checks that it gives what `resolve`
/// gives.
#[derive(Clone, Debug, Default)]
struct Remembering {
    copy: Copy,
    resolver: Shared,
}

#[derive(Clone, Default)]
struct Shared(Rc<RefCell<Option<Resolver>>>);

impl std::fmt::Debug for Shared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Resolver")
    }
}

impl Remembering {
    /// Checks the resolver against `resolve` on the copy's changes.
    fn check(&self) {
        let (space, _, base) = base();
        let changes: Vec<&Change> = base.iter().chain(self.copy.changes.values()).collect();
        let mut resolver = self.resolver.0.borrow_mut();
        let resolver = resolver.get_or_insert_with(|| Resolver::new(*space));
        let remembered = resolver.resolve(changes.iter().copied());
        let oracle = Resolver::from_scratch(*space).resolve(changes.iter().copied());
        assert_eq!(
            remembered, oracle,
            "the resolver gives what resolve gives, every past folded from scratch"
        );
        assert_eq!(resolve(*space, changes.iter().copied()), oracle);
        assert_eq!(remembered.encode(), self.copy.resolved().encode());
    }
}

impl Mergeable for Remembering {
    type Change = Change;
    type Read = control::Read;

    fn apply(&mut self, op: &Op<Change>) {
        // A resolver of the set before the change, to add the change to
        // (NOC-497).
        let (space, _, base) = base();
        let mut adding = (!self.copy.changes.contains_key(&op.id)).then(|| {
            let mut resolver = Resolver::new(*space);
            resolver.resolve(base.iter().chain(self.copy.changes.values()));
            resolver
        });
        self.copy.apply(op);
        self.check();
        if let Some(added) = adding.as_mut().and_then(|r| r.add([&op.body])) {
            let changes = base.iter().chain(self.copy.changes.values());
            let oracle = Resolver::from_scratch(*space).resolve(changes);
            assert_eq!(added, oracle, "add gives what resolve gives");
        }
    }

    fn state_bytes(&self) -> Vec<u8> {
        self.check();
        self.copy.state_bytes()
    }

    fn read(&self) -> control::Read {
        self.copy.read()
    }

    fn outcome(&self, id: OpId) -> Outcome {
        self.copy.outcome(id)
    }
}

/// The members of `control`, writing from the copy a [`Remembering`]
/// wraps.
struct Through(Members);

impl Generator<Remembering> for Through {
    type Raw = Raw;

    fn raw(&self, member: u8) -> proptest::strategy::BoxedStrategy<Raw> {
        self.0.raw(member)
    }

    fn make(&self, author: &Author<'_, Remembering>, raw: &Raw) -> Change {
        let author = Author {
            member: author.member,
            id: author.id,
            copy: &author.copy.copy,
            seen: author.seen,
        };
        self.0.make(&author, raw)
    }
}

fn check(members: Members, cases: u32) {
    let config = Config {
        members: 3,
        max_changes_per_member: 4,
        cases,
        ..Config::default()
    };
    let result = convergence::check::<Remembering, _>(&config, &Through(members));
    assert!(result.is_ok(), "{}", result.unwrap_err());
}

#[test]
fn the_resolver_gives_what_resolve_gives_for_every_set_in_any_order() {
    check(
        Members {
            forks: false,
            shreds: false,
        },
        32,
    );
}

#[test]
fn the_resolver_gives_what_resolve_gives_for_person_keys_shreds_and_revocations() {
    // Every change a person-key entry, a shred or a revocation, without
    // equivocations: most person-key entries then extend the fold of the
    // set before them (NOC-442).
    check(
        Members {
            forks: false,
            shreds: true,
        },
        64,
    );
}

#[test]
fn the_resolver_gives_what_resolve_gives_with_equivocations_and_shreds() {
    check(
        Members {
            forks: true,
            shreds: false,
        },
        32,
    );
    check(
        Members {
            forks: true,
            shreds: true,
        },
        32,
    );
}

/// A chain of `n` person-key entries after the genesis, each depending on
/// the one before, as the SDK writes them.
fn chain(s: &mut space::Space, n: u8) -> Vec<Change> {
    let mut changes = vec![s.genesis.clone()];
    for person in 1..=n {
        let last = changes.last().expect("the genesis").clone();
        changes.push(s.person_key(1, person, person, &[&last]));
    }
    changes
}

#[test]
fn a_new_control_change_judges_only_itself_whatever_the_space_holds() {
    for held in [20u8, 200] {
        let mut s = space::Space::new(1);
        let mut changes = chain(&mut s, held);
        let mut resolver = Resolver::new(s.space);
        resolver.resolve(&changes);
        assert_eq!(
            resolver.judged(),
            changes.len(),
            "the first call judges all"
        );
        let last = changes.last().expect("a change").clone();
        changes.push(s.person_key(1, held + 1, 1, &[&last]));
        let resolved = resolver.resolve(&changes);
        assert_eq!(resolver.judged(), 1, "with {held} person keys held");
        assert_eq!(resolved, resolve(s.space, &changes));
    }
}

#[test]
fn a_change_that_arrives_into_a_gap_judges_what_it_completes_again() {
    let mut s = space::Space::new(1);
    let changes = chain(&mut s, 5);
    let mut resolver = Resolver::new(s.space);
    // The third person key's entry arrives last: the two after it wait in
    // a gap until it does.
    let held: Vec<&Change> = changes
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != 3)
        .map(|(_, change)| change)
        .collect();
    assert_eq!(
        resolver.resolve(held.iter().copied()),
        resolve(s.space, held.iter().copied())
    );
    let resolved = resolver.resolve(&changes);
    assert_eq!(resolved, resolve(s.space, &changes));
    assert_eq!(resolver.judged(), 3, "the change and the two after it");
}

#[test]
fn a_change_with_a_signature_that_does_not_verify_is_not_taken_for_one_that_does() {
    let mut s = space::Space::new(1);
    let changes = chain(&mut s, 2);
    let mut resolver = Resolver::new(s.space);
    resolver.resolve(&changes);
    // The same header under another signature has the same change id.
    let mut bytes = changes[2].bytes().to_vec();
    let last = bytes.len() - 1;
    bytes[last] ^= 1;
    let forged = Change::decode(&bytes).expect("a change that decodes");
    assert_eq!(forged.id(), changes[2].id());
    let with_forged = [&changes[0], &changes[1], &forged];
    assert_eq!(
        resolver.resolve(with_forged),
        resolve(s.space, with_forged),
        "the forged change is rejected as resolve rejects it"
    );
}
