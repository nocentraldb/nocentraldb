//! The person-key store at the relay (RFC 0003, "The person key"; RFC 0007,
//! "Keys and the person-key store"; NOC-206 part 2): a member puts the
//! signed, wrapped record of a person key after its person-key entry, and
//! another member fetches the space's records and holds each person key
//! whose signature verifies, that unwraps under its generation, and that
//! matches the commitment in the log. A record that fails is not held.
//! A rotation re-wraps every person key its admin holds under the fresh
//! generation and puts the new record, which replaces the old one at the
//! relay (RFC 0003, "Removing a member and rotating the space key", step
//! 3; NOC-206 part 3).

use ncdb_core::ids::SpaceId as CoreSpaceId;
use ncdb_core::keys::PersonKey;
use ncdb_core::membership::{
    self, ControlBody, Entry, PersonKeyEntry, record_hash, sign_record, wrap_person_key,
};

use super::support::*;
use crate::db::{Db, lock};
use crate::members::{AddMember, Fingerprint, MemberId, Role};
use crate::space::Space;

/// The admin, who creates the space and its person key.
const ADMIN: Who = 1;
/// The member who fetches the records.
const MEMBER: Who = 2;
/// A member the admin removes, which rotates the space key.
const GUEST: Who = 3;

const PERSON: [u8; 16] = [0x9e; 16];

/// The admin announces the person key `committed` of `PERSON` in `space`
/// with a person-key entry whose record wraps `wrapped`, folds the entry,
/// and hands the record to syncing to put.
fn person_key(admin: &Db, space: &Space, committed: &PersonKey, wrapped: &PersonKey) {
    let id = space.id().0;
    let mut inner = lock(&admin.inner);
    let member = inner.member.key();
    let crate::db::Inner { view, spaces, .. } = &mut *inner;
    let state = spaces.get_mut(&id).expect("the admin holds the space");
    let next = crate::next::next(view, id, &state.checks, member.id());
    let generation = next.generation.expect("a generation");
    let key = state.checks.space_key(generation).expect("its key");
    let bytes = wrap_person_key(key, id, generation, PERSON, wrapped);
    let signature = sign_record(&member, id, PERSON, generation, &bytes);
    let entry = Entry::PersonKey(PersonKeyEntry {
        person_key: PERSON,
        commitment: committed.commitment(id, PERSON),
        generation,
        record_hash: record_hash(&bytes),
    });
    let change = membership::write(
        &member,
        id,
        next.sequence,
        generation,
        &ControlBody::new(crate::members::control_heads(view, id), entry),
    );
    crate::space::fold(view, &mut state.checks, &change).unwrap();
    let record = crate::sync::Record {
        person_key: PERSON,
        generation,
        writer: member.id().0,
        wrapped: bytes.to_vec(),
        signature,
    };
    crate::sync::put_person_keys(&mut inner, id, vec![record]);
    crate::sync::wake(&inner);
}

/// Both databases synced, and the space the admin shares with the member
/// as a writer.
async fn shared(relay: &LocalRelay) -> (Db, Db, Space) {
    let admin = open(ADMIN, &relay.url()).await;
    let member = open(MEMBER, &relay.url()).await;
    let project = now(admin.spaces().create()).unwrap();
    let options = AddMember {
        role: Role::Writer,
        fingerprint: None,
    };
    now(project.members().add(&now(member.invitation()), options)).unwrap();
    synced(&admin).await;
    (admin, member, project)
}

/// The commitment of the person key `PERSON` that `db` holds in `space`.
fn held(db: &Db, space: CoreSpaceId) -> Option<[u8; 32]> {
    let inner = lock(&db.inner);
    let state = inner.spaces.get(&space)?;
    let key = state.person_keys.get(&PERSON)?;
    Some(key.commitment(space, PERSON))
}

/// Whether `db` fetched the records of `space` in its current connection.
fn fetched(db: &Db, space: CoreSpaceId) -> bool {
    let inner = lock(&db.inner);
    inner
        .spaces
        .get(&space)
        .is_some_and(|state| state.person_keys_fetched)
}

