//! How the operations of basic values, sets and links travel in a change,
//! as RFC 0004 Amendment 1 defines it: the operation number in key 4 of
//! RFC 0002's operation map, the arguments as a map in key 5.

// The red step's stub: the bodies come with the green step.
#![allow(unused_variables)]

use crate::cbor::Value;
use crate::change::{ChangeError, Operation};
use crate::clock::Hlc;
use crate::field::FieldType;
use crate::ids::{MemberId, ObjectId, OperationId};
use crate::log::OperationParkReason;

/// Operation number 1: set, on a basic value or a link.
pub const SET: u64 = 1;
/// Operation number 2: add, on a set or links.
pub const ADD: u64 = 2;
/// Operation number 3: remove, on a set or links.
pub const REMOVE: u64 = 3;

/// What a field operation takes from its change rather than from its own
/// bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Origin {
    /// The change id and the operation's position in the payload.
    pub id: OperationId,
    /// The member that signed the change.
    pub author: MemberId,
    /// The payload's clock.
    pub stamp: Hlc,
}

/// A field type whose operations travel in a change.
pub trait Encoding: FieldType {
    /// The operation number and the arguments of `op`.
    fn arguments(&self, op: &Self::Operation) -> (u64, Value);

    /// Reads `operation`, an operation on a field of this type, or says
    /// why it is parked.
    fn decode(
        &self,
        operation: &Operation,
        origin: Origin,
    ) -> Result<Self::Operation, OperationParkReason>;
}

/// The operation map that writes `op` to `field` of `object` in
/// `collection`.
pub fn encode<T: Encoding>(
    field_type: &T,
    op: &T::Operation,
    object: ObjectId,
    collection: &str,
    field: &str,
) -> Operation {
    todo!()
}

impl Operation {
    /// The canonical bytes of this operation map.
    pub fn encode(&self) -> Vec<u8> {
        todo!()
    }

    /// Decodes canonical bytes of one operation map, as it sits in a
    /// payload's operation array.
    pub fn decode(bytes: &[u8]) -> Result<Operation, ChangeError> {
        todo!()
    }
}

impl Encoding for crate::basic::BasicValue {
    fn arguments(&self, op: &crate::basic::Write) -> (u64, Value) {
        todo!()
    }

    fn decode(
        &self,
        operation: &Operation,
        origin: Origin,
    ) -> Result<crate::basic::Write, OperationParkReason> {
        todo!()
    }
}

impl Encoding for crate::link::Link {
    fn arguments(&self, op: &crate::basic::Write) -> (u64, Value) {
        todo!()
    }

    fn decode(
        &self,
        operation: &Operation,
        origin: Origin,
    ) -> Result<crate::basic::Write, OperationParkReason> {
        todo!()
    }
}

impl Encoding for crate::set::Set {
    fn arguments(&self, op: &crate::set::Edit) -> (u64, Value) {
        todo!()
    }

    fn decode(
        &self,
        operation: &Operation,
        origin: Origin,
    ) -> Result<crate::set::Edit, OperationParkReason> {
        todo!()
    }
}

impl Encoding for crate::link::Links {
    fn arguments(&self, op: &crate::set::Edit) -> (u64, Value) {
        todo!()
    }

    fn decode(
        &self,
        operation: &Operation,
        origin: Origin,
    ) -> Result<crate::set::Edit, OperationParkReason> {
        todo!()
    }
}
