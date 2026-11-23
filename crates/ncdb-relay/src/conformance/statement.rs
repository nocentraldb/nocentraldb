//! The suite's rule for the highest account key statement number per
//! identity (RFC 0015, "The account's identity"; NOC-420): an append
//! raises it, never lowers it, and holds whatever number it carries.

use super::{Backend, block_on, entry, space};
use crate::store::{AppendOutcome, Effect, RelayStore};

pub fn the_highest_statement_number_is_kept_per_identity<B: Backend>(backend: &B) {
    let store = backend.connect();
    let (identity, other) = ([0x53; 32], [0x54; 32]);
    assert_eq!(block_on(store.statement(identity)).unwrap(), None);

    let raise = |at: u8, identity: [u8; 32], number: u64| {
        let effect = Effect::Statement { identity, number };
        block_on(store.append(space(at), 0, vec![entry(u32::from(at))], vec![effect])).unwrap()
    };
    assert_eq!(raise(1, identity, 2), AppendOutcome::Appended { head: 1 });
    // A lower number holds the append and leaves the higher one stored.
    assert_eq!(raise(2, identity, 1), AppendOutcome::Appended { head: 1 });
    let fresh = backend.connect();
    assert_eq!(block_on(fresh.statement(identity)).unwrap(), Some(2));

    assert_eq!(
        raise(3, identity, 1_800_000_000),
        AppendOutcome::Appended { head: 1 }
    );
    assert_eq!(
        block_on(fresh.statement(identity)).unwrap(),
        Some(1_800_000_000)
    );
    // Another identity has its own number.
    assert_eq!(block_on(fresh.statement(other)).unwrap(), None);
    raise(4, other, 5);
    assert_eq!(block_on(fresh.statement(other)).unwrap(), Some(5));
    assert_eq!(
        block_on(fresh.statement(identity)).unwrap(),
        Some(1_800_000_000)
    );
}
