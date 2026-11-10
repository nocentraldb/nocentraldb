//! A member's row of the membership table holds what `resolve` says about
//! the member, and its sealed-change verdict is the core's.

mod support;

use ncdb_core::change::Header;
use ncdb_core::membership::{self, Role};
use ncdb_relay::record;
use support::space::{Space, id};

/// A space where 0 created it, added 1 as a writer and 2 as a reader, made
/// 2 a writer, and removed 1 with cutoff 3.
fn space() -> (Space, Vec<ncdb_core::change::Change>) {
    let mut space = Space::new(0);
    let genesis = space.genesis.clone();
    let add_1 = space.add(0, 1, Role::Writer, &[&genesis]);
    let add_2 = space.add(0, 2, Role::Reader, &[&add_1]);
    let raise = space.set_role(0, 2, Role::Writer, 0, &[&add_2]);
    let remove = space.remove(0, 1, 3, &[&raise]);
    let changes = vec![genesis, add_1, add_2, raise, remove];
    (space, changes)
}

fn header(space: &Space, author: u8, sequence: u64) -> Header {
    let mut header = space.genesis.header().clone();
    header.author = id(author);
    header.sequence = sequence;
    header.key_generation = space.first_generation;
    header.kind = ncdb_core::change::Kind::SEALED;
    header
}

#[test]
fn every_member_has_a_row_that_decodes_to_its_state() {
    let (space, changes) = space();
    let resolved = membership::resolve(space.space, &changes);
    let records = record::records(&resolved);
    assert_eq!(records.len(), 3);
    for who in 0..3 {
        let row = records.iter().find(|r| r.member == id(who)).expect("a row");
        assert_eq!(row.current, resolved.role(id(who)).is_some());
        let decoded = record::decode(row).expect("the row decodes");
        assert_eq!(Some(&decoded.state), resolved.member(id(who)));
    }
    let removed = records.iter().find(|r| r.member == id(1)).unwrap();
    assert!(!removed.current);
}

#[test]
fn a_row_admits_what_resolve_admits() {
    let (space, changes) = space();
    let resolved = membership::resolve(space.space, &changes);
    let records = record::records(&resolved);
    for who in 0..3 {
        let row = records.iter().find(|r| r.member == id(who)).unwrap();
        let row = record::decode(row).unwrap();
        for sequence in 1..6 {
            let header = header(&space, who, sequence);
            assert_eq!(
                row.admits(&header),
                resolved.admits(&header),
                "member {who}, sequence {sequence}"
            );
        }
    }
    let one = record::decode(records.iter().find(|r| r.member == id(1)).unwrap()).unwrap();
    assert!(one.admits(&header(&space, 1, 3)));
    assert!(!one.admits(&header(&space, 1, 4)));
}

#[test]
fn a_row_that_is_not_the_cores_encoding_does_not_decode() {
    let (space, changes) = space();
    let resolved = membership::resolve(space.space, &changes);
    let mut row = record::records(&resolved).remove(0);
    row.state.truncate(row.state.len() - 1);
    assert!(record::decode(&row).is_err());
    row.state = vec![0xa0];
    assert!(record::decode(&row).is_err());
}
