//! The suite's rule for the one space of an account the relay does not
//! list (RFC 0007, "Who may create a space"; Carsten, 2026-11-12,
//! NOC-286): an append that records it holds once per account, and an
//! append that would record a second one applies nothing.

use super::{Backend, block_on, entry, space};
use crate::store::{AppendOutcome, Effect, RelayStore};

pub fn an_account_records_one_guest_space<B: Backend>(backend: &B) {
    let store = backend.connect();
    let (guest, other) = ([0x47; 32], [0x48; 32]);
    let first =
        block_on(store.append(space(1), 0, vec![entry(1)], vec![Effect::GuestSpace(guest)]));
    assert_eq!(first.unwrap(), AppendOutcome::Appended { head: 1 });

    let second =
        block_on(store.append(space(2), 0, vec![entry(2)], vec![Effect::GuestSpace(guest)]));
    assert_eq!(second.unwrap(), AppendOutcome::GuestSpaceTaken);
    // Nothing of the refused append is applied, through any handle.
    let fresh = backend.connect();
    assert_eq!(block_on(fresh.head(space(2))).unwrap(), None);
    assert_eq!(
        block_on(fresh.find(space(2), entry(2).change)).unwrap(),
        None
    );

    // Another account has its own one space.
    let third =
        block_on(store.append(space(3), 0, vec![entry(3)], vec![Effect::GuestSpace(other)]));
    assert_eq!(third.unwrap(), AppendOutcome::Appended { head: 1 });
}
