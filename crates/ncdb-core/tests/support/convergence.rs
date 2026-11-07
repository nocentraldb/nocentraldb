//! The convergence harness: principle 2, "the same set of changes yields the
//! same state on every copy, in any order of arrival".

use core::fmt::Debug;

use proptest::strategy::BoxedStrategy;

/// A state that changes merge into. The harness drives it; every field type
/// the core ships must pass the harness.
pub trait Mergeable: Default + Clone + PartialEq + Debug {
    /// One change, as a member makes it and a copy receives it.
    type Change: Clone + Debug;

    /// Merges one received change into this state.
    fn apply(&mut self, change: &Self::Change);
}

/// Who made a change: one of the simulated members.
pub type Member = u8;

/// How the harness runs.
#[derive(Clone, Debug)]
pub struct Config {
    /// Number of simulated members that make changes.
    pub members: Member,
    /// Number of copies the changes are delivered to; at least three.
    pub copies: usize,
    /// Most changes one member makes in one case.
    pub max_changes_per_member: usize,
    /// Number of generated cases.
    pub cases: u32,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            members: 3,
            copies: 3,
            max_changes_per_member: 4,
            cases: 256,
        }
    }
}

/// Copies that ended in different states.
#[derive(Clone, Debug)]
pub struct Divergence {}

/// Delivers generated changes to several copies in different orders and
/// checks that every copy ends in the same state. `change` generates one
/// change that `member` makes.
pub fn check<S: Mergeable>(
    _config: &Config,
    _change: impl Fn(Member) -> BoxedStrategy<S::Change>,
) -> Result<(), Divergence> {
    todo!("convergence harness")
}
