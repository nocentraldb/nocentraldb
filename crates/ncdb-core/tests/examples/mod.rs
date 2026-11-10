//! The byte-exact examples of RFC 0004 Amendment 1 ("Byte-exact examples")
//! and Amendment 4 (E15 to E17), with the ids, members and types they use.
//! The vectors are written by
//! `docs/rfc/vectors/0004/generate.py` with cbor2, independently of this
//! crate; include_str! puts them into the test binary, so the tests also
//! run on wasm32, where they cannot read files.

#![allow(dead_code)]

use std::collections::BTreeSet;

use ncdb_core::basic::{self, BasicValue, Write};
use ncdb_core::cbor::Value;
use ncdb_core::change::Operation;
use ncdb_core::clock::Hlc;
use ncdb_core::field_op::Origin;
use ncdb_core::ids::{ChangeId, MemberId, ObjectId, OperationId, SpaceId};
use ncdb_core::link::{Link, Links, Target};
use ncdb_core::reference::{self, Reference};
use ncdb_core::set::{Action, Edit, Set};

pub const E1: &str = include_str!("../../../../docs/rfc/vectors/0004/E1.hex");
pub const E2: &str = include_str!("../../../../docs/rfc/vectors/0004/E2.hex");
pub const E3: &str = include_str!("../../../../docs/rfc/vectors/0004/E3.hex");
pub const E4: &str = include_str!("../../../../docs/rfc/vectors/0004/E4.hex");
pub const E5: &str = include_str!("../../../../docs/rfc/vectors/0004/E5.hex");
pub const E6: &str = include_str!("../../../../docs/rfc/vectors/0004/E6.hex");
pub const E7: &str = include_str!("../../../../docs/rfc/vectors/0004/E7.hex");
pub const E8: &str = include_str!("../../../../docs/rfc/vectors/0004/E8.hex");
pub const E9: &str = include_str!("../../../../docs/rfc/vectors/0004/E9.hex");
pub const E10: &str = include_str!("../../../../docs/rfc/vectors/0004/E10.hex");
pub const E11: &str = include_str!("../../../../docs/rfc/vectors/0004/E11.hex");
pub const E12: &str = include_str!("../../../../docs/rfc/vectors/0004/E12.hex");
pub const E13: &str = include_str!("../../../../docs/rfc/vectors/0004/E13.hex");
pub const E14: &str = include_str!("../../../../docs/rfc/vectors/0004/E14.hex");
pub const E15: &str = include_str!("../../../../docs/rfc/vectors/0004/E15.hex");
pub const E16: &str = include_str!("../../../../docs/rfc/vectors/0004/E16.hex");
pub const E17: &str = include_str!("../../../../docs/rfc/vectors/0004/E17.hex");

pub fn hex(text: &str) -> Vec<u8> {
    let text: String = text.split_whitespace().collect();
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

pub fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Object `T1`: 16 bytes of `11`.
pub const T1: ObjectId = ObjectId([0x11; 16]);
/// Link target `T2`: 16 bytes of `22`.
pub const T2: ObjectId = ObjectId([0x22; 16]);
/// Space `S` of Amendment 4's examples: 16 bytes of `33`.
pub const S: SpaceId = SpaceId([0x33; 16]);
/// Customer `C1`, in `S`: 16 bytes of `44`.
pub const C1: ObjectId = ObjectId([0x44; 16]);
/// Customer `C2`, in `S`: 16 bytes of `55`.
pub const C2: ObjectId = ObjectId([0x55; 16]);
/// Change id `A`: 32 bytes of `aa`.
pub const A: ChangeId = ChangeId([0xaa; 32]);
/// Change id `B`: 32 bytes of `bb`.
pub const B: ChangeId = ChangeId([0xbb; 32]);

pub fn id(change: ChangeId, position: u64) -> OperationId {
    OperationId { change, position }
}

pub fn ids(items: &[OperationId]) -> BTreeSet<OperationId> {
    items.iter().copied().collect()
}

/// The change an example operation sits in. The bytes do not carry it.
pub fn origin() -> Origin {
    Origin {
        id: id(ChangeId([0xcc; 32]), 0),
        author: MemberId([7; 32]),
        stamp: Hlc(42),
    }
}

pub fn text(text: &str) -> Value {
    Value::Text(text.to_owned())
}

pub fn target(object: ObjectId) -> Value {
    Value::Bytes(object.0.to_vec())
}

// The fields of the collection `tasks`.
pub fn title() -> BasicValue {
    BasicValue {
        kind: basic::Kind::Text,
    }
}

pub fn priority() -> BasicValue {
    BasicValue {
        kind: basic::Kind::Integer,
    }
}

pub fn tags() -> Set {
    Set {
        kind: basic::Kind::Text,
    }
}

pub fn parent() -> Link {
    Link {
        collections: ["tasks".to_owned()].into(),
    }
}

pub fn depends_on() -> Links {
    Links {
        collections: ["tasks".to_owned()].into(),
    }
}

// The field `customer` of the collection `orders` (Amendment 4).
pub fn customer() -> Reference {
    Reference {
        collections: ["customers".to_owned()].into(),
    }
}

/// The reference to `object` in `S`, as a set writes it.
pub fn reference(object: ObjectId) -> Value {
    reference::value(Target { space: S, object })
}

/// A set, from the example's origin.
pub fn write(value: Value, replaces: &[OperationId]) -> Write {
    let origin = origin();
    Write {
        id: origin.id,
        author: origin.author,
        stamp: origin.stamp,
        value,
        replaces: ids(replaces),
    }
}

/// An add, from the example's origin.
pub fn add(element: Value, replaces: &[OperationId]) -> Edit {
    edit(
        element,
        Action::Add {
            replaces: ids(replaces),
        },
    )
}

/// A remove, from the example's origin.
pub fn remove(element: Value, observed: &[OperationId]) -> Edit {
    edit(
        element,
        Action::Remove {
            observed: ids(observed),
        },
    )
}

fn edit(element: Value, action: Action) -> Edit {
    let origin = origin();
    Edit {
        id: origin.id,
        author: origin.author,
        stamp: origin.stamp,
        element,
        action,
    }
}

/// The operation map of example `hex`.
pub fn operation(hex: &str) -> Operation {
    Operation::decode(&self::hex(hex)).expect("an example decodes")
}
