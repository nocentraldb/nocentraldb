//! Test doubles for a member that receives changes (NOC-28): the checks of
//! RFC 0002, "What a member does with a change it receives", steps 3
//! to 5, and a fold that records which field
//! operations it folded and keeps the objects that creates and deletes fold
//! into (NOC-39). The doubles stand in for real keys, so that a test
//! of the steps can name a change's author and key generation by a small
//! number; `tests/checks.rs` runs the same steps with real keys.
//!
//! Each test file includes this module with `#[path]`; not every file uses
//! every helper.
#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};

use ncdb_core::change::{Change, Header, Kind, Operation, Payload};
use ncdb_core::clock::Hlc;
use ncdb_core::field_op::Origin;
use ncdb_core::ids::{MemberId, ObjectId, OperationId, SpaceId};
use ncdb_core::log::{Log, OperationParkReason, ParkReason, RejectReason, State};
use ncdb_core::object::{Object, ObjectOperation};
use ncdb_core::receive::{Checks, Fold, Stop};

/// The space every test change belongs to.
pub const SPACE: SpaceId = SpaceId([1; 16]);
/// The member that founded the space: known and a member at every key
/// generation from the start, standing in for the space's first entry.
pub const FOUNDER: u8 = 0;
/// The suite the doubles know.
pub const SUITE: u64 = 1;
/// A signature the double accepts; any other is bad.
pub const GOOD: &[u8] = &[1];
/// A signature the double refuses.
pub const BAD: &[u8] = &[0];
/// A sealed payload the double cannot decrypt, as if the AEAD failed.
pub const GARBLED: &[u8] = b"garbled";
/// The `model.id` of the member's lock, which its fold reports.
pub const MODEL: [u8; 16] = [0x6d; 16];

/// The collections and the field the recorder's model knows.
pub const COLLECTION: &str = "task";
pub const OTHER: &str = "project";
pub const FIELD: &str = "title";

/// The member id of a simulated member.
pub fn member(n: u8) -> MemberId {
    MemberId([n; 32])
}

/// The control state of the double: membership entries and the key
/// generations this member holds an envelope for. A function of the set of
/// control changes it was given.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Control {
    /// (member, key generation) pairs the membership entries name.
    pub members: BTreeSet<(MemberId, u64)>,
    /// Key generations this member holds a key for.
    pub keys: BTreeSet<u64>,
}

impl Control {
    fn known(&self, author: MemberId) -> bool {
        author == member(FOUNDER) || self.members.iter().any(|(m, _)| *m == author)
    }
}

impl Checks for Control {
    fn knows_suite(&self, suite: u64) -> bool {
        suite == SUITE
    }

    fn verify(&self, change: &Change) -> Result<(), Stop> {
        if !self.known(change.header().author) {
            Err(Stop::Rejected(RejectReason::UnknownAuthor))
        } else if change.signature() == GOOD {
            Ok(())
        } else {
            Err(Stop::Rejected(RejectReason::BadSignature))
        }
    }

    fn is_member(&self, change: &Change) -> bool {
        let header = change.header();
        header.author == member(FOUNDER)
            || self
                .members
                .contains(&(header.author, header.key_generation))
    }

    fn open(&self, change: &Change) -> Result<Payload, Stop> {
        let header = change.header();
        if !self.keys.contains(&header.key_generation) {
            Err(Stop::Rejected(RejectReason::NoKey))
        } else if header.payload == GARBLED {
            Err(Stop::Rejected(RejectReason::Undecryptable))
        } else {
            // The double's "ciphertext" is the plaintext payload map.
            Payload::decode(&header.payload)
                .map_err(|_| Stop::Parked(ParkReason::UnknownPayloadFormat))
        }
    }

    fn add_control(&mut self, change: &Change) {
        match change.header().payload.as_slice() {
            [0, m, generation] => {
                self.members.insert((member(*m), u64::from(*generation)));
            }
            [1, generation] => {
                self.keys.insert(u64::from(*generation));
            }
            _ => {}
        }
    }
}

/// A fold stub: knows two collections, the first with one field, records
/// the field operations it folded, each with the field it folded into, and
/// keeps the objects of the copy.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Recorder {
    pub folded: BTreeSet<OperationId>,
    pub fields: BTreeMap<OperationId, (ObjectId, String, String)>,
    pub objects: BTreeMap<ObjectId, Object>,
}

