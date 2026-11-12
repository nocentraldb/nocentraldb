//! What an equivocating or lying admin does to an accepted shred (NOC-226,
//! NOC-229; RFC 0002, "Author and author sequence"; RFC 0008, "The cutoff"
//! and "A shred is final when accepted").
//!
//! RFC 0002 keeps and folds both changes of an equivocation, inside a
//! revocation's cutoff too. Under RFC 0008's earlier rule, a revocation an
//! admin wrote under a sequence the stability rule had counted, or under a
//! new sequence with dependencies that leave the shred out, rejected a shred
//! that was stable (NOC-224). The owner decided that a shred is final when
//! accepted (Carsten, 2026-11-11, NOC-224), so the same changes now take
//! effect on the membership and leave the shred accepted. A former admin's
//! fork inside a senior admin's demotion still counts only when its change
//! id sorts before the change it duplicates; it no longer reaches the shred.
//! Every copy agrees, in every order.

#[path = "support/space.rs"]
mod space;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::BTreeSet;

use ncdb_core::change::{Change, Operation};
use ncdb_core::ids::{ObjectId, SpaceId};
use ncdb_core::membership::{
    self, Action, Certificate, ControlBody, Entry, Genesis, MemberKind, MembershipEntry, Rejection,
    Resolved, Role, Verdict, resolve,
};

use space::{Space, Who, account, dependencies, id, key, other_account, space_key};

const ALICE: u8 = 1;
const BOB: u8 = 2;
const CAROL: u8 = 3;
const DAN: u8 = 4;

/// Alice creates the space, Bob is a writer, Carol an admin of a second
/// account. Bob writes a person-key entry and shreds it under Carol's
/// account's sign-off; Alice and Carol then write after the shred.
struct Shredded {
    s: Space,
    changes: Vec<Change>,
    carol: Change,
    shred: Change,
}

fn shredded() -> Shredded {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let bob = s.add(ALICE, BOB, Role::Writer, &[&g]);
    s.certify_by(CAROL, other_account(1));
    let carol = s.add(ALICE, CAROL, Role::Admin, &[&bob]);
    let entry = s.person_key(BOB, 1, 0xc1, &[&carol]);
    let shred = s.shred(BOB, 1, &other_account(1), &[&entry]);
    let by_alice = s.envelope(ALICE, BOB, &[&shred]);
    let by_carol = s.envelope(CAROL, BOB, &[&shred]);
    let changes = vec![
        g,
        bob,
        carol.clone(),
        entry,
        shred.clone(),
        by_alice,
        by_carol,
    ];
    Shredded {
        s,
        changes,
        carol,
        shred,
    }
}

/// A removal of Bob by `author` under `sequence`, with this cutoff, after
/// `deps` only: whatever sequence and dependencies its writer chooses.
fn removal_at(s: &Space, author: Who, sequence: u64, cutoff: u64, deps: &[&Change]) -> Change {
    revocation_at(s, author, sequence, Action::Remove { cutoff }, deps)
}

/// A revocation of Bob's rights by `author` under `sequence`, after `deps`.
fn revocation_at(
    s: &Space,
    author: Who,
    sequence: u64,
    action: Action,
    deps: &[&Change],
) -> Change {
    let entry = Entry::Membership(MembershipEntry {
        subject: id(BOB),
        action,
    });
    let body = ControlBody::new(dependencies(deps), entry);
    membership::write(&key(author), s.space, sequence, s.first_generation, &body)
}

/// A space like [`Space::new`] whose genesis seals its model id under a
/// fixed nonce, so that every change id in it is the same on every run.
fn deterministic(creator: Who) -> Space {
    let mut s = Space::new(creator);
    let creator_key = key(creator);
    let genesis = Genesis {
        nonce: [7; 16],
        creator: creator_key.public(),
        certificate: Certificate::sign(&account(), creator_key.id(), MemberKind::Agent),
        first_generation: s.first_generation,
        model_id: membership::seal_model_id_with_nonce(
            &space_key(),
            s.space,
            creator_key.id(),
            [1; 16],
            [0; 24],
        ),
    };
    let body = ControlBody::new(BTreeSet::new(), Entry::Genesis(genesis));
    s.genesis = membership::write(&creator_key, s.space, 1, s.first_generation, &body);
    s
}

