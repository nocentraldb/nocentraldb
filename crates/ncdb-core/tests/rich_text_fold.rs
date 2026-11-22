//! The field type `ncdb/richText`, rule 1 (RFC 0013): operation 5 carries a
//! Yjs update and the nonce of each client id it adds, the fold checks the
//! client ids against the change's author, applies the update with Yrs in
//! any order of arrival, keeps the state as canonical bytes, and gives each
//! operation its outcome: applied, replaced or lost.
#![cfg(feature = "rich-text")]

mod rich_text_support;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::BTreeMap;

use ncdb_core::cbor::{Key, Value};
use ncdb_core::change::Operation;
use ncdb_core::clock::Hlc;
use ncdb_core::field::{FieldType, Outcome};
use ncdb_core::field_op::{Encoding, Origin};
use ncdb_core::ids::{ChangeId, MemberId, ObjectId, OperationId};
use ncdb_core::log::OperationParkReason::{MalformedArguments, UnknownArgument, UnknownOperation};
use ncdb_core::rich_text::{Edit, Fit, RichText, State, client_id};
use rich_text_support::{
    add_block, apply, bold, delete_block, delete_text, doc, set_attribute, type_text,
};

const OBJECT: ObjectId = ObjectId([1; 16]);
const ALICE: MemberId = MemberId([0xa; 32]);
const BOB: MemberId = MemberId([0xb; 32]);
const NONCE: [u8; 8] = [1, 2, 3, 4, 5, 6, 7, 8];

/// A member's writer: its client id comes from the member and `NONCE`.
struct Writer {
    member: MemberId,
    doc: yrs::Doc,
}

impl Writer {
    fn new(member: MemberId) -> Writer {
        Writer {
            member,
            doc: doc(client_id(&member, &NONCE)),
        }
    }

    fn client(&self) -> u64 {
        client_id(&self.member, &NONCE)
    }

    /// The operation 5 that carries `update`, with this writer's nonce when
    /// the update adds items; an update that only deletes lists none.
    fn operation(&self, update: &[u8]) -> Operation {
        let adds = !ncdb_core::rich_text::update::read(update)
            .expect("an update Yrs wrote")
            .runs
            .is_empty();
        match adds {
            true => operation(update, &[(self.client(), NONCE)]),
            false => operation(update, &[]),
        }
    }
}

fn operation(update: &[u8], clients: &[(u64, [u8; 8])]) -> Operation {
    let clients = clients
        .iter()
        .map(|(client, nonce)| (Key::Unsigned(*client), Value::Bytes(nonce.to_vec())))
        .collect();
    arguments(vec![
        (Key::Unsigned(1), Value::Bytes(update.to_vec())),
        (Key::Unsigned(2), Value::Map(clients)),
    ])
}

fn arguments(entries: Vec<(Key, Value)>) -> Operation {
    Operation::new(
        OBJECT,
        "notes".to_owned(),
        Some("body".to_owned()),
        5,
        Some(Value::Map(entries.into_iter().collect())),
    )
}

fn origin(change: u8, author: MemberId, stamp: u64) -> Origin {
    Origin {
        id: OperationId {
            change: ChangeId([change; 32]),
            position: 0,
        },
        author,
        stamp: Hlc(stamp),
    }
}

fn edit(operation: &Operation, origin: Origin) -> Edit {
    RichText
        .decode(operation, origin)
        .expect("a well-formed edit")
}

fn fold(edits: &[&Edit]) -> State {
    let mut state = State::default();
    for edit in edits {
        RichText.apply(&mut state, edit);
    }
    state
}

#[test]
fn an_edit_decodes_and_encodes_as_operation_5() {
    let alice = Writer::new(ALICE);
    let op = alice.operation(&add_block(&alice.doc, "paragraph", "Hello"));
    let e = edit(&op, origin(1, ALICE, 10));
    let (number, args) = RichText.arguments(&e);
    assert_eq!(number, 5);
    assert_eq!(Some(args), op.value);
}

#[test]
fn another_operation_number_is_unknown() {
    let alice = Writer::new(ALICE);
    let mut op = alice.operation(&add_block(&alice.doc, "paragraph", "Hello"));
    op.op = 1;
    assert_eq!(
        RichText.decode(&op, origin(1, ALICE, 10)).err(),
        Some(UnknownOperation)
    );
}

