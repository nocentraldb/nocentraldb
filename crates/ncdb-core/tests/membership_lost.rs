//! The lost grant of RFC 0008 ("Concurrent add and remove of one member";
//! "The function", the outcome "lost"): a grant is lost only when an entry
//! that caps it below its role is concurrent with it, so that neither saw
//! the other. A grant that a later removal or demotion saw was revoked on
//! purpose and stays accepted.

#[path = "support/space.rs"]
mod space;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::membership::{Role, Verdict, resolve};

use space::{Space, id};

const ALICE: u8 = 1;
const BOB: u8 = 2;
const CAROL: u8 = 3;

#[test]
fn a_grant_a_later_removal_saw_is_not_lost() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let add = s.add(ALICE, CAROL, Role::Writer, &[&g]);
    let remove = s.remove(ALICE, CAROL, 0, &[&add]);
    let state = resolve(s.space, [&g, &add, &remove]);
    assert_eq!(state.role(id(CAROL)), None);
    assert_eq!(state.verdict(add.id()), Some(&Verdict::Accepted));
}

#[test]
fn a_grant_a_later_demotion_saw_is_not_lost() {
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let add = s.add(ALICE, CAROL, Role::Admin, &[&g]);
    let demote = s.set_role(ALICE, CAROL, Role::Reader, 0, &[&add]);
    let state = resolve(s.space, [&g, &add, &demote]);
    assert_eq!(state.role(id(CAROL)), Some(Role::Reader));
    assert_eq!(state.verdict(add.id()), Some(&Verdict::Accepted));
    assert_eq!(state.verdict(demote.id()), Some(&Verdict::Accepted));
}

#[test]
fn x_adds_z_x_removes_z_and_y_raises_z_concurrently() {
    // Alice adds Carol and later removes her; Bob, who saw the add but not
    // the removal, raises Carol. The removal revoked the add on purpose and
    // capped the concurrent raise: only the raise is lost.
    let mut s = Space::new(ALICE);
    let g = s.genesis.clone();
    let bob = s.add(ALICE, BOB, Role::Admin, &[&g]);
    let add = s.add(ALICE, CAROL, Role::Writer, &[&bob]);
    let remove = s.remove(ALICE, CAROL, 0, &[&add]);
    let raise = s.set_role(BOB, CAROL, Role::Admin, 0, &[&add]);
    for set in [
        [&g, &bob, &add, &remove, &raise],
        [&raise, &remove, &add, &bob, &g],
    ] {
        let state = resolve(s.space, set);
        assert_eq!(state.role(id(CAROL)), None);
        assert_eq!(state.verdict(add.id()), Some(&Verdict::Accepted));
        assert_eq!(state.verdict(remove.id()), Some(&Verdict::Accepted));
        assert_eq!(state.verdict(raise.id()), Some(&Verdict::Lost));
    }
}
