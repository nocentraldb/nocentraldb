//! Helpers of the relay's tests. The simulated space, whose members write
//! real, signed changes, is the core's own test helper, so the relay is
//! tested with the changes the core's tests use.
#![allow(dead_code)]

#[path = "../../../ncdb-core/tests/support/space.rs"]
pub mod space;
