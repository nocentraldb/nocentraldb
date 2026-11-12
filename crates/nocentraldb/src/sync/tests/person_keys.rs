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

use ncdb_core::cbor::Value;
use ncdb_core::ids::{ChangeId as CoreChangeId, SpaceId as CoreSpaceId};
use ncdb_core::keys::PersonKey;
use ncdb_core::membership::{
    self, ControlBody, Entry, PersonKeyEntry, record_hash, sign_record, wrap_person_key,
};

use super::support::*;
use crate::db::{Db, OpenOptions, lock};
use crate::members::{AddMember, Fingerprint, MemberId, Role};
use crate::space::Space;

/// The admin, who creates the space and its person key.
const ADMIN: Who = 1;
/// The member who fetches the records.
const MEMBER: Who = 2;
/// A member the admin removes, which rotates the space key.
const GUEST: Who = 3;

pub(super) const PERSON: [u8; 16] = [0x9e; 16];

/// The admin announces the person key `committed` of `PERSON` in `space`
/// with a person-key entry whose record wraps `wrapped`, folds the entry,
/// and hands the record to syncing to put.
pub(super) fn person_key(admin: &Db, space: &Space, committed: &PersonKey, wrapped: &PersonKey) {
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
pub(super) async fn shared(relay: &LocalRelay) -> (Db, Db, Space) {
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
pub(super) fn held(db: &Db, space: CoreSpaceId) -> Option<[u8; 32]> {
    let inner = lock(&db.inner);
    let state = inner.spaces.get(&space)?;
    let key = state.person_keys.get(&PERSON)?;
    Some(key.commitment(space, PERSON))
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
    // The member may have fetched before the admin's put reached the
    // relay: once the relay stores the record, one reconnect fetches it.
    // Reconnecting in a loop while the old connection's fetch still counts
    // kept cutting the new one before it could fetch.
    stored(&relay, space).await;
    relay.proxy.cut();
    relay.proxy.restore();
    eventually("the member holds the person key", || {
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
    let space = project.id().0;
    stored(&relay, space).await;
    relay.proxy.cut();
    relay.proxy.restore();
    let writer = Fingerprint(lock(&admin.inner).account.verifying_key().to_bytes());
    eventually("the member names the record's writer", || {
        let status = handle(&member, space).status().current();
        status.bad_person_key_records == vec![writer]
    })
    .await;

    assert_eq!(held(&member, space), None);
}

/// Waits until the relay stores a person-key record of `space`: the put
/// follows the push, so `synced` may return before it.
pub(super) async fn stored(relay: &LocalRelay, space: CoreSpaceId) {
    let start = std::time::Instant::now();
    while relay.person_keys(space).await.is_empty() {
        assert!(
            start.elapsed() < std::time::Duration::from_secs(10),
            "the relay stores the record: not in time"
        );
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
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

/// A member of a second account, who signs off the admin's shred.
const SIGNER: Who = 4;

/// The admin writes `entry` into `space` after the control heads, folds it
/// and wakes syncing. Returns its id.
fn write(admin: &Db, space: &Space, entry: Entry) -> CoreChangeId {
    let id = space.id().0;
    let mut inner = lock(&admin.inner);
    let member = inner.member.key();
    let crate::db::Inner { view, spaces, .. } = &mut *inner;
    let state = spaces.get_mut(&id).expect("the admin holds the space");
    let next = crate::next::next(view, id, &state.checks, member.id());
    let generation = next.generation.expect("a generation");
    let change = membership::write(
        &member,
        id,
        next.sequence,
        generation,
        &ControlBody::new(crate::members::control_heads(view, id), entry),
    );
    crate::space::fold(view, &mut state.checks, &change).unwrap();
    crate::sync::wake(&inner);
    change.id()
}

/// The admin shreds `PERSON` in `space`, signed off by `account`. Returns
/// the shred's id.
fn shred_signed(admin: &Db, space: &Space, account: &ed25519_dalek::SigningKey) -> CoreChangeId {
    let performer = lock(&admin.inner).member.key().id();
    let entry = Entry::Shred(membership::ShredEntry {
        person_key: PERSON,
        sign_off: membership::SignOff::sign(account, space.id().0, PERSON, performer),
    });
    write(admin, space, entry)
}

/// The admin shreds `PERSON` in `space`, signed off by the account of
/// `SIGNER`, folds the shred and wakes syncing. Returns the shred's id.
fn shred(admin: &Db, space: &Space) -> CoreChangeId {
    shred_signed(admin, space, &space::other_account(1))
}

/// A shred is final when accepted (RFC 0008, "A shred is final when
/// accepted"; NOC-229): a member that holds the person key drops it as
/// soon as it folds the shred, and the relay holds no record of it.
/// The admin and the member share a space with a signer of a second
/// account; the member holds the person key `PERSON`, which the admin then
/// shreds. Returns the shred's id.
async fn shredded(relay: &LocalRelay) -> (Db, Db, Space, CoreChangeId) {
    let (admin, member, project) = shared(relay).await;
    let options = OpenOptions::new(LOCK).in_memory().relay(&relay.url());
    let signer = Db::open_as(
        options,
        [SIGNER; 32],
        [SIGNER ^ 0x80; 32],
        space::other_account(1),
    )
    .await
    .expect("the test lock opens");
    let invitation = now(signer.invitation());
    let options = AddMember {
        role: Role::Writer,
        fingerprint: Some(invitation.fingerprint()),
    };
    now(project.members().add(&invitation, options)).unwrap();
    let key = PersonKey::generate();
    person_key(&admin, &project, &key, &key);
    synced(&admin).await;
    let space = project.id().0;
    stored(relay, space).await;
    relay.proxy.cut();
    relay.proxy.restore();
    eventually("the member holds the person key", || {
        held(&member, space).is_some()
    })
    .await;
    let shred = shred(&admin, &project);
    synced(&admin).await;
    (admin, member, project, shred)
}

#[tokio::test(flavor = "multi_thread")]
async fn a_member_drops_the_person_key_as_soon_as_it_folds_its_shred() {
    let relay = LocalRelay::start().await;
    let (_admin, member, project, _) = shredded(&relay).await;
    let space = project.id().0;

    eventually("the member folds the shred", || {
        let inner = lock(&member.inner);
        inner.spaces[&space]
            .checks
            .resolved()
            .person_key(PERSON)
            .is_some_and(|key| key.shred.is_some())
    })
    .await;
    eventually("the member drops the person key", || {
        held(&member, space).is_none()
    })
    .await;
    assert!(
        relay.person_keys(space).await.is_empty(),
        "the relay deleted the record"
    );
}

/// The acknowledgements `db`'s own member wrote in `space`, in its order:
/// the shred each names, and whether it says "deleted".
pub(super) fn acknowledgements(db: &Db, space: CoreSpaceId) -> Vec<(CoreChangeId, bool)> {
    let inner = lock(&db.inner);
    let me = inner.member.key().id();
    let log = inner.view.log(space).expect("the space's log");
    let mut own: Vec<_> = log
        .after(ncdb_core::log::Position::START)
        .map(|(_, change)| change)
        .filter(|change| change.header().author == me)
        .filter_map(|change| {
            let body = ControlBody::decode(&change.header().payload).ok()?;
            let Entry::Other { entry_type, fields } = body.entry else {
                return None;
            };
            if entry_type != membership::ACKNOWLEDGEMENT {
                return None;
            }
            let shred = match fields.get(&8)? {
                Value::Bytes(bytes) => CoreChangeId(bytes.as_slice().try_into().ok()?),
                _ => return None,
            };
            let deleted = matches!(fields.get(&9)?, Value::Bool(true));
            Some((change.header().sequence, shred, deleted))
        })
        .collect();
    own.sort();
    own.into_iter()
        .map(|(_, shred, deleted)| (shred, deleted))
        .collect()
}

/// RFC 0008, "Acknowledgement" (NOC-238): a member that folds a shred
/// writes "seen" and then "deleted" for it, once, and pushes both. The
/// performer acknowledges its own shred as well once it folds what others
/// sent, and a reconnect, which fetches and folds again, writes nothing
/// new.
#[tokio::test(flavor = "multi_thread")]
async fn a_member_that_folds_a_shred_acknowledges_it_seen_then_deleted_once() {
    let relay = LocalRelay::start().await;
    let (admin, member, project, shred) = shredded(&relay).await;
    let space = project.id().0;
    let both = vec![(shred, false), (shred, true)];

    eventually("the member acknowledges the shred", || {
        acknowledgements(&member, space) == both
    })
    .await;
    synced(&member).await;
    eventually(
        "the admin holds the member's acknowledgements and writes its own",
        || acknowledgements(&admin, space) == both,
    )
    .await;
    relay.proxy.cut();
    relay.proxy.restore();
    synced(&member).await;

    assert_eq!(
        acknowledgements(&member, space),
        both,
        "a second fold writes nothing new"
    );
}

/// Every role acknowledges a shred (RFC 0008, "Roles and rights"): a
/// reader writes "seen" and "deleted" too.
#[tokio::test(flavor = "multi_thread")]
async fn a_reader_acknowledges_a_shred_as_well() {
    let relay = LocalRelay::start().await;
    let (admin, member, project) = shared(&relay).await;
    let reader = open(GUEST, &relay.url()).await;
    let options = AddMember {
        role: Role::Reader,
        fingerprint: None,
    };
    now(project.members().add(&now(reader.invitation()), options)).unwrap();
    synced(&admin).await;
    drop(member);
    let signer_options = OpenOptions::new(LOCK).in_memory().relay(&relay.url());
    let signer = Db::open_as(
        signer_options,
        [SIGNER; 32],
        [SIGNER ^ 0x80; 32],
        space::other_account(1),
    )
    .await
    .expect("the test lock opens");
    let invitation = now(signer.invitation());
    let options = AddMember {
        role: Role::Writer,
        fingerprint: Some(invitation.fingerprint()),
    };
    now(project.members().add(&invitation, options)).unwrap();
    let key = PersonKey::generate();
    person_key(&admin, &project, &key, &key);
    let shred = shred(&admin, &project);
    synced(&admin).await;
    let space = project.id().0;

    eventually("the reader acknowledges the shred", || {
        acknowledgements(&reader, space) == vec![(shred, false), (shred, true)]
    })
    .await;
}

/// A provisional shred (RFC 0008, "A provisional erasure in a space with
/// one account"; NOC-234) keeps the key: a member that folds it writes
/// "seen" only and keeps the key. Once the performer's account finalizes
/// it, the member writes "deleted" and drops the key.
#[tokio::test(flavor = "multi_thread")]
async fn a_provisional_shred_is_seen_and_only_deleted_once_finalized() {
    let relay = LocalRelay::start().await;
    let (admin, member, project) = shared(&relay).await;
    let key = PersonKey::generate();
    person_key(&admin, &project, &key, &key);
    synced(&admin).await;
    let space = project.id().0;
    stored(&relay, space).await;
    relay.proxy.cut();
    relay.proxy.restore();
    eventually("the member holds the person key", || {
        held(&member, space).is_some()
    })
    .await;

    // Both members belong to one account, so the admin's own account signs
    // off: the shred is provisional.
    let shred = shred_signed(&admin, &project, &account());
    synced(&admin).await;
    eventually("the member writes seen", || {
        acknowledgements(&member, space) == vec![(shred, false)]
    })
    .await;
    assert!(
        held(&member, space).is_some(),
        "a provisional shred keeps the key"
    );

    let decision = Entry::ErasureDecision(membership::ErasureDecision {
        shred,
        finalize: true,
    });
    write(&admin, &project, decision);
    synced(&admin).await;
    eventually("the member writes deleted and drops the key", || {
        acknowledgements(&member, space) == vec![(shred, false), (shred, true)]
            && held(&member, space).is_none()
    })
    .await;
}