#[test]
fn an_item_whose_client_id_is_not_its_authors_is_malformed() {
    let alice = Writer::new(ALICE);
    let op = alice.operation(&add_block(&alice.doc, "paragraph", "Hello"));
    // Bob signs the change: Alice's client id does not derive from Bob.
    assert_eq!(
        RichText.decode(&op, origin(1, BOB, 10)).err(),
        Some(MalformedArguments)
    );
}

#[test]
fn an_item_whose_client_id_is_not_listed_is_malformed() {
    let alice = Writer::new(ALICE);
    let update = add_block(&alice.doc, "paragraph", "Hello");
    assert_eq!(
        RichText
            .decode(&operation(&update, &[]), origin(1, ALICE, 10))
            .err(),
        Some(MalformedArguments)
    );
}

#[test]
fn a_listed_client_id_that_adds_nothing_or_names_another_nonce_is_malformed() {
    let alice = Writer::new(ALICE);
    let update = add_block(&alice.doc, "paragraph", "Hello");
    let other = [9; 8];
    let extra = operation(
        &update,
        &[(alice.client(), NONCE), (client_id(&ALICE, &other), other)],
    );
    let wrong_nonce = operation(&update, &[(alice.client(), other)]);
    for op in [extra, wrong_nonce] {
        assert_eq!(
            RichText.decode(&op, origin(1, ALICE, 10)).err(),
            Some(MalformedArguments)
        );
    }
}

#[test]
fn malformed_arguments_park() {
    let alice = Writer::new(ALICE);
    let update = add_block(&alice.doc, "paragraph", "Hello");
    let clients =
        || Value::Map([(Key::Unsigned(alice.client()), Value::Bytes(NONCE.to_vec()))].into());
    let cases = [
        // No update.
        arguments(vec![(Key::Unsigned(2), clients())]),
        // An update that is not one.
        arguments(vec![
            (Key::Unsigned(1), Value::Bytes(vec![1, 2, 3])),
            (Key::Unsigned(2), clients()),
        ]),
        // A nonce that is not 8 bytes.
        operation(&update, &[]).tap(|op| {
            op.value = Some(Value::Map(
                [
                    (Key::Unsigned(1), Value::Bytes(update.clone())),
                    (
                        Key::Unsigned(2),
                        Value::Map(
                            [(Key::Unsigned(alice.client()), Value::Bytes(vec![1; 7]))].into(),
                        ),
                    ),
                ]
                .into(),
            ))
        }),
        // No arguments at all.
        Operation::new(OBJECT, "notes".into(), Some("body".into()), 5, None),
    ];
    for op in cases {
        assert_eq!(
            RichText.decode(&op, origin(1, ALICE, 10)).err(),
            Some(MalformedArguments)
        );
    }
}

trait Tap: Sized {
    fn tap(mut self, f: impl FnOnce(&mut Self)) -> Self {
        f(&mut self);
        self
    }
}
impl Tap for Operation {}

#[test]
fn an_unknown_must_understand_argument_parks_and_one_from_64_is_ignored() {
    // Keys 1 to 3 are RFC 0004's: key 3, which operation 5 does not
    // define, is malformed; keys 4 to 63 are unknown (RFC 0013, "How an
    // edit travels in a change").
    let alice = Writer::new(ALICE);
    let op = alice.operation(&add_block(&alice.doc, "paragraph", "Hello"));
    let with = |key: u64| {
        let Some(Value::Map(mut entries)) = op.value.clone() else {
            unreachable!()
        };
        entries.insert(Key::Unsigned(key), Value::Null);
        arguments(entries.into_iter().collect())
    };
    assert_eq!(
        RichText.decode(&with(3), origin(1, ALICE, 10)).err(),
        Some(MalformedArguments)
    );
    assert_eq!(
        RichText.decode(&with(4), origin(1, ALICE, 10)).err(),
        Some(UnknownArgument)
    );
    edit(&with(64), origin(1, ALICE, 10));
}

