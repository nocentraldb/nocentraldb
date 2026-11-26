//! A warm relay judges a new person-key entry from the space's memory
//! without reading the space's other control changes again (NOC-497): its
//! resolver adds the entry to the fold it holds.

#[path = "../../../ncdb-core/tests/support/space.rs"]
mod space;

use std::collections::BTreeSet;

use ncdb_core::change::Change;
use ncdb_core::membership::Role;

use crate::conformance::block_on;
use crate::sqlite::SqliteStore;
use crate::{Config, Pushed, Relay};

use space::{Space, Who, account, id};

const CREATOR: Who = 0;
const WRITER: Who = 1;

fn push(relay: &Relay<SqliteStore>, who: Who, change: &Change) {
    let space = change.header().space;
    let frames = vec![change.bytes().to_vec()];
    let pushed = block_on(relay.push(id(who), space, frames)).results;
    assert!(matches!(pushed[..], [Pushed::Accepted(_)]), "{pushed:?}");
}

#[test]
fn a_warm_push_of_a_person_key_entry_reads_only_that_entry_into_the_resolver() {
    let store = SqliteStore::in_memory().expect("the database opens");
    let config = Config::new(BTreeSet::from([account().verifying_key().to_bytes()]));
    let relay = Relay::new(store, config);
    let mut s = Space::new(CREATOR);
    let genesis = s.genesis.clone();
    let writer = s.add(CREATOR, WRITER, Role::Writer, &[&genesis]);
    push(&relay, CREATOR, &genesis);
    push(&relay, CREATOR, &writer);
    let mut head = writer;
    for person in 1..=20 {
        let change = s.person_key(WRITER, person, person, &[&head]);
        push(&relay, WRITER, &change);
        head = change;
    }
    let known = relay
        .memory()
        .take(s.space)
        .expect("the relay remembers it");
    assert_eq!(known.entries.len(), 22);
    assert_eq!(
        known.resolver.read(),
        1,
        "the last push read only its own change into the resolver"
    );
}
