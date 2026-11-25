//! Later edits after a reopen (RFC 0016, "Later edits"; pm on NOC-475):
//! later edits are not stored on the trace but computed from the old
//! space's log, so a view reopened from disk, whose log is read back from
//! its file, gives exactly the later edits it gave before the close.

mod support;

#[path = "support/member.rs"]
mod member;
#[path = "support/view.rs"]
mod view_support;

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use std::collections::BTreeSet;

use ncdb_core::basic::{BasicValue, Kind, Write};
use ncdb_core::cbor::Value;
use ncdb_core::change::{Change, Operation, Payload};
use ncdb_core::clock::Hlc;
use ncdb_core::field_op;
use ncdb_core::ids::{ChangeId, ObjectId, OperationId, SpaceId};
use ncdb_core::moves;
use ncdb_core::view::View;

use member::*;
use view_support::*;

const T: ObjectId = ObjectId([0x11; 16]);
const NEW: SpaceId = SpaceId([0x5a; 16]);

fn write(value: Value) -> Write {
    Write {
        id: OperationId {
            change: ChangeId([0; 32]),
            position: 0,
        },
        author: member(0),
        stamp: Hlc(0),
        value,
        replaces: BTreeSet::new(),
    }
}

fn title(value: &str) -> Operation {
    let text = BasicValue { kind: Kind::Text };
    field_op::encode(&text, &write(Value::Text(value.into())), T, TASK, TITLE)
}

/// A change of `author` at `clock` that depends on `after`.
fn change(
    author: u8,
    sequence: u64,
    clock: u64,
    after: &[&[u8]],
    operations: Vec<Operation>,
) -> Vec<u8> {
    let dependencies = after
        .iter()
        .map(|f| Change::decode(f).unwrap().id())
        .collect();
    let payload = Payload::new(Hlc(clock), dependencies, operations).encode();
    frame(sealed_header(author, sequence, 0, payload), GOOD)
}

/// The checks of a member of the space: they learn the control changes
/// from the log.
fn checks() -> Control {
    let mut control = Control::default();
    control.keys.insert(0);
    control.members = (0..3).map(|n| (member(n), 0)).collect();
    control
}

#[test]
fn a_reopened_view_gives_the_later_edits_it_gave_before_the_close() {
    let created = change(1, 1, 1, &[], vec![field_op::create(T, TASK), title("a")]);
    let moved = change(
        1,
        2,
        3,
        &[&created],
        vec![field_op::move_object(
            T,
            TASK,
            &write(Value::Bytes(NEW.0.to_vec())),
        )],
    );
    let offline = change(2, 1, 2, &[&created], vec![title("offline")]);

    let db = TempDb::new("later-edits-reopen");
    let before = {
        let mut view = View::open(&db.0, &lock()).unwrap();
        let mut control = checks();
        for frame in [&created, &moved, &offline] {
            view.receive(frame, &mut control).unwrap();
        }
        moves::later_edits_in(
            view.log(SPACE).unwrap(),
            &control,
            &mut view.stored_frames(),
        )
    };
    assert_eq!(before[&T].len(), 1, "the offline title is a later edit");

    let view = View::open_with(&db.0, &lock(), |_| checks()).unwrap();
    let after = moves::later_edits_in(
        view.log(SPACE).unwrap(),
        &checks(),
        &mut view.stored_frames(),
    );
    assert_eq!(after, before);
}
