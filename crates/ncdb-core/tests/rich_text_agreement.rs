//! Yjs and Yrs agree (RFC 0013, "How rich text meets the plugin contract",
//! C3): the updates Yjs 13.6.33 wrote for two members, applied by the core
//! with Yrs in any order, reach the document a Yjs document reaches, and
//! the same XML, because the editor shows Yjs's result while every member
//! folds with Yrs. The same document: the same items with the same
//! content, as the core's own reading of the encoding sees them, and the
//! same deletions. Not the same bytes: Yjs writes the delete set's clients
//! from the largest id down, Yrs from the smallest up. tests/vectors/rich-text/generate.mjs wrote
//! the files with Yjs itself. An upgrade of Yjs or Yrs must pass them
//! unchanged; one that does not changes rule 1.
#![cfg(feature = "rich-text")]

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ncdb_core::cbor::{Key, Value};
use ncdb_core::change::Operation;
use ncdb_core::clock::Hlc;
use ncdb_core::field::FieldType;
use ncdb_core::field_op::{Encoding, Origin};
use ncdb_core::ids::{ChangeId, MemberId, ObjectId, OperationId};
use ncdb_core::rich_text::{RichText, State, client_id, update};

const UPDATES: &str = include_str!("vectors/rich-text/updates.txt");
const STATE: &str = include_str!("vectors/rich-text/state.hex");
const XML: &str = include_str!("vectors/rich-text/document.xml");

const NONCE: [u8; 8] = [1, 2, 3, 4, 5, 6, 7, 8];

fn hex(text: &str) -> Vec<u8> {
    let text = text.trim();
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

/// The vectors' operations: each member's updates, as operation 5 of a
/// change it signed.
fn edits() -> Vec<ncdb_core::rich_text::Edit> {
    UPDATES
        .lines()
        .enumerate()
        .map(|(n, line)| {
            let (who, bytes) = line.split_once(' ').unwrap();
            let author = MemberId([if who == "alice" { 0x0a } else { 0x0b }; 32]);
            let update_bytes = hex(bytes);
            let adds = !update::read(&update_bytes).unwrap().runs.is_empty();
            let clients = match adds {
                true => [(
                    Key::Unsigned(client_id(&author, &NONCE)),
                    Value::Bytes(NONCE.to_vec()),
                )]
                .into(),
                false => Default::default(),
            };
            let operation = Operation::new(
                ObjectId([1; 16]),
                "notes".into(),
                Some("body".into()),
                5,
                Some(Value::Map(
                    [
                        (Key::Unsigned(1), Value::Bytes(update_bytes)),
                        (Key::Unsigned(2), Value::Map(clients)),
                    ]
                    .into(),
                )),
            );
            let origin = Origin {
                id: OperationId {
                    change: ChangeId([n as u8; 32]),
                    position: 0,
                },
                author,
                stamp: Hlc(n as u64),
            };
            RichText
                .decode(&operation, origin)
                .expect("Yjs's update decodes")
        })
        .collect()
}

#[test]
fn yrs_reaches_yjss_state_and_xml_in_every_order() {
    let edits = edits();
    assert_eq!(edits.len(), 10);
    let n = edits.len();
    let orders: Vec<Vec<usize>> = vec![
        (0..n).collect(),
        (0..n).rev().collect(),
        (0..n).step_by(2).chain((1..n).step_by(2)).collect(),
    ];
    for order in orders {
        let mut state = State::default();
        for i in &order {
            RichText.apply(&mut state, &edits[*i]);
        }
        let ours = update::read(state.document()).unwrap();
        let yjs = update::read(&hex(STATE)).unwrap();
        assert_eq!(ours, yjs, "order {order:?}");
        assert_eq!(RichText.read(&state).xml, XML.trim(), "order {order:?}");
    }
}
