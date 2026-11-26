//! Un-folding a rejected rich-text change (RFC 0013, "Un-folding a
//! rejected change"): the field keeps the rejected operation's items as
//! deleted placeholders and leaves its delete set out, so that later edits
//! placed next to its characters stay, the text it deleted is back, and
//! text typed inside a block it added is lost. A placeholder gives way to
//! every operation the field folds that adds one of its ids.
#![cfg(ncdb_rich_text)]

mod rich_text_support;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::cbor::{Key, Value};
use ncdb_core::change::Operation;
use ncdb_core::clock::Hlc;
use ncdb_core::field::{FieldType, Outcome};
use ncdb_core::field_op::{Encoding, Origin};
use ncdb_core::ids::{ChangeId, MemberId, ObjectId, OperationId};
use ncdb_core::rich_text::{Edit, Fit, RichText, State, client_id};

use rich_text_support::{add_block, apply, delete_text, doc, type_text};

const NONCE: [u8; 8] = [5; 8];

fn member(n: u8) -> MemberId {
    MemberId([n; 32])
}

fn client(n: u8) -> u64 {
    client_id(&member(n), &NONCE)
}

/// The edit of `author`'s operation `n`, at stamp `n`.
fn edit(n: u8, author: u8, update: &[u8]) -> Edit {
    let adds = !ncdb_core::rich_text::update::read(update)
        .unwrap()
        .runs
        .is_empty();
    let clients = match adds {
        true => [(Key::Unsigned(client(author)), Value::Bytes(NONCE.to_vec()))].into(),
        false => Default::default(),
    };
    let operation = Operation::new(
        ObjectId([1; 16]),
        "notes".to_owned(),
        Some("body".to_owned()),
        5,
        Some(Value::Map(
            [
                (Key::Unsigned(1), Value::Bytes(update.to_vec())),
                (Key::Unsigned(2), Value::Map(clients)),
            ]
            .into(),
        )),
    );
    let origin = Origin {
        id: OperationId {
            change: ChangeId([n; 32]),
            position: 0,
        },
        author: member(author),
        stamp: Hlc(u64::from(n)),
    };
    RichText.decode(&operation, origin).expect("a well-formed edit")
}

/// What a state reads, without markup.
fn text(state: &State) -> String {
    let xml = RichText.read(state).xml;
    let mut out = String::new();
    let mut in_tag = false;
    for c in xml.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}

/// A state that folded `folded` and placed `placed`, in that order.
fn fold(folded: &[&Edit], placed: &[&Edit]) -> State {
    let mut state = State::default();
    for edit in folded {
        RichText.try_apply(&mut state, edit).unwrap();
    }
    for edit in placed {
        RichText.place(&mut state, edit);
    }
    state
}

/// RFC 0013's example: Alice writes "Base.", Carol " Carol." after it,
/// Bob " Bob after Carol." right after Carol's full stop; Carol's change
/// is rejected.
struct Example {
    base: Edit,
    carol: Edit,
    bob: Edit,
}

fn example() -> Example {
    let alice = doc(client(1));
    let base = add_block(&alice, "paragraph", "Base.");
    let carol_doc = doc(client(3));
    apply(&carol_doc, &base);
    let carol = type_text(&carol_doc, 0, 5, " Carol.");
    let bob_doc = doc(client(2));
    apply(&bob_doc, &base);
    apply(&bob_doc, &carol);
    let bob = type_text(&bob_doc, 0, 12, " Bob after Carol.");
    Example {
        base: edit(1, 1, &base),
        carol: edit(2, 3, &carol),
        bob: edit(3, 2, &bob),
    }
}

#[test]
fn a_rejected_changes_items_stay_as_placeholders_that_later_edits_are_placed_next_to() {
    let e = example();
    let state = fold(&[&e.base, &e.bob], &[&e.carol]);
    assert_eq!(text(&state), "Base. Bob after Carol.");
    assert_eq!(
        RichText.outcome(&state, e.bob.id),
        Some(Outcome::Visible),
        "Bob's text is in the document"
    );
    assert_eq!(RichText.outcome(&state, e.carol.id), None);
}

#[test]
fn a_placeholder_folds_to_the_same_state_in_any_order() {
    let e = example();
    let orders: [&[&Edit]; 3] = [
        &[&e.base, &e.carol, &e.bob],
        &[&e.bob, &e.carol, &e.base],
        &[&e.carol, &e.bob, &e.base],
    ];
    let mut ends = Vec::new();
    for order in orders {
        let mut state = State::default();
        for edit in order {
            match edit.id == e.carol.id {
                true => RichText.place(&mut state, edit),
                false => RichText.try_apply(&mut state, edit).unwrap(),
            }
        }
        assert_eq!(text(&state), "Base. Bob after Carol.");
        ends.push(RichText.state_bytes(&state));
    }
    assert_eq!(ends[0], ends[1]);
    assert_eq!(ends[0], ends[2]);
}

