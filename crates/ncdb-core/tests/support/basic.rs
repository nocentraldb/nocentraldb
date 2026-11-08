//! The adapter that plugs basic values (`ncdb_core::basic`) into the
//! convergence harness: the harness's small operation ids and members
//! become the core's operation and member ids, and a member's write is made
//! with the core's own `set` from its simulated copy.

// Every test binary that declares `mod support;` compiles the adapter but
// not every one uses it.
#![allow(dead_code)]

use core::marker::PhantomData;
use std::collections::BTreeSet;

use ncdb_core::basic::{BasicValue, Kind, Read, State, Write};
use ncdb_core::cbor::Value;
use ncdb_core::clock::Hlc;
use ncdb_core::field::{FieldType, Outcome as CoreOutcome};
use ncdb_core::ids::{ChangeId, MemberId, ObjectId, OperationId, SpaceId};
use ncdb_core::link::{self, Link};
use proptest::prelude::*;

use super::convergence::{Author, Generator, Member, Mergeable, Op, OpId, Outcome};

/// The schema a test runs basic values with: the field the copies read
/// with, and the values members write.
pub trait Schema: core::fmt::Debug + 'static {
    /// The field type the copies apply and read with: a basic value, or a
    /// type that merges as one (a link).
    type Field: FieldType<State = State, Operation = Write, Read = Read>;
    /// The field as the copies' schema declares it.
    fn field() -> Self::Field;
    /// A member's write of `value`, with the writers' schema; a newer
    /// schema may allow values the copies' schema does not.
    fn set(state: &State, id: OperationId, author: MemberId, stamp: Hlc, value: Value) -> Write;
    /// The values members write, all allowed by the writers' schema.
    fn values() -> Vec<Value>;
}

/// A choice whose writers already know an option the readers do not.
#[derive(Debug)]
pub struct Choice;

impl Schema for Choice {
    type Field = BasicValue;

    fn field() -> BasicValue {
        BasicValue {
            kind: Kind::Choice(["open", "done"].map(String::from).into()),
        }
    }

    fn set(state: &State, id: OperationId, author: MemberId, stamp: Hlc, value: Value) -> Write {
        let writer = BasicValue {
            kind: Kind::Choice(["open", "done", "archived"].map(String::from).into()),
        };
        writer
            .set(state, id, author, stamp, value)
            .expect("the writer's schema allows its values")
    }

    fn values() -> Vec<Value> {
        ["open", "done", "archived"]
            .map(|o| Value::Text(o.to_owned()))
            .into_iter()
            .chain([Value::Null])
            .collect()
    }
}

/// Floats, with two NaNs of different bits and both zeros.
#[derive(Debug)]
pub struct Float;

impl Schema for Float {
    type Field = BasicValue;

    fn field() -> BasicValue {
        BasicValue { kind: Kind::Float }
    }

    fn set(state: &State, id: OperationId, author: MemberId, stamp: Hlc, value: Value) -> Write {
        Self::field()
            .set(state, id, author, stamp, value)
            .expect("the writer's schema allows its values")
    }

    fn values() -> Vec<Value> {
        vec![
            Value::Float(f64::NAN),
            Value::Float(f64::from_bits(f64::NAN.to_bits() ^ 1)),
            Value::Float(0.0),
            Value::Float(-0.0),
            Value::Float(1.5),
            Value::Null,
        ]
    }
}

/// The space the links of the harness are written in.
pub const SPACE: SpaceId = SpaceId([1; 16]);

/// A link (one target) to one of three objects, or cleared.
#[derive(Debug)]
pub struct Target;

impl Schema for Target {
    type Field = Link;

    fn field() -> Link {
        Link {
            collections: ["tasks".to_owned()].into(),
        }
    }

    /// Writes with `Link::set`, in the link's own space.
    fn set(state: &State, id: OperationId, author: MemberId, stamp: Hlc, value: Value) -> Write {
        let target = match value {
            Value::Bytes(bytes) => Some(link::Target {
                space: SPACE,
                object: ObjectId(bytes.try_into().expect("an object id")),
            }),
            _ => None,
        };
        Self::field()
            .set(state, id, author, stamp, SPACE, target)
            .expect("a target in the link's space")
    }

    fn values() -> Vec<Value> {
        (1..=3)
            .map(|n| link::value(ObjectId([n; 16])))
            .chain([Value::Null])
            .collect()
    }
}

/// The number of the schema's values, as the `u8` the generator draws.
fn value_count<S: Schema>() -> u8 {
    u8::try_from(S::values().len()).expect("a few values")
}

/// The core's operation id for the harness's: the id, big-endian, at the
/// start of the change id, so both order alike.
pub fn operation_id(id: OpId) -> OperationId {
    let mut change = [0u8; 32];
    change[..4].copy_from_slice(&id.to_be_bytes());
    OperationId {
        change: ChangeId(change),
        position: 0,
    }
}

