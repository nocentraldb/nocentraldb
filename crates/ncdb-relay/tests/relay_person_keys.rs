//! RFC 0007, "Keys and the person-key store" and "What the relay checks":
//! a shred entry is appended once its second account's sign-off holds, the
//! relay stores person-key records it matched to their person-key entries
//! and refuses every other record with `bad_record`, and a member reads its
//! key envelopes, the key chain and the records.

mod support;

use ncdb_core::change::Change;
use ncdb_core::ids::SpaceId;
use ncdb_core::membership::{self, Role};
use ncdb_relay::conformance::block_on;
use ncdb_relay::sqlite::SqliteStore;
use ncdb_relay::store::{PersonKeyId, PersonKeyRecord};
use ncdb_relay::{ErrorCode, Pushed, Relay};
use support::interleave::Interleave;
use support::space::{Space, account, id, key, other_account, person, wrapped};
use support::{config, push, push_one, relay};

const CREATOR: u8 = 0;
const WRITER: u8 = 1;
const READER: u8 = 2;
/// An admin of a second account, who signs off shreds.
const SECOND: u8 = 3;
const STRANGER: u8 = 9;

/// A space on the relay where the creator added a writer, a reader and an
/// admin of a second account, and the writer created person 1's key.
fn held() -> (Relay<SqliteStore>, support::TempDir, Space, Vec<Change>) {
    let (relay, dir) = relay();
    let mut space = Space::new(CREATOR);
    let genesis = space.genesis.clone();
    let writer = space.add(CREATOR, WRITER, Role::Writer, &[&genesis]);
    let reader = space.add(CREATOR, READER, Role::Reader, &[&writer]);
    space.certify_by(SECOND, other_account(1));
    let second = space.add(CREATOR, SECOND, Role::Admin, &[&reader]);
    let entry = space.person_key(WRITER, 1, 0xc1, &[&second]);
    let held = vec![genesis, writer, reader, second, entry];
    let refs: Vec<&Change> = held.iter().collect();
    let pushed = push(&relay, CREATOR, space.space, &refs);
    assert!(pushed.iter().all(|p| matches!(p, Pushed::Accepted(_))));
    (relay, dir, space, held)
}

/// The record of simulated person `who` under `generation`, written and
/// signed by `writer`.
fn record(space: SpaceId, writer: u8, who: u8, generation: u64) -> PersonKeyRecord {
    let wrapped = wrapped(who, generation);
    let signature = membership::sign_record(&key(writer), space, person(who), generation, &wrapped);
    PersonKeyRecord {
        id: PersonKeyId(person(who)),
        generation,
        writer: id(writer),
        wrapped,
        signature: signature.to_vec(),
    }
}

fn put(
    relay: &Relay<SqliteStore>,
    uploader: u8,
    space: SpaceId,
    records: Vec<PersonKeyRecord>,
) -> Result<(), ErrorCode> {
    block_on(relay.put_person_keys(id(uploader), space, records))
}

fn stored(relay: &Relay<SqliteStore>, reader: u8, space: SpaceId) -> Vec<PersonKeyRecord> {
    block_on(relay.person_keys(id(reader), space)).expect("the reader reads the records")
}

#[test]
fn a_shred_signed_off_by_a_second_admin_account_is_appended() {
    let (relay, _dir, mut space, held) = held();
    let shred = space.shred(WRITER, 1, &other_account(1), &[&held[4]]);
    assert_eq!(push_one(&relay, WRITER, &shred), Pushed::Accepted(6));
}

#[test]
fn a_shred_without_a_valid_sign_off_breaks_the_membership_rules() {
    let (relay, _dir, mut space, held) = held();
    let own_account = space.shred(WRITER, 1, &account(), &[&held[4]]);
    let no_admin = space.shred(WRITER, 1, &other_account(2), &[&held[4]]);
    let unknown_key = space.shred(WRITER, 2, &other_account(1), &[&held[4]]);
    for shred in [&own_account, &no_admin, &unknown_key] {
        assert_eq!(
            push_one(&relay, WRITER, shred),
            Pushed::Rejected(ErrorCode::MembershipRule)
        );
    }
}

#[test]
fn a_record_its_person_key_entry_announces_is_stored_and_read_back() {
    let (relay, _dir, space, _) = held();
    let record = record(space.space, WRITER, 1, space.first_generation);
    assert_eq!(
        put(&relay, WRITER, space.space, vec![record.clone()]),
        Ok(())
    );
    // Every member reads the records, a reader included.
    assert_eq!(stored(&relay, READER, space.space), [record]);
}

