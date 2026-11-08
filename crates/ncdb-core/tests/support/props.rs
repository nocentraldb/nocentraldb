//! Property tests with a fixed seed, natively and on wasm32, and the
//! generators the tests of the change and the log share.
//!
//! Each test file includes this module with `#[path]`, so it stays apart
//! from the convergence harness in `mod.rs`; not every file uses every
//! generator.
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fmt::Debug;

use ncdb_core::cbor::{Key, Value};
use ncdb_core::change::{Header, Kind, Operation, Payload};
use ncdb_core::clock::Hlc;
use ncdb_core::ids::{ChangeId, MemberId, ObjectId, SpaceId};
use proptest::prelude::*;
use proptest::test_runner::{Config, RngAlgorithm, TestError, TestRng, TestRunner};

/// Runs `test` on `cases` generated inputs, from a fixed seed, so every run
/// on every target checks the same inputs. Panics with the smallest failing
/// input.
pub fn check<S>(cases: u32, strategy: S, test: impl Fn(S::Value) -> Result<(), TestCaseError>)
where
    S: Strategy,
    S::Value: Debug,
{
    let rng = TestRng::from_seed(RngAlgorithm::ChaCha, &[7; 32]);
    let config = Config {
        cases,
        failure_persistence: None,
        ..Config::default()
    };
    let mut runner = TestRunner::new_with_rng(config, rng);
    match runner.run(&strategy, test) {
        Ok(()) => {}
        Err(TestError::Fail(reason, input)) => panic!("{reason}; smallest input: {input:?}"),
        Err(TestError::Abort(reason)) => panic!("aborted: {reason}"),
    }
}

/// Floats that reach every encoded width: half, single and double
/// precision, subnormals, infinities, NaN and both zeros.
pub fn float() -> BoxedStrategy<f64> {
    prop_oneof![
        any::<f64>(),
        any::<f32>().prop_map(f64::from),
        any::<i16>().prop_map(f64::from),
        (any::<i8>(), -30i32..16).prop_map(|(m, e)| f64::from(m) * 2f64.powi(e)),
        Just(f64::NAN),
        Just(-f64::NAN),
        Just(0.0),
        Just(-0.0),
        Just(f64::INFINITY),
        Just(f64::NEG_INFINITY),
    ]
    .boxed()
}

/// Text of up to `max` characters, any of Unicode's.
pub fn text(max: usize) -> BoxedStrategy<String> {
    proptest::collection::vec(any::<char>(), 0..=max)
        .prop_map(String::from_iter)
        .boxed()
}

/// A map key of the profile.
pub fn key() -> BoxedStrategy<Key> {
    prop_oneof![
        any::<u64>().prop_map(Key::Unsigned),
        (0u64..30).prop_map(Key::Unsigned),
        text(6).prop_map(Key::Text),
    ]
    .boxed()
}

/// Any value of the profile, nested a few levels deep.
pub fn value() -> BoxedStrategy<Value> {
    let leaf = prop_oneof![
        any::<u64>().prop_map(Value::Unsigned),
        (0u64..300).prop_map(Value::Unsigned),
        any::<u64>().prop_map(Value::Negative),
        proptest::collection::vec(any::<u8>(), 0..40).prop_map(Value::Bytes),
        text(12).prop_map(Value::Text),
        any::<bool>().prop_map(Value::Bool),
        Just(Value::Null),
        float().prop_map(Value::Float),
    ];
    leaf.prop_recursive(3, 32, 6, |inner| {
        prop_oneof![
            proptest::collection::vec(inner.clone(), 0..6).prop_map(Value::Array),
            proptest::collection::btree_map(key(), inner, 0..6).prop_map(Value::Map),
        ]
    })
    .boxed()
}

pub fn space_id() -> BoxedStrategy<SpaceId> {
    any::<[u8; 16]>().prop_map(SpaceId).boxed()
}

pub fn object_id() -> BoxedStrategy<ObjectId> {
    any::<[u8; 16]>().prop_map(ObjectId).boxed()
}

pub fn member_id() -> BoxedStrategy<MemberId> {
    any::<[u8; 32]>().prop_map(MemberId).boxed()
}

pub fn change_id() -> BoxedStrategy<ChangeId> {
    any::<[u8; 32]>().prop_map(ChangeId).boxed()
}

/// Unknown keys, as a newer client writes them: "must understand" keys
/// from `first` to 63 when `must` is set, and "may ignore" keys from 64 up.
pub fn extensions(first: u64, must: bool) -> BoxedStrategy<BTreeMap<u64, Value>> {
    let keys = if must {
        prop_oneof![first..64, 64u64..200, Just(u64::MAX)].boxed()
    } else {
        prop_oneof![64u64..200, Just(u64::MAX)].boxed()
    };
    proptest::collection::btree_map(keys, value(), 0..3).boxed()
}

/// A header, sometimes of a newer format or kind, or with unknown keys.
pub fn header() -> BoxedStrategy<Header> {
    (
        (space_id(), member_id(), any::<u64>(), any::<u64>()),
        (
            prop_oneof![
                Just(Kind::SEALED),
                Just(Kind::CONTROL),
                any::<u64>().prop_map(Kind)
            ],
            proptest::collection::vec(any::<u8>(), 0..64),
            prop_oneof![Just(Header::FORMAT), any::<u64>()],
            any::<u64>(),
        ),
        any::<bool>().prop_flat_map(|must| extensions(9, must)),
    )
        .prop_map(
            |((space, author, sequence, generation), (kind, payload, format, suite), ext)| {
                let mut header =
                    Header::new(space, author, sequence, generation, kind, payload, suite);
                header.format = format;
                for (key, value) in ext {
                    assert!(header.insert_extension(key, value));
                }
                header
            },
        )
        .boxed()
}

/// An operation, sometimes with unknown keys.
pub fn operation() -> BoxedStrategy<Operation> {
    (
        object_id(),
        text(8),
        proptest::option::of(text(8)),
        any::<u64>(),
        proptest::option::of(value()),
        any::<bool>().prop_flat_map(|must| extensions(6, must)),
    )
        .prop_map(|(object, collection, field, op, value, ext)| {
            let mut operation = Operation::new(object, collection, field, op, value);
            for (key, value) in ext {
                assert!(operation.insert_extension(key, value));
            }
            operation
        })
        .boxed()
}

/// A payload, sometimes of a newer format, padded, or with unknown keys.
pub fn payload() -> BoxedStrategy<Payload> {
    (
        any::<u64>().prop_map(Hlc),
        proptest::collection::btree_set(change_id(), 0..4),
        proptest::collection::vec(operation(), 0..4),
        prop_oneof![Just(Payload::FORMAT), any::<u64>()],
        proptest::option::of(proptest::collection::vec(any::<u8>(), 0..32)),
        any::<bool>().prop_flat_map(|must| extensions(6, must)),
    )
        .prop_map(|(clock, dependencies, operations, format, padding, ext)| {
            let mut payload = Payload::new(clock, dependencies, operations);
            payload.format = format;
            payload.padding = padding;
            for (key, value) in ext {
                assert!(payload.insert_extension(key, value));
            }
            payload
        })
        .boxed()
}
