//! The adapter that plugs sets (`ncdb_core::set`) and links
//! (`ncdb_core::link::Links`) into the convergence harness: the harness's
//! small operation ids and members become the core's ids, and a member's
//! addition or removal is made with the core's own `add` and `remove` from
//! its simulated copy.

// Every test binary that declares `mod support;` compiles the adapter but
// not every one uses it.
#![allow(dead_code)]

use core::marker::PhantomData;
use std::collections::BTreeSet;

use ncdb_core::basic::Kind;
use ncdb_core::cbor::Value;
use ncdb_core::clock::Hlc;
use ncdb_core::field::FieldType;
use ncdb_core::ids::ObjectId;
use ncdb_core::link::{self, Links};
use ncdb_core::set::{Action, Edit, Read, Set, State};
use proptest::prelude::*;

use super::basic::{member_id, op_id, operation_id, outcome};
use super::convergence::{Author, Generator, Member, Mergeable, Op, OpId, Outcome};

/// The schema a test runs sets with.
pub trait Schema: core::fmt::Debug + 'static {
    /// The field type the copies apply and read with: a set, or a type
    /// that merges as one (links).
    type Field: FieldType<State = State, Operation = Edit, Read = Read>;
    /// The field as the copies' schema declares it.
    fn field() -> Self::Field;
    /// The set the writers make operations with; a newer schema may allow
    /// elements the copies' schema does not.
    fn writer() -> Set;
    /// The elements members add and remove, all allowed by `writer()`. A
    /// few, so that members often add and remove the same element.
    fn values() -> Vec<Value>;
}

/// Tags from a choice whose writers already know an option the readers do
/// not.
#[derive(Debug)]
pub struct Tags;

impl Schema for Tags {
    type Field = Set;

    fn field() -> Set {
        Set {
            kind: Kind::Choice(["urgent", "later"].map(String::from).into()),
        }
    }

    fn writer() -> Set {
        Set {
            kind: Kind::Choice(["urgent", "later", "blocked"].map(String::from).into()),
        }
    }

    fn values() -> Vec<Value> {
        ["urgent", "later", "blocked"]
            .map(|o| Value::Text(o.to_owned()))
            .into()
    }
}

/// Floats, with two NaNs of different bits, which are one element, and
/// both zeros, which are two.
#[derive(Debug)]
pub struct Floats;

impl Schema for Floats {
    type Field = Set;

    fn field() -> Set {
        Set { kind: Kind::Float }
    }

    fn writer() -> Set {
        Self::field()
    }

    fn values() -> Vec<Value> {
        vec![
            Value::Float(f64::NAN),
            Value::Float(f64::from_bits(f64::NAN.to_bits() ^ 1)),
            Value::Float(0.0),
            Value::Float(-0.0),
        ]
    }
}

/// Links (many targets) to three objects.
#[derive(Debug)]
pub struct Targets;

impl Schema for Targets {
    type Field = Links;

    fn field() -> Links {
        Links {
            collections: ["tasks".to_owned()].into(),
        }
    }

    fn writer() -> Set {
        Links::field()
    }

    fn values() -> Vec<Value> {
        (1..=3).map(|n| link::value(ObjectId([n; 16]))).collect()
    }
}

/// The number of the schema's values, as the `u8` the generator draws.
fn value_count<S: Schema>() -> u8 {
    u8::try_from(S::values().len()).expect("a few values")
}

/// One set operation as the harness carries it; the id and author come
/// with the harness's operation.
#[derive(Clone, Debug, PartialEq)]
pub struct Body {
    pub stamp: u64,
    pub element: Value,
    /// Whether it removes; an addition otherwise.
    pub remove: bool,
    /// An addition's replaced removals, or a removal's observed tags.
    pub names: BTreeSet<OpId>,
}

/// The core's operation for a harness operation.
pub fn edit(op: &Op<Body>) -> Edit {
    let names = op.body.names.iter().copied().map(operation_id).collect();
    Edit {
        id: operation_id(op.id),
        author: member_id(op.author),
        stamp: Hlc(op.body.stamp),
        element: op.body.element.clone(),
        action: if op.body.remove {
            Action::Remove { observed: names }
        } else {
            Action::Add { replaces: names }
        },
    }
}

