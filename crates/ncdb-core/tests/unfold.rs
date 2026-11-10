//! Un-folding (NOC-49; RFC 0006, "Un-folding"; RFC 0008, "The cutoff"): a
//! revocation that arrives after a member folded the revoked member's
//! changes moves every one of them above the cutoff from folded to
//! rejected, and the fold no longer holds their operations. The fields
//! and objects they touched are folded again from the changes still
//! folded. Changes at or below the cutoff stay folded.

#[path = "support/member.rs"]
mod member;
#[path = "support/space.rs"]
mod space;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::change::Change;
use ncdb_core::checks::SealChecks;
use ncdb_core::field_op;
use ncdb_core::ids::{ObjectId, OperationId};
use ncdb_core::log::{Log, RejectReason, State};
use ncdb_core::membership::Role;

use member::{COLLECTION, Recorder, set_title};
use space::{Space, key};

const ALICE: u8 = 1;
const BOB: u8 = 2;
const CAROL: u8 = 3;

/// Carol's copy of the space: her log, her checks and her fold.
struct Copy {
    log: Log,
    checks: SealChecks,
    fold: Recorder,
}

impl Copy {
    fn of(space: &Space) -> Copy {
        Copy {
            log: Log::new(space.space),
            checks: SealChecks::new(space.space, key(CAROL)),
            fold: Recorder::default(),
        }
    }

    fn receive(&mut self, changes: &[&Change]) {
        for change in changes {
            self.log
                .receive(change.bytes(), &mut self.checks, &mut self.fold)
                .expect("a change of the space is appended");
        }
    }

    fn state(&self, change: &Change) -> State {
        self.log.state(change.id()).cloned().expect("a held change")
    }

    /// Whether the fold holds the operation at `position` of `change`.
    fn shows(&self, change: &Change, position: u64) -> bool {
        self.fold.folded.contains(&change.operation_id(position))
    }

    /// Whether the fold holds a create of `object`.
    fn exists(&self, object: ObjectId) -> bool {
        self.fold.objects.get(&object).is_some_and(|o| o.exists())
    }
}

fn folded() -> State {
    State::Folded { parked: vec![] }
}

fn not_member() -> State {
    State::Rejected(RejectReason::NotMember)
}

fn object(n: u8) -> ObjectId {
    ObjectId([n; 16])
}

/// Alice founds the space with Bob as a writer and Carol as a reader, and
/// wraps the space key to Carol. Bob then writes three changes and Alice
/// one: Bob's first and third and Alice's write the title of object 1,
/// Bob's second creates object 2 and writes its title.
struct Written {
    space: Space,
    controls: Vec<Change>,
    last_control: Change,
    bob: [Change; 3],
    alice: Change,
}

fn written() -> Written {
    let mut space = Space::new(ALICE);
    let genesis = space.genesis.clone();
    let bob = space.add(ALICE, BOB, Role::Writer, &[&genesis]);
    let carol = space.add(ALICE, CAROL, Role::Reader, &[&bob]);
    let to_carol = space.envelope(ALICE, CAROL, &[&carol]);
    let b1 = space.sealed(BOB, vec![set_title(1)]);
    let b2 = space.sealed(
        BOB,
        vec![field_op::create(object(2), COLLECTION), set_title(2)],
    );
    let b3 = space.sealed(BOB, vec![set_title(1)]);
    let alice = space.sealed(ALICE, vec![set_title(1)]);
    Written {
        space,
        controls: vec![genesis, bob, carol],
        last_control: to_carol,
        bob: [b1, b2, b3],
        alice,
    }
}

impl Written {
    /// Carol's copy after every change so far, all of them folded.
    fn copy(&self) -> Copy {
        let mut copy = Copy::of(&self.space);
        for change in self.controls.iter().chain([&self.last_control]) {
            copy.receive(&[change]);
        }
        copy.receive(&[&self.bob[0], &self.bob[1], &self.bob[2], &self.alice]);
        for change in self.bob.iter().chain([&self.alice]) {
            assert_eq!(copy.state(change), folded(), "before the revocation");
        }
        copy
    }

    /// Alice removes Bob with this cutoff.
    fn remove_bob(&mut self, cutoff: u64) -> Change {
        let after = self.last_control.clone();
        self.space.remove(ALICE, BOB, cutoff, &[&after])
    }
}

#[test]
fn a_late_revocation_un_folds_the_changes_above_its_cutoff() {
    let mut w = written();
    let mut copy = w.copy();
    let cutoff = w.bob[0].header().sequence;
    let removal = w.remove_bob(cutoff);
    copy.receive(&[&removal]);

    assert_eq!(copy.state(&removal), folded());
    assert_eq!(copy.state(&w.bob[0]), folded(), "at the cutoff");
    assert_eq!(copy.state(&w.bob[1]), not_member(), "above the cutoff");
    assert_eq!(copy.state(&w.bob[2]), not_member(), "above the cutoff");
    assert_eq!(copy.state(&w.alice), folded(), "another member's change");

    // The fold no longer shows the operations of the un-folded changes,
    // and keeps the others' on the fields those touched.
    assert!(copy.shows(&w.bob[0], 0));
    assert!(!copy.shows(&w.bob[1], 1), "Bob's title of object 2");
    assert!(!copy.shows(&w.bob[2], 0), "Bob's later title of object 1");
    assert!(copy.shows(&w.alice, 0), "Alice's title of object 1");
    assert!(!copy.exists(object(2)), "Bob's create of object 2");
}

#[test]
fn changes_at_or_below_the_cutoff_stay_folded() {
    let mut w = written();
    let mut copy = w.copy();
    let cutoff = w.bob[1].header().sequence;
    let removal = w.remove_bob(cutoff);
    copy.receive(&[&removal]);

    assert_eq!(copy.state(&w.bob[0]), folded(), "below the cutoff");
    assert_eq!(copy.state(&w.bob[1]), folded(), "at the cutoff");
    assert_eq!(copy.state(&w.bob[2]), not_member(), "one above the cutoff");
    assert!(copy.shows(&w.bob[1], 1));
    assert!(copy.exists(object(2)));
    assert!(!copy.shows(&w.bob[2], 0));
}

#[test]
fn the_fold_after_un_folding_equals_the_fold_of_a_copy_that_never_folded_them() {
    let mut w = written();
    let mut late = w.copy();
    let removal = w.remove_bob(0);
    late.receive(&[&removal]);

    let mut early = Copy::of(&w.space);
    for change in w.controls.iter().chain([&w.last_control, &removal]) {
        early.receive(&[change]);
    }
    early.receive(&[&w.bob[0], &w.bob[1], &w.bob[2], &w.alice]);

    for change in &w.bob {
        assert_eq!(late.state(change), not_member());
        assert_eq!(early.state(change), not_member());
    }
    assert_eq!(late.fold, early.fold);
    let alice: OperationId = w.alice.operation_id(0);
    assert_eq!(late.fold.folded, [alice].into());
}