#[test]
fn edits_fold_to_the_same_state_bytes_in_any_order() {
    let alice = Writer::new(ALICE);
    let bob = Writer::new(BOB);
    let a1 = edit(
        &alice.operation(&add_block(&alice.doc, "paragraph", "Hello")),
        origin(1, ALICE, 10),
    );
    let b1 = edit(
        &bob.operation(&add_block(&bob.doc, "heading", "Title")),
        origin(2, BOB, 11),
    );
    let a2 = edit(
        &alice.operation(&type_text(&alice.doc, 0, 5, " world")),
        origin(3, ALICE, 12),
    );
    let one = fold(&[&a1, &b1, &a2]);
    let other = fold(&[&a2, &b1, &a1]);
    assert_eq!(RichText.state_bytes(&one), RichText.state_bytes(&other));
    assert_eq!(RichText.read(&one), RichText.read(&other));
    assert!(RichText.read(&one).xml.contains("Hello world"));
}

#[test]
fn content_whose_dependencies_have_not_arrived_waits_and_then_shows() {
    let alice = Writer::new(ALICE);
    let a1 = edit(
        &alice.operation(&add_block(&alice.doc, "paragraph", "Hello")),
        origin(1, ALICE, 10),
    );
    let a2 = edit(
        &alice.operation(&type_text(&alice.doc, 0, 5, " world")),
        origin(2, ALICE, 11),
    );
    let mut state = fold(&[&a2]);
    assert!(!RichText.read(&state).xml.contains("world"));
    assert_eq!(RichText.outcome(&state, a2.id), Some(Outcome::Visible));
    RichText.apply(&mut state, &a1);
    assert!(RichText.read(&state).xml.contains("Hello world"));
}

#[test]
fn applying_an_edit_twice_changes_nothing() {
    let alice = Writer::new(ALICE);
    let a1 = edit(
        &alice.operation(&add_block(&alice.doc, "paragraph", "Hello")),
        origin(1, ALICE, 10),
    );
    let once = fold(&[&a1]);
    let twice = fold(&[&a1, &a1]);
    assert_eq!(RichText.state_bytes(&once), RichText.state_bytes(&twice));
}

#[test]
fn the_state_reads_back_from_its_bytes() {
    let alice = Writer::new(ALICE);
    let a1 = edit(
        &alice.operation(&add_block(&alice.doc, "paragraph", "Hello")),
        origin(1, ALICE, 10),
    );
    let a2 = edit(
        &alice.operation(&delete_text(&alice.doc, 0, 1, 2)),
        origin(2, ALICE, 11),
    );
    let state = fold(&[&a1, &a2]);
    let bytes = RichText.state_bytes(&state);
    let read = State::from_bytes(&bytes).expect("the core's own bytes");
    assert_eq!(RichText.state_bytes(&read), bytes);
    assert!(State::from_bytes(&[0xff]).is_err());
}

/// Alice and Bob start from one paragraph; returns the shared edit.
fn shared_paragraph(alice: &Writer, bob: &Writer) -> Edit {
    let update = add_block(&alice.doc, "paragraph", "First.");
    apply(&bob.doc, &update);
    edit(&alice.operation(&update), origin(1, ALICE, 10))
}

#[test]
fn text_typed_into_a_block_another_member_deleted_meanwhile_is_lost() {
    // F2: Alice deletes the paragraph while Bob types into it.
    let alice = Writer::new(ALICE);
    let bob = Writer::new(BOB);
    let base = shared_paragraph(&alice, &bob);
    let deleted = edit(
        &alice.operation(&delete_block(&alice.doc, 0)),
        origin(2, ALICE, 11),
    );
    let typed = edit(
        &bob.operation(&type_text(&bob.doc, 0, 6, " Bob typed this.")),
        origin(3, BOB, 12),
    );
    for state in [
        fold(&[&base, &deleted, &typed]),
        fold(&[&base, &typed, &deleted]),
    ] {
        assert_eq!(RichText.outcome(&state, typed.id), Some(Outcome::Lost));
        assert_eq!(
            RichText.outcome(&state, base.id),
            Some(Outcome::Replaced(deleted.id))
        );
        assert_eq!(RichText.outcome(&state, deleted.id), Some(Outcome::Visible));
        assert!(RichText.read(&state).lost.contains(&typed.id));
    }
}