/// A set field's state, as the harness drives it.
#[derive(Debug)]
pub struct SetOf<S> {
    pub state: State,
    schema: PhantomData<S>,
}

impl<S> Clone for SetOf<S> {
    fn clone(&self) -> Self {
        SetOf {
            state: self.state.clone(),
            schema: PhantomData,
        }
    }
}

impl<S> Default for SetOf<S> {
    fn default() -> Self {
        SetOf {
            state: State::default(),
            schema: PhantomData,
        }
    }
}

impl<S> AsRef<SetOf<S>> for SetOf<S> {
    fn as_ref(&self) -> &SetOf<S> {
        self
    }
}

impl<S: Schema> Mergeable for SetOf<S> {
    type Change = Body;
    type Read = Read;

    fn apply(&mut self, op: &Op<Body>) {
        S::field().apply(&mut self.state, &edit(op));
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

/// Generates additions and removals the way a member makes them, with the
/// core's `add` and `remove`: a removal observes the live additions of its
/// author's copy, an addition replaces the removals it held. The stamp is
/// above every stamp its author had seen. Mixes in malformed operations
/// (C5): any stamp, any element, and named ids that may be unknown, the
/// operation itself, of the wrong kind, or form a cycle.
pub struct Edits<S>(PhantomData<S>);

pub fn edits<S>() -> Edits<S> {
    Edits(PhantomData)
}

/// Also generates for broken variants that wrap a [`SetOf`].
impl<S: Schema, M: Mergeable<Change = Body> + AsRef<SetOf<S>>> Generator<M> for Edits<S> {
    /// How far the stamp moves past the highest stamp the author had seen,
    /// whether it removes (odd) or adds, and which of the schema's values.
    /// All `u8`: a `usize` is drawn differently on 32-bit wasm32 and 64-bit
    /// native targets, which breaks C3's comparison.
    type Raw = (u8, u8, u8);

    fn raw(&self, _: Member) -> BoxedStrategy<(u8, u8, u8)> {
        (0u8..2, 0u8..2, 0..value_count::<S>()).boxed()
    }

    fn make(&self, author: &Author<'_, M>, &(step, remove, value): &(u8, u8, u8)) -> Body {
        let highest = author.seen.iter().map(|op| op.body.stamp).max();
        let stamp = highest
            .map_or(0, |stamp| stamp.saturating_add(1))
            .saturating_add(u64::from(step));
        let state = &author.copy.as_ref().state;
        let (id, member) = (operation_id(author.id), member_id(author.member));
        let element = S::values()[usize::from(value)].clone();
        let remove = remove % 2 == 1;
        let made = if remove {
            S::writer().remove(state, id, member, Hlc(stamp), element)
        } else {
            S::writer().add(state, id, member, Hlc(stamp), element)
        };
        let made = made.expect("the writer's schema allows its values");
        let names = match made.action {
            Action::Add { replaces } => replaces,
            Action::Remove { observed } => observed,
        };
        Body {
            stamp,
            element: made.element,
            remove,
            names: names.into_iter().map(op_id).collect(),
        }
    }

    fn malformed(&self) -> Option<BoxedStrategy<Body>> {
        let element = prop_oneof![
            (0..value_count::<S>()).prop_map(|i| S::values()[usize::from(i)].clone()),
            any::<u64>().prop_map(Value::Unsigned),
            proptest::collection::vec(any::<u8>(), 0..3).prop_map(Value::Bytes),
            Just(Value::Null),
        ];
        let names = proptest::collection::btree_set(0..8 as OpId, 0..4);
        Some(
            (
                prop_oneof![any::<u64>(), 0u64..4],
                element,
                any::<bool>(),
                names,
            )
                .prop_map(|(stamp, element, remove, names)| Body {
                    stamp,
                    element,
                    remove,
                    names,
                })
                .boxed(),
        )
    }
}
