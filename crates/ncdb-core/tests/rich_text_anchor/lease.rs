//! An agent's edits for one field go through one lease, in order (RFC
//! 0013, "Client ids name their author"; F13): each edit continues the
//! lease's clock, even when it is made from a version that does not hold
//! the edits before it, so no two edits write one item id and every copy
//! ends alike, whatever order they arrive in.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::ids::ObjectId;
use ncdb_core::rich_text::anchor::Error;
use ncdb_core::rich_text::update;

use crate::doc::{p, t};
use crate::support::{Agent, Field, replace};

fn field() -> Field {
    Field::new(vec![p(vec![t("We ship on Friday.")]), p(vec![t("Outro.")])])
}

/// The first clock of each run of ids `update` adds.
fn clocks(written: &[u8]) -> Vec<(u64, u64)> {
    update::read(written)
        .unwrap()
        .runs
        .iter()
        .map(|run| (run.range.clock, run.range.len))
        .collect()
}

#[test]
fn two_edits_from_one_read_continue_the_leases_clock() {
    let field = field();
    let mut agent = Agent::new();
    let reading = agent.read(&field);
    let first = agent
        .edit(&reading, &[replace("Friday", "Monday")])
        .unwrap();
    assert_eq!(agent.lease.clock, 3);
    let second = agent
        .edit(&reading, &[replace("Outro.", "The end.")])
        .unwrap();
    assert_eq!(clocks(&first.update), vec![(0, 3)]);
    assert_eq!(clocks(&second.update)[0].0, 3);
    assert_eq!(agent.lease.clock, 3 + clocks(&second.update)[0].1);
}

#[test]
fn a_later_edit_from_the_same_read_sees_the_edits_its_lease_wrote() {
    let mut field = field();
    let mut agent = Agent::new();
    let reading = agent.read(&field);
    let first = agent
        .edit(&reading, &[replace("Friday", "Monday")])
        .unwrap();
    let second = agent
        .edit(&reading, &[replace("Monday", "Monday morning")])
        .unwrap();
    field.agent(&first);
    field.agent(&second);
    assert_eq!(field.markdown(), "We ship on Monday morning.\n\nOutro.");
}

#[test]
fn copies_that_receive_the_edits_in_either_order_end_alike() {
    let base = field();
    let mut agent = Agent::new();
    let reading = agent.read(&base);
    let first = agent
        .edit(&reading, &[replace("Friday", "Monday")])
        .unwrap();
    let second = agent
        .edit(&reading, &[replace("Outro.", "The end.")])
        .unwrap();
    let (mut a, mut b) = (field(), field());
    a.agent(&first);
    a.agent(&second);
    b.agent(&second);
    b.agent(&first);
    assert_eq!(a.markdown(), "We ship on Monday.\n\nThe end.");
    assert_eq!(b.markdown(), a.markdown());
    assert_eq!(a.state.document(), b.state.document());
}

#[test]
fn a_refused_edit_leaves_the_lease_where_it_was() {
    let field = field();
    let mut agent = Agent::new();
    let reading = agent.read(&field);
    agent
        .edit(&reading, &[replace("Friday", "Monday")])
        .unwrap();
    assert_eq!(
        agent.edit(&reading, &[replace("Nowhere", "x")]),
        Err(Error::AnchorNotFound { edit: 0 })
    );
    assert_eq!(agent.lease.clock, 3);
}

#[test]
fn a_lease_for_another_object_is_refused() {
    let field = field();
    let mut agent = Agent::new();
    let reading = agent.read(&field);
    agent.lease.object = ObjectId([0x22; 16]);
    assert_eq!(
        agent.edit(&reading, &[replace("Friday", "Monday")]),
        Err(Error::LeaseMismatch)
    );
}

#[test]
fn a_lease_whose_items_the_version_does_not_hold_is_refused() {
    // The lease reached clock 5 through a writer this cache does not
    // know of; a new item at clock 0 would reuse an id.
    let field = field();
    let mut agent = Agent::new();
    let reading = agent.read(&field);
    agent.lease.clock = 5;
    assert_eq!(
        agent.edit(&reading, &[replace("Friday", "Monday")]),
        Err(Error::LeaseBehind)
    );
}
