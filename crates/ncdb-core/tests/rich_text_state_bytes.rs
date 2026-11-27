//! A rich-text field's state bytes are a function of the updates it folded
//! (RFC 0013, "The state"; RFC 0004's C1): two copies that fold the same
//! updates in any order hold the same bytes, also when a value holds a map
//! of several attributes, as every TipTap link mark and many node
//! attributes do. Yrs keeps such maps in hash maps, whose order differs
//! from map to map. And the rule for the same id twice (RFC 0013) takes
//! items with the same content as the same, whatever order their writer
//! wrote a map in.
#![cfg(ncdb_rich_text)]

mod rich_text_support;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::HashMap;

use ncdb_core::cbor::{Key, Value};
use ncdb_core::change::Operation;
use ncdb_core::clock::Hlc;
use ncdb_core::field::FieldType;
use ncdb_core::field::Outcome;
use ncdb_core::field_op::{Encoding, Origin};
use ncdb_core::ids::{ChangeId, MemberId, ObjectId, OperationId};
use ncdb_core::rich_text::{Edit, RichText, State, client_id, update};
use rich_text_support::{add_block, doc, link, mark, set_attribute_value, state, type_text};
use yrs::Any;

const ALICE: MemberId = MemberId([0xa; 32]);
const NONCE: [u8; 8] = [1, 2, 3, 4, 5, 6, 7, 8];

/// Alice's updates, each as the edit its change carries, in stamp order.
fn edits(updates: &[Vec<u8>]) -> Vec<Edit> {
    let client = client_id(&ALICE, &NONCE);
    updates
        .iter()
        .enumerate()
        .map(|(n, update)| {
            let clients = [(Key::Unsigned(client), Value::Bytes(NONCE.to_vec()))].into();
            let operation = Operation::new(
                ObjectId([1; 16]),
                "notes".to_owned(),
                Some("body".to_owned()),
                5,
                Some(Value::Map(
                    [
                        (Key::Unsigned(1), Value::Bytes(update.clone())),
                        (Key::Unsigned(2), Value::Map(clients)),
                    ]
                    .into(),
                )),
            );
            let origin = Origin {
                id: OperationId {
                    change: ChangeId([n as u8 + 1; 32]),
                    position: 0,
                },
                author: ALICE,
                stamp: Hlc(10 + n as u64),
            };
            RichText
                .decode(&operation, origin)
                .expect("a well-formed edit")
        })
        .collect()
}

fn fold(edits: &[&Edit]) -> State {
    let mut state = State::default();
    for edit in edits {
        RichText.apply(&mut state, edit);
    }
    state
}

/// Every order of three edits.
fn orders() -> [[usize; 3]; 6] {
    [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ]
}

/// Folds `updates` in every order, several times over, and asserts one
/// document and one state.
fn assert_one_state(updates: &[Vec<u8>]) {
    let edits = edits(updates);
    let first = fold(&edits.iter().collect::<Vec<_>>());
    for _ in 0..8 {
        for order in orders() {
            let state = fold(&order.map(|n| &edits[n]));
            assert_eq!(state.document(), first.document(), "order {order:?}");
            assert_eq!(
                RichText.state_bytes(&state),
                RichText.state_bytes(&first),
                "order {order:?}"
            );
        }
    }
}

#[test]
fn a_link_with_several_attributes_folds_to_the_same_bytes_in_every_order() {
    let alice = doc(client_id(&ALICE, &NONCE));
    let updates = vec![
        add_block(&alice, "paragraph", "Ship it on Friday"),
        mark(&alice, 0, 0, 7, "link", link("https://example.com/a")),
        type_text(&alice, 0, 17, " please"),
    ];
    assert_one_state(&updates);
}

#[test]
fn a_node_attribute_holding_a_map_folds_to_the_same_bytes_in_every_order() {
    let alice = doc(client_id(&ALICE, &NONCE));
    let image = HashMap::from([
        ("src".to_owned(), Any::from("https://example.com/i.png")),
        ("alt".to_owned(), Any::from("a picture")),
        ("title".to_owned(), Any::Null),
        (
            "size".to_owned(),
            Any::from(HashMap::from([
                ("width".to_owned(), Any::from(640.0)),
                ("height".to_owned(), Any::from(480.0)),
                ("unit".to_owned(), Any::from("px")),
            ])),
        ),
    ]);
    let updates = vec![
        add_block(&alice, "paragraph", "Look"),
        set_attribute_value(&alice, 0, "data", Any::from(image)),
        type_text(&alice, 0, 4, " here"),
    ];
    assert_one_state(&updates);
}

/// The link's attributes as the key-order writer writes them, and the same
/// attributes in the reverse order, as another writer may: equal lengths.
const SORTED: &str = r#"{"class":null,"href":"https://example.com/a","rel":"noopener noreferrer nofollow","target":"_blank"}"#;
const REVERSED: &str = r#"{"target":"_blank","rel":"noopener noreferrer nofollow","href":"https://example.com/a","class":null}"#;

/// `update` with the link's attributes written in the reverse order.
fn reversed(update: &[u8]) -> Vec<u8> {
    let at = update
        .windows(SORTED.len())
        .position(|w| w == SORTED.as_bytes())
        .expect("the update holds the link's attributes in key order");
    let mut out = update.to_vec();
    out[at..at + SORTED.len()].copy_from_slice(REVERSED.as_bytes());
    out
}

/// Alice writes a paragraph with a link as one update, in key order; a
/// writer that encodes maps in another order sends the same items again
/// with the link's attributes reversed, as a member restored from a
/// backup does; then Alice types on.
fn restored_writer() -> Vec<Vec<u8>> {
    let alice = doc(client_id(&ALICE, &NONCE));
    let canonical = |u: Vec<u8>| update::canonical(&u).expect("an update Yrs wrote");
    add_block(&alice, "paragraph", "Ship it on Friday");
    mark(&alice, 0, 0, 7, "link", link("https://example.com/a"));
    let first = canonical(state(&alice));
    let again = reversed(&first);
    let typed = canonical(type_text(&alice, 0, 17, " please"));
    vec![first, again, typed]
}

#[test]
fn a_map_in_another_key_order_gives_the_same_digest() {
    let updates = restored_writer();
    assert_ne!(updates[0], updates[1]);
    assert_eq!(
        update::read(&updates[1]).unwrap().runs,
        update::read(&updates[0]).unwrap().runs
    );
}

#[test]
fn a_writer_that_sends_the_same_items_with_a_map_in_another_key_order_does_not_park() {
    let edits = edits(&restored_writer());
    let mut state = State::default();
    for edit in &edits {
        assert_eq!(RichText.try_apply(&mut state, edit), Ok(()));
    }
    for edit in &edits {
        assert_eq!(RichText.outcome(&state, edit.id), Some(Outcome::Visible));
    }
    assert!(RichText.read(&state).markdown.contains("Friday please"));
}