#[test]
fn a_badly_signed_record_is_a_bad_record() {
    let (relay, _dir, space, _) = held();
    let mut forged = record(space.space, WRITER, 1, space.first_generation);
    forged.signature[0] ^= 1;
    let mut short = record(space.space, WRITER, 1, space.first_generation);
    short.signature.pop();
    // Signed by the reader, but naming the writer.
    let mut other = record(space.space, READER, 1, space.first_generation);
    other.writer = id(WRITER);
    // Signed by a member the space does not know.
    let stranger = record(space.space, STRANGER, 1, space.first_generation);
    for bad in [forged, short, other, stranger] {
        assert_eq!(
            put(&relay, WRITER, space.space, vec![bad]),
            Err(ErrorCode::BadRecord)
        );
    }
    assert!(stored(&relay, WRITER, space.space).is_empty());
}

#[test]
fn a_record_no_person_key_entry_announces_is_a_bad_record() {
    let (relay, _dir, space, _) = held();
    // Person 2 has no entry; person 1's entry names another generation and
    // another hash.
    let unknown = record(space.space, WRITER, 2, space.first_generation);
    let other_generation = record(space.space, WRITER, 1, space.first_generation + 1);
    let mut other_bytes = record(space.space, WRITER, 1, space.first_generation);
    other_bytes.wrapped.push(0);
    other_bytes.signature = membership::sign_record(
        &key(WRITER),
        space.space,
        person(1),
        space.first_generation,
        &other_bytes.wrapped,
    )
    .to_vec();
    for bad in [unknown, other_generation, other_bytes] {
        assert_eq!(
            put(&relay, WRITER, space.space, vec![bad]),
            Err(ErrorCode::BadRecord)
        );
    }
}

#[test]
fn one_bad_record_keeps_every_record_of_the_request_out() {
    let (relay, _dir, space, _) = held();
    let good = record(space.space, WRITER, 1, space.first_generation);
    let bad = record(space.space, WRITER, 2, space.first_generation);
    assert_eq!(
        put(&relay, WRITER, space.space, vec![good, bad]),
        Err(ErrorCode::BadRecord)
    );
    assert!(stored(&relay, WRITER, space.space).is_empty());
}

#[test]
fn a_record_of_a_shredded_person_key_is_a_bad_record() {
    let (relay, _dir, mut space, held) = held();
    let shred = space.shred(WRITER, 1, &other_account(1), &[&held[4]]);
    assert_eq!(push_one(&relay, WRITER, &shred), Pushed::Accepted(6));
    let record = record(space.space, WRITER, 1, space.first_generation);
    assert_eq!(
        put(&relay, WRITER, space.space, vec![record]),
        Err(ErrorCode::BadRecord)
    );
}

#[test]
fn a_re_wrap_replaces_the_record_only_under_a_descendant_generation() {
    let (relay, _dir, mut space, held) = held();
    let first = space.first_generation;
    let (chain, next) = space.rotate(CREATOR, &[first], &[&held[4]]);
    let rewrap = space.person_key_at(CREATOR, 1, 0xc1, next, &[&chain]);
    let pushed = push(&relay, CREATOR, space.space, &[&chain, &rewrap]);
    assert_eq!(pushed, [Pushed::Accepted(6), Pushed::Accepted(7)]);

    let older = record(space.space, WRITER, 1, first);
    let newer = record(space.space, CREATOR, 1, next);
    assert_eq!(
        put(&relay, WRITER, space.space, vec![older.clone()]),
        Ok(())
    );
    assert_eq!(
        put(&relay, CREATOR, space.space, vec![newer.clone()]),
        Ok(())
    );
    assert_eq!(
        stored(&relay, WRITER, space.space),
        std::slice::from_ref(&newer)
    );
    // The first generation does not descend from the next.
    assert_eq!(
        put(&relay, WRITER, space.space, vec![older]),
        Err(ErrorCode::BadRecord)
    );
    // The same generation again replaces it.
    assert_eq!(
        put(&relay, CREATOR, space.space, vec![newer.clone()]),
        Ok(())
    );
    assert_eq!(stored(&relay, WRITER, space.space), [newer]);
}