/// Every order of `items`.
fn permutations<T: Clone>(items: &[T]) -> Vec<Vec<T>> {
    if items.len() <= 1 {
        return vec![items.to_vec()];
    }
    let mut all = Vec::new();
    for i in 0..items.len() {
        let mut rest = items.to_vec();
        let first = rest.remove(i);
        for mut tail in permutations(&rest) {
            tail.insert(0, first.clone());
            all.push(tail);
        }
    }
    all
}

/// The state of `held` and then `late`, these arriving in every order: one
/// state, or the test fails.
fn in_every_order(space: SpaceId, held: &[Change], late: &[Change]) -> Resolved {
    let first = resolve(space, held.iter().chain(late));
    for order in permutations(late) {
        let state = resolve(space, held.iter().chain(&order));
        assert_eq!(state.encode(), first.encode());
        assert_eq!(state.verdicts(), first.verdicts());
    }
    first
}

#[test]
fn the_shred_is_accepted_before_any_admin_equivocates() {
    let Shredded {
        s, changes, shred, ..
    } = shredded();

    let state = resolve(s.space, &changes);

    assert_eq!(state.verdict(shred.id()), Some(&Verdict::Accepted));
}

/// Carol writes a second change under the sequence of her change that saw
/// the shred, with only her own add as its past: a removal of Bob whose
/// cutoff lies below the shred. It is inside every cutoff, so it is
/// accepted and removes Bob, but no revocation excludes a shred.
#[test]
fn an_admins_equivocation_below_its_change_that_saw_the_shred_leaves_the_shred_accepted() {
    let Shredded {
        s,
        changes,
        carol,
        shred,
    } = shredded();
    let counted = s.sequence(CAROL);
    let before_shred = shred.header().sequence - 1;
    let fork = removal_at(&s, CAROL, counted, before_shred, &[&carol]);

    let state = in_every_order(
        s.space,
        &changes[..4],
        &[&changes[4..], std::slice::from_ref(&fork)].concat(),
    );

    assert_eq!(state.verdict(fork.id()), Some(&Verdict::Accepted));
    assert_eq!(state.role(id(BOB)), None);
    assert_eq!(state.verdict(shred.id()), Some(&Verdict::Accepted));
}

/// No equivocation: Carol's device writes the same removal under its next
/// sequence, naming dependencies and a cutoff that leave the shred out.
/// Nobody can check what a revoker held, so it removes Bob; the shred
/// stays accepted.
#[test]
fn an_admins_new_change_that_leaves_the_shred_out_leaves_the_shred_accepted() {
    let Shredded {
        s,
        changes,
        carol,
        shred,
    } = shredded();
    let next = s.sequence(CAROL) + 1;
    let before_shred = shred.header().sequence - 1;
    let lie = removal_at(&s, CAROL, next, before_shred, &[&carol]);

    let state = in_every_order(
        s.space,
        &changes[..4],
        &[&changes[4..], std::slice::from_ref(&lie)].concat(),
    );

    assert_eq!(state.verdict(lie.id()), Some(&Verdict::Accepted));
    assert_eq!(state.verdict(shred.id()), Some(&Verdict::Accepted));
}