/// The harness's operation id for the core's.
pub fn op_id(id: OperationId) -> OpId {
    let mut bytes = [0u8; 4];
    bytes.copy_from_slice(&id.change.0[..4]);
    OpId::from_be_bytes(bytes)
}

pub fn member_id(member: Member) -> MemberId {
    MemberId([member; 32])
}

/// One write as the harness carries it; the id and author come with the
/// harness's operation.
#[derive(Clone, Debug, PartialEq)]
pub struct Body {
    pub stamp: u64,
    pub value: Value,
    pub replaces: BTreeSet<OpId>,
}

/// The core's write for a harness operation.
pub fn write(op: &Op<Body>) -> Write {
    Write {
        id: operation_id(op.id),
        author: member_id(op.author),
        stamp: Hlc(op.body.stamp),
        value: op.body.value.clone(),
        replaces: op.body.replaces.iter().copied().map(operation_id).collect(),
    }
}

/// A basic value field's state, as the harness drives it.
#[derive(Debug)]
pub struct Basic<S> {
    pub state: State,
    schema: PhantomData<S>,
}

impl<S> Clone for Basic<S> {
    fn clone(&self) -> Self {
        Basic {
            state: self.state.clone(),
            schema: PhantomData,
        }
    }
}

impl<S> Default for Basic<S> {
    fn default() -> Self {
        Basic {
            state: State::default(),
            schema: PhantomData,
        }
    }
}

/// The core's outcome in the harness's terms.
pub fn outcome(outcome: CoreOutcome) -> Outcome {
    match outcome {
        CoreOutcome::Visible => Outcome::Visible,
        CoreOutcome::Replaced(by) => Outcome::Replaced(op_id(by)),
        CoreOutcome::Lost => Outcome::Lost,
    }
}

impl<S: Schema> Mergeable for Basic<S> {
    type Change = Body;
    type Read = Read;

    fn apply(&mut self, op: &Op<Body>) {
        S::field().apply(&mut self.state, &write(op));
    }

    fn state_bytes(&self) -> Vec<u8> {
        S::field().state_bytes(&self.state)
    }

    fn read(&self) -> Read {
        S::field().read(&self.state)
    }

    fn outcome(&self, id: OpId) -> Outcome {
        let core = S::field().outcome(&self.state, operation_id(id));
        outcome(core.expect("the harness asks only for received operations"))
    }
}

/// Generates writes the way a member makes them, with the core's `set`:
/// the stamp is above every stamp its author had seen (O3 of RFC 0004),
/// so concurrent writes often share a stamp, and it replaces the live
/// writes of its author's copy. Mixes in malformed writes (C5): any stamp,
/// any value, and replaced ids that may be unknown, the write itself, later
/// writes, or form a cycle.
pub struct Writes<S>(PhantomData<S>);

pub fn writes<S>() -> Writes<S> {
    Writes(PhantomData)
}

impl<S> AsRef<Basic<S>> for Basic<S> {
    fn as_ref(&self) -> &Basic<S> {
        self
    }
}

/// Also generates for broken variants that wrap a [`Basic`].
impl<S: Schema, M: Mergeable<Change = Body> + AsRef<Basic<S>>> Generator<M> for Writes<S> {
    /// How far the stamp moves past the highest stamp the author had seen,
    /// and which of the schema's values it writes. Both are `u8`: a
    /// `usize` is drawn differently on 32-bit wasm32 and 64-bit native
    /// targets, which breaks C3's comparison.
    type Raw = (u8, u8);

    fn raw(&self, _: Member) -> BoxedStrategy<(u8, u8)> {
        (0u8..2, 0..value_count::<S>()).boxed()
    }

    fn make(&self, author: &Author<'_, M>, &(step, value): &(u8, u8)) -> Body {
        let highest = author.seen.iter().map(|op| op.body.stamp).max();
        let stamp = highest
            .map_or(0, |stamp| stamp.saturating_add(1))
            .saturating_add(u64::from(step));
        let write = S::set(
            &author.copy.as_ref().state,
            operation_id(author.id),
            member_id(author.member),
            Hlc(stamp),
            S::values()[usize::from(value)].clone(),
        );
        Body {
            stamp,
            value: write.value,
            replaces: write.replaces.into_iter().map(op_id).collect(),
        }
    }

    fn malformed(&self) -> Option<BoxedStrategy<Body>> {
        let value = prop_oneof![
            (0..value_count::<S>()).prop_map(|i| S::values()[usize::from(i)].clone()),
            any::<u64>().prop_map(Value::Unsigned),
            proptest::collection::vec(any::<u8>(), 0..3).prop_map(Value::Bytes),
            any::<bool>().prop_map(Value::Bool),
        ];
        let replaces = proptest::collection::btree_set(0..8 as OpId, 0..4);
        Some(
            (prop_oneof![any::<u64>(), 0u64..4], value, replaces)
                .prop_map(|(stamp, value, replaces)| Body {
                    stamp,
                    value,
                    replaces,
                })
                .boxed(),
        )
    }
}