#[test]
fn a_put_overtaken_by_a_newer_generation_never_takes_the_record_back() {
    let (relay, dir, mut space, held) = held();
    let first = space.first_generation;
    let (chain, next) = space.rotate(CREATOR, &[first], &[&held[4]]);
    let rewrap = space.person_key_at(CREATOR, 1, 0xc1, next, &[&chain]);
    let pushed = push(&relay, CREATOR, space.space, &[&chain, &rewrap]);
    assert_eq!(pushed, [Pushed::Accepted(6), Pushed::Accepted(7)]);
    let older = record(space.space, WRITER, 1, first);
    let newer = record(space.space, CREATOR, 1, next);
    assert_eq!(
        put(&relay, WRITER, space.space, vec![older.clone()]),
        Ok(())
    );

    // A second relay process on the same database stores the newer record
    // after this relay checked the older one against the stored record and
    // before it writes it. The older generation then no longer descends
    // from the stored one.
    let (rival, in_space) = (newer.clone(), space.space);
    let racing = Relay::new(
        Interleave::new(dir.store(), move || {
            let put = relay.put_person_keys(id(CREATOR), in_space, vec![rival]);
            assert_eq!(block_on(put), Ok(()));
        }),
        config(),
    );
    assert_eq!(
        block_on(racing.put_person_keys(id(WRITER), space.space, vec![older])),
        Err(ErrorCode::BadRecord)
    );
    let stored = block_on(racing.person_keys(id(WRITER), space.space));
    assert_eq!(stored, Ok(vec![newer]));
}

#[test]
fn only_current_members_put_and_read_records_of_a_space_the_relay_holds() {
    let (relay, _dir, space, _) = held();
    let record = record(space.space, WRITER, 1, space.first_generation);
    assert_eq!(
        put(&relay, STRANGER, space.space, vec![record.clone()]),
        Err(ErrorCode::NotMember)
    );
    assert_eq!(
        block_on(relay.person_keys(id(STRANGER), space.space)),
        Err(ErrorCode::NotMember)
    );
    let unknown = SpaceId([0xee; 16]);
    assert_eq!(
        put(&relay, WRITER, unknown, vec![record]),
        Err(ErrorCode::UnknownSpace)
    );
    assert_eq!(
        block_on(relay.person_keys(id(WRITER), unknown)),
        Err(ErrorCode::UnknownSpace)
    );
}

#[test]
fn a_member_reads_its_own_key_envelopes_and_every_key_chain_entry() {
    let (relay, _dir, mut space, held) = held();
    let first = space.first_generation;
    let to_reader = space.envelope(CREATOR, READER, &[&held[4]]);
    let to_writer = space.envelope(CREATOR, WRITER, &[&to_reader]);
    let (chain, next) = space.rotate(CREATOR, &[first], &[&to_writer]);
    let rotated = space.envelope_at(CREATOR, READER, next, &[&chain]);
    let changes = [&to_reader, &to_writer, &chain, &rotated];
    let pushed = push(&relay, CREATOR, space.space, &changes);
    assert!(pushed.iter().all(|p| matches!(p, Pushed::Accepted(_))));

    let keys = block_on(relay.keys(id(READER), space.space)).expect("the reader's keys");
    let frames = |entries: &[ncdb_relay::store::Entry]| -> Vec<Vec<u8>> {
        entries.iter().map(|e| e.frame.clone()).collect()
    };
    assert_eq!(
        frames(&keys.envelopes),
        [to_reader.bytes().to_vec(), rotated.bytes().to_vec()]
    );
    assert_eq!(frames(&keys.chain), [chain.bytes().to_vec()]);
    assert_eq!(
        keys.envelopes
            .iter()
            .map(|e| e.position)
            .collect::<Vec<_>>(),
        [6, 9]
    );

    assert_eq!(
        block_on(relay.keys(id(STRANGER), space.space)).map(|_| ()),
        Err(ErrorCode::NotMember)
    );
    assert_eq!(
        block_on(relay.keys(id(READER), SpaceId([0xee; 16]))).map(|_| ()),
        Err(ErrorCode::UnknownSpace)
    );
}

#[test]
fn no_record_is_accepted_for_a_key_whose_shred_outlived_its_entries() {
    let (relay, _dir, mut space, held) = held();
    // The writer's entry (held[4]) and its shred; the creator, senior,
    // writes a concurrent first entry with another commitment, which takes
    // the person key in the order, and which the shred then rejects. The
    // rival reaches the relay first: after the shred, the relay would
    // refuse it.
    let shred = space.shred(WRITER, 1, &other_account(1), &[&held[4]]);
    let rival = space.person_key(CREATOR, 1, 0xc2, &[&held[3]]);
    let pushed = push(&relay, CREATOR, space.space, &[&rival, &shred]);
    assert_eq!(pushed, [Pushed::Accepted(6), Pushed::Accepted(7)]);
    for (writer, record) in [
        (
            WRITER,
            record(space.space, WRITER, 1, space.first_generation),
        ),
        (
            CREATOR,
            record(space.space, CREATOR, 1, space.first_generation),
        ),
    ] {
        assert_eq!(
            put(&relay, writer, space.space, vec![record]),
            Err(ErrorCode::BadRecord)
        );
    }
}