impl Fold for Recorder {
    fn model_id(&self) -> [u8; 16] {
        MODEL
    }

    fn knows_collection(&self, collection: &str) -> bool {
        collection == COLLECTION || collection == OTHER
    }

    fn knows_field(&self, collection: &str, field: &str) -> bool {
        collection == COLLECTION && field == FIELD
    }

    fn fold(&mut self, origin: Origin, operation: &Operation) -> Result<(), OperationParkReason> {
        self.folded.insert(origin.id);
        let field = operation.field.clone().expect("a field operation");
        let target = (operation.object, operation.collection.clone(), field);
        self.fields.insert(origin.id, target);
        Ok(())
    }

    fn fold_object(&mut self, object: ObjectId, operation: &ObjectOperation) {
        self.objects.entry(object).or_default().apply(operation);
    }

    fn clear_field(&mut self, object: ObjectId, collection: &str, field: &str) {
        let target = (object, collection.to_owned(), field.to_owned());
        self.fields.retain(|_, folded_into| *folded_into != target);
        let fields = &self.fields;
        self.folded.retain(|id| fields.contains_key(id));
    }

    fn clear_object(&mut self, object: ObjectId) {
        self.objects.remove(&object);
    }
}

/// An operation on the known collection and field.
pub fn set_title(object: u8) -> Operation {
    Operation::new(
        ObjectId([object; 16]),
        COLLECTION.into(),
        Some(FIELD.into()),
        1,
        None,
    )
}

/// The canonical bytes of a frame with this header and signature.
pub fn frame(header: Header, signature: &[u8]) -> Vec<u8> {
    Change::new(header, signature.to_vec()).bytes().to_vec()
}

/// A header of a sealed change whose "ciphertext" is `payload`.
pub fn sealed_header(author: u8, sequence: u64, generation: u64, payload: Vec<u8>) -> Header {
    Header::new(
        SPACE,
        member(author),
        sequence,
        generation,
        Kind::SEALED,
        payload,
        SUITE,
    )
}

/// The plaintext payload map of these operations.
pub fn payload(operations: Vec<Operation>) -> Vec<u8> {
    Payload::new(Hlc(1), BTreeSet::new(), operations).encode()
}

/// A well-signed sealed change of `author` at `generation`.
pub fn sealed(author: u8, sequence: u64, generation: u64, operations: Vec<Operation>) -> Vec<u8> {
    frame(
        sealed_header(author, sequence, generation, payload(operations)),
        GOOD,
    )
}

/// A header of a control change with this body.
pub fn control_header(author: u8, sequence: u64, body: Vec<u8>) -> Header {
    Header::new(
        SPACE,
        member(author),
        sequence,
        0,
        Kind::CONTROL,
        body,
        SUITE,
    )
}

/// A membership entry, signed by `author`: `m` is a member at `generation`.
pub fn membership(author: u8, sequence: u64, m: u8, generation: u8) -> Vec<u8> {
    frame(
        control_header(author, sequence, vec![0, m, generation]),
        GOOD,
    )
}

/// A key envelope for this member, signed by `author`, for `generation`.
pub fn envelope(author: u8, sequence: u64, generation: u8) -> Vec<u8> {
    frame(control_header(author, sequence, vec![1, generation]), GOOD)
}

/// A member's log with its control state and fold.
#[derive(Clone, Debug)]
pub struct Member {
    pub log: Log,
    pub control: Control,
    pub fold: Recorder,
}

impl Default for Member {
    fn default() -> Member {
        Member {
            log: Log::new(SPACE),
            control: Control::default(),
            fold: Recorder::default(),
        }
    }
}

impl Member {
    /// Receives a frame; panics when it is not appended.
    pub fn receive(&mut self, bytes: &[u8]) -> Change {
        self.log
            .receive(bytes, &mut self.control, &mut self.fold)
            .expect("the change is appended");
        Change::decode(bytes).expect("a well-formed frame")
    }

    /// The state of a change this member holds.
    pub fn state(&self, change: &Change) -> State {
        self.log
            .state(change.id())
            .cloned()
            .expect("the log holds the change with a state")
    }
}