#[test]
fn the_text_a_rejected_change_deleted_is_back() {
    let alice = doc(client(1));
    let base = add_block(&alice, "paragraph", "Keep this.");
    let carol_doc = doc(client(3));
    apply(&carol_doc, &base);
    let cut = delete_text(&carol_doc, 0, 0, 5);
    let base = edit(1, 1, &base);
    let cut = edit(2, 3, &cut);
    assert_eq!(text(&fold(&[&base, &cut], &[])), "this.");
    let state = fold(&[&base], &[&cut]);
    assert_eq!(text(&state), "Keep this.");
    assert_eq!(RichText.outcome(&state, base.id), Some(Outcome::Visible));
    // A change that only deletes leaves a placeholder all the same.
    assert_ne!(state.bytes(), fold(&[&base], &[]).bytes());
}

#[test]
fn text_typed_inside_a_block_the_rejected_change_added_is_lost() {
    let alice = doc(client(1));
    let base = add_block(&alice, "paragraph", "Base.");
    let carol_doc = doc(client(3));
    apply(&carol_doc, &base);
    let block = add_block(&carol_doc, "paragraph", "Carol's");
    let bob_doc = doc(client(2));
    apply(&bob_doc, &base);
    apply(&bob_doc, &block);
    let inside = type_text(&bob_doc, 1, 7, " and Bob's");
    let (base, block, inside) = (edit(1, 1, &base), edit(2, 3, &block), edit(3, 2, &inside));
    let state = fold(&[&base, &inside], &[&block]);
    assert_eq!(text(&state), "Base.");
    assert_eq!(RichText.outcome(&state, inside.id), Some(Outcome::Lost));
    assert!(RichText.read(&state).lost.contains(&inside.id));
}

/// Carol writes "P", is restored from a backup taken before, and writes
/// "Q" under the same ids: one of the two changes is rejected.
fn same_ids() -> (Edit, Edit) {
    (edit(1, 3, &p_update()), edit(2, 3, &q_update()))
}

fn p_update() -> Vec<u8> {
    add_block(&doc(client(3)), "paragraph", "P")
}

fn q_update() -> Vec<u8> {
    add_block(&doc(client(3)), "paragraph", "Q")
}

#[test]
fn a_placeholder_gives_its_ids_to_an_operation_the_field_folds() {
    let (p, q) = same_ids();
    // P, the earlier, is rejected: Q, which parked for it, now fits.
    let state = fold(&[], &[&p]);
    assert_eq!(RichText.fit(&state, &q), Fit::Displaces(vec![p.id]));
    let state = fold(&[&q], &[&p]);
    assert_eq!(text(&state), "Q");
    assert_eq!(RichText.outcome(&state, q.id), Some(Outcome::Visible));
    assert_eq!(
        RichText.fit_placeholder(&fold(&[&q], &[]), &p),
        Fit::Later,
        "a placeholder never takes an id an operation added"
    );
}

#[test]
fn a_placeholder_never_deletes_what_an_operation_added_with_the_same_content() {
    let (p, _) = same_ids();
    // The same update, in a change that folded and in one rejected.
    let copy = edit(9, 3, &p_update());
    assert_eq!(text(&fold(&[&p], &[&copy])), "P");
    assert_eq!(
        RichText.fit(&fold(&[], &[&copy]), &p),
        Fit::Displaces(vec![copy.id])
    );
}

#[test]
fn of_two_placeholders_that_give_one_id_different_content_the_earlier_keeps_it() {
    let (p, q) = same_ids();
    let state = fold(&[], &[&p]);
    assert_eq!(RichText.fit_placeholder(&state, &q), Fit::Later);
    let state = fold(&[], &[&q]);
    assert_eq!(RichText.fit_placeholder(&state, &p), Fit::Displaces(vec![q.id]));
}

#[test]
fn a_state_with_placeholders_reads_back_from_its_bytes() {
    let e = example();
    let state = fold(&[&e.base, &e.bob], &[&e.carol]);
    assert_eq!(State::from_bytes(&state.bytes()), Ok(state.clone()));
    assert_ne!(state.bytes(), fold(&[&e.base, &e.bob], &[]).bytes());
}