/// A former admin bounded by a stable demotion forks below that
/// demotion's cutoff, under the sequence of its own change the demotion
/// saw. The fork is inside the cutoff, but step 5 also asks that its author
/// hold the right in the state so far. The fork and the change it
/// duplicates have the same past, so step 4 puts the one with the lower
/// change id first:
///
/// - a fork before its twin also comes before the senior Alice's demotion,
///   so Carol is still an admin there: it is accepted and revokes Bob;
/// - a fork after its twin comes after the demotion: it is rejected with
///   `NoRight`.
///
/// Every copy agrees either way, and the change id decides it (RFC 0008,
/// "The cutoff"). The shred stays accepted in both cases. Every change here is deterministic, so both cases are
/// found among forks that differ in their cutoff, in whether they remove
/// Bob or lower him to a reader, and in how they spell the same past.
#[test]
fn a_former_admins_fork_inside_its_demotions_cutoff_counts_only_if_it_sorts_before_its_twin_and_never_reaches_the_shred()
 {
    let mut s = deterministic(ALICE);
    let g = s.genesis.clone();
    let bob = s.add(ALICE, BOB, Role::Writer, &[&g]);
    s.certify_by(CAROL, other_account(1));
    let carol = s.add(ALICE, CAROL, Role::Admin, &[&bob]);
    let twin = s.other(CAROL, membership::ACKNOWLEDGEMENT, &[&carol]);
    let cutoff = s.sequence(CAROL);
    let demote = s.set_role(ALICE, CAROL, Role::Writer, cutoff, &[&twin]);
    let carol_ack = s.other(CAROL, membership::ACKNOWLEDGEMENT, &[&demote]);
    let entry = s.person_key(BOB, 1, 0xc1, &[&demote]);
    s.certify_by(DAN, other_account(2));
    let dan = s.add(ALICE, DAN, Role::Admin, &[&entry]);
    let shred = s.shred(BOB, 1, &other_account(2), &[&dan]);
    let by_alice = s.other(ALICE, membership::ACKNOWLEDGEMENT, &[&shred]);
    let by_dan = s.other(DAN, membership::ACKNOWLEDGEMENT, &[&shred]);
    let changes = vec![
        g,
        bob.clone(),
        carol.clone(),
        twin.clone(),
        demote.clone(),
        carol_ack,
        entry,
        dan,
        shred.clone(),
        by_alice,
        by_dan,
    ];
    let state = resolve(s.space, &changes);
    assert_eq!(state.verdict(demote.id()), Some(&Verdict::Accepted));
    assert_eq!(state.verdict(shred.id()), Some(&Verdict::Accepted));
    let before_shred = shred.header().sequence - 1;
    let mut forks = Vec::new();
    for below in 0..=before_shred {
        for deps in [vec![&carol], vec![&carol, &bob]] {
            let actions = [
                Action::Remove { cutoff: below },
                Action::SetRole {
                    role: Role::Reader,
                    from_generation: None,
                    cutoff: below,
                },
            ];
            for action in actions {
                forks.push(revocation_at(&s, CAROL, cutoff, action, &deps));
            }
        }
    }
    let (first, after): (Vec<_>, Vec<_>) = forks.iter().partition(|f| f.id() < twin.id());
    assert!(
        !first.is_empty() && !after.is_empty(),
        "the forks fall on both sides of their twin"
    );

    for (fork, sorts_first) in first
        .iter()
        .map(|f| (f, true))
        .chain(after.iter().map(|f| (f, false)))
    {
        let late = [&changes[8..], &[(*fork).clone()]].concat();
        let state = in_every_order(s.space, &changes[..8], &late);

        if sorts_first {
            assert_eq!(state.verdict(fork.id()), Some(&Verdict::Accepted));
        } else {
            assert_eq!(
                state.verdict(fork.id()),
                Some(&Verdict::Rejected(Rejection::NoRight))
            );
        }
        assert_eq!(state.verdict(shred.id()), Some(&Verdict::Accepted));
    }
}

/// A writer's equivocation of sealed changes inside its cutoff: both are
/// admitted, as RFC 0002 folds both changes of an equivocation.
#[test]
fn both_sealed_changes_of_an_equivocation_inside_a_cutoff_are_admitted() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let bob = s.add(ALICE, BOB, Role::Writer, &[&g]);
    let first = s.sealed(BOB, vec![]);
    // A second copy of the same space, in which Bob writes another sealed
    // change under the same sequence.
    let mut restored = Space::new(ALICE);
    restored.skip(BOB, first.header().sequence - 1);
    let task = Operation::new(
        ObjectId([7; 16]),
        "task".into(),
        Some("title".into()),
        1,
        None,
    );
    let second = restored.sealed(BOB, vec![task]);
    assert_eq!(first.header().sequence, second.header().sequence);
    assert_ne!(first.id(), second.id());
    let remove = s.remove(ALICE, BOB, first.header().sequence, &[&bob]);

    let state = resolve(s.space, [&g, &bob, &remove]);

    assert!(state.admits(first.header()));
    assert!(state.admits(second.header()));
}