#[tokio::test(flavor = "multi_thread")]
async fn a_member_fetches_and_holds_the_person_key_another_member_put() {
    let relay = LocalRelay::start().await;
    let (admin, member, project) = shared(&relay).await;
    let key = PersonKey::generate();

    person_key(&admin, &project, &key, &key);
    let status = synced(&admin).await;
    assert_eq!(status.refused, 0);

    let space = project.id().0;
    let commitment = key.commitment(space, PERSON);
    eventually("the member holds the person key", || {
        // A reconnect fetches the records again; the member may have
        // fetched before the admin's put reached the relay.
        if fetched(&member, space) && held(&member, space).is_none() {
            relay.proxy.cut();
            relay.proxy.restore();
        }
        held(&member, space) == Some(commitment)
    })
    .await;
    assert_eq!(held(&admin, space), Some(commitment), "its writer holds it");
    let status = handle(&member, space).status().current();
    assert_eq!(status.bad_person_key_records, Vec::<Fingerprint>::new());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_record_whose_key_does_not_match_its_commitment_is_not_held() {
    let relay = LocalRelay::start().await;
    let (admin, member, project) = shared(&relay).await;

    // The relay cannot tell: the record hash matches the wrapped bytes.
    person_key(
        &admin,
        &project,
        &PersonKey::generate(),
        &PersonKey::generate(),
    );
    synced(&admin).await;
    relay.proxy.cut();
    relay.proxy.restore();
    let space = project.id().0;
    eventually("the member fetched the records", || fetched(&member, space)).await;
    synced(&member).await;

    assert_eq!(held(&member, space), None);
    let writer = Fingerprint(lock(&admin.inner).account.verifying_key().to_bytes());
    let status = handle(&member, space).status().current();
    assert_eq!(status.bad_person_key_records, vec![writer]);
}

/// The space's current generation on `db`.
fn current(db: &Db, space: CoreSpaceId) -> Option<u64> {
    let inner = lock(&db.inner);
    inner
        .spaces
        .get(&space)?
        .checks
        .resolved()
        .current_generation()
}

/// A removal rotates the space key; the rotation re-wraps the person key
/// under the fresh generation and puts the record, which replaces the old
/// one at the relay. The other member, which forgot the key, holds it
/// again from that record alone once it reconnects.
#[tokio::test(flavor = "multi_thread")]
async fn a_rotation_re_wraps_the_person_key_and_the_relay_keeps_the_new_record() {
    let relay = LocalRelay::start().await;
    let (admin, member, project) = shared(&relay).await;
    let guest = open(GUEST, &relay.url()).await;
    let invitation = now(guest.invitation());
    let options = AddMember {
        role: Role::Writer,
        fingerprint: None,
    };
    now(project.members().add(&invitation, options)).unwrap();
    let key = PersonKey::generate();
    person_key(&admin, &project, &key, &key);
    synced(&admin).await;
    let space = project.id().0;
    let before = current(&admin, space).expect("a generation");

    now(project.members().remove(MemberId(invitation.key.id()))).unwrap();
    let status = synced(&admin).await;

    assert_eq!(status.refused, 0);
    let fresh = current(&admin, space).expect("a generation");
    assert_ne!(fresh, before);
    // The put follows the push of the rotation, so `synced` may return
    // before the relay stored the new record.
    let start = std::time::Instant::now();
    let stored = loop {
        let stored = relay.person_keys(space).await;
        if stored.iter().any(|record| record.generation == fresh) {
            break stored;
        }
        assert!(
            start.elapsed() < std::time::Duration::from_secs(10),
            "the relay stores the re-wrapped record: not in time; it holds generations {:?}",
            stored
                .iter()
                .map(|record| record.generation)
                .collect::<Vec<_>>()
        );
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    };
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].writer.0, lock(&admin.inner).member.key().id().0);

    eventually("the member reaches the fresh generation", || {
        current(&member, space) == Some(fresh)
    })
    .await;
    lock(&member.inner)
        .spaces
        .get_mut(&space)
        .expect("the member holds the space")
        .person_keys
        .clear();
    relay.proxy.cut();
    relay.proxy.restore();
    let commitment = key.commitment(space, PERSON);
    eventually("the member holds the person key again", || {
        held(&member, space) == Some(commitment)
    })
    .await;
}