#[test]
fn of_two_concurrent_values_of_one_attribute_the_overwritten_one_is_lost() {
    // F3: Alice and Bob set a heading's level at the same time.
    let alice = Writer::new(ALICE);
    let bob = Writer::new(BOB);
    let update = add_block(&alice.doc, "heading", "Title");
    apply(&bob.doc, &update);
    let base = edit(&alice.operation(&update), origin(1, ALICE, 10));
    let by_alice = edit(
        &alice.operation(&set_attribute(&alice.doc, 0, "level", "1")),
        origin(2, ALICE, 11),
    );
    let by_bob = edit(
        &bob.operation(&set_attribute(&bob.doc, 0, "level", "2")),
        origin(3, BOB, 12),
    );
    let state = fold(&[&base, &by_alice, &by_bob]);
    let outcomes = [
        RichText.outcome(&state, by_alice.id),
        RichText.outcome(&state, by_bob.id),
    ];
    assert!(outcomes.contains(&Some(Outcome::Lost)), "{outcomes:?}");
    assert!(outcomes.contains(&Some(Outcome::Visible)), "{outcomes:?}");
}

#[test]
fn text_deleted_by_a_member_who_had_seen_it_is_replaced_and_formatting_is_applied() {
    let alice = Writer::new(ALICE);
    let bob = Writer::new(BOB);
    let base = shared_paragraph(&alice, &bob);
    let typed_update = type_text(&bob.doc, 0, 6, " More.");
    let typed = edit(&bob.operation(&typed_update), origin(2, BOB, 11));
    let made_bold = edit(&bob.operation(&bold(&bob.doc, 0, 0, 5)), origin(3, BOB, 12));
    // Alice sees Bob's typing, then deletes it.
    apply(&alice.doc, &typed_update);
    let deleted = edit(
        &alice.operation(&delete_text(&alice.doc, 0, 6, 6)),
        origin(4, ALICE, 13),
    );
    let state = fold(&[&base, &typed, &made_bold, &deleted]);
    assert_eq!(
        RichText.outcome(&state, typed.id),
        Some(Outcome::Replaced(deleted.id))
    );
    assert_eq!(
        RichText.outcome(&state, made_bold.id),
        Some(Outcome::Visible)
    );
    assert!(RichText.read(&state).lost.is_empty());
}

#[test]
fn an_operation_the_state_never_received_has_no_outcome() {
    let state = State::default();
    let id = origin(1, ALICE, 10).id;
    assert_eq!(RichText.outcome(&state, id), None);
}

/// Two edits of Alice's that give her client id's first clocks different
/// content, as a member restored from a backup writes.
fn same_ids_twice() -> (Edit, Edit) {
    let first = Writer::new(ALICE);
    let restored = Writer::new(ALICE);
    let p = edit(
        &first.operation(&add_block(&first.doc, "paragraph", "P")),
        origin(1, ALICE, 10),
    );
    let q = edit(
        &restored.operation(&add_block(&restored.doc, "paragraph", "Q")),
        origin(2, ALICE, 11),
    );
    (p, q)
}

#[test]
fn of_two_edits_that_give_one_id_different_content_the_earlier_keeps_it() {
    let (earlier, later) = same_ids_twice();
    let holds_earlier = fold(&[&earlier]);
    assert_eq!(RichText.fit(&holds_earlier, &later), Fit::Later);
    let holds_later = fold(&[&later]);
    assert_eq!(
        RichText.fit(&holds_later, &earlier),
        Fit::Displaces(vec![later.id])
    );
}

#[test]
fn an_edit_whose_ids_and_content_another_holds_already_fits() {
    let alice = Writer::new(ALICE);
    let update = add_block(&alice.doc, "paragraph", "P");
    let once = edit(&alice.operation(&update), origin(1, ALICE, 10));
    let again = edit(&alice.operation(&update), origin(2, ALICE, 11));
    let state = fold(&[&once]);
    assert_eq!(RichText.fit(&state, &again), Fit::Fits);
    let both = fold(&[&once, &again]);
    assert_eq!(RichText.outcome(&both, again.id), Some(Outcome::Visible));
}

#[test]
fn the_state_and_the_read_print_redacted() {
    let alice = Writer::new(ALICE);
    let a1 = edit(
        &alice.operation(&add_block(&alice.doc, "paragraph", "Secret words")),
        origin(1, ALICE, 10),
    );
    let state = fold(&[&a1]);
    for printed in [
        format!("{state:?}"),
        format!("{:?}", RichText.read(&state)),
        format!("{a1:?}"),
    ] {
        assert!(!printed.contains("Secret"), "{printed}");
        assert!(printed.contains("<redacted>"), "{printed}");
    }
    let _: BTreeMap<u64, [u8; 8]> = a1.clients.clone();
}
