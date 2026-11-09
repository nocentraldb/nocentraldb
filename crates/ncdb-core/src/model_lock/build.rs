//! From a schema's description and the previous lock to the next lock.

/// The description of a schema.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Draft;

/// The result of a build.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Built;

/// Builds the next lock.
pub fn build() {}
