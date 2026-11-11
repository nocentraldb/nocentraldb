//! A rotation of the SDK (RFC 0003, "Removing a member and rotating the
//! space key"; RFC 0008, "Key chain entry"): it chains every head of the
//! generation graph, and re-wraps every live person key under the fresh
//! generation, with a person-key entry that repeats the key's commitment
//! and a signed record handed to syncing (step 3; NOC-206 part 3). A space
//! with a live person key the member does not hold is not rotated, and
//! nothing is written.

use ed25519_dalek::SigningKey;
use ncdb_core::keys::PersonKey;
use ncdb_core::membership::{
    self, ControlBody, Entry, PersonKeyEntry, record_hash, sign_record, wrap_person_key,
};

use super::tests::{joined, now, remove, writer};
use super::{MemberId, Role};
use crate::db::Db;
use crate::error::ErrorCode;
use crate::space::Space;

/// The person-key id the tests' held person key has.
const PERSON: [u8; 16] = [0x9e; 16];

/// The admin announces a person key in `space`, through the core, as
/// another client could, without its record: the admin does not hold it.
fn person_key(admin: &Db, space: &Space) {
    let mut inner = admin.lock();
    let member = inner.member.key();
    let id = space.id().0;
    let crate::db::Inner { view, spaces, .. } = &mut *inner;
    let state = spaces.get_mut(&id).expect("the admin holds the space");
    let next = crate::next::next(view, id, &state.checks, member.id());
    let generation = next.generation.expect("a generation");
    let entry = Entry::PersonKey(PersonKeyEntry {
        person_key: [0x0f; 16],
        commitment: [0xc0; 32],
        generation,
        record_hash: membership::record_hash(b"a wrapped record"),
    });
    let change = membership::write(
        &member,
        id,
        next.sequence,
        generation,
        &ControlBody::new(super::control_heads(view, id), entry),
    );
    crate::space::fold(view, &mut state.checks, &change).unwrap();
}

/// The number of changes the admin's log of `space` holds.
fn changes(admin: &Db, space: &Space) -> usize {
    let inner = admin.lock();
    let log = inner.view.log(space.id().0).expect("the space's log");
    log.after(ncdb_core::log::Position::START).count()
}

/// The admin creates the person key `PERSON` in `space`: its person-key
/// entry, through the core, and its signed record wrapped under the
/// current generation, which the admin hands to syncing and so holds.
/// Returns the key.
fn held_person_key(admin: &Db, space: &Space) -> PersonKey {
    let key = PersonKey::generate();
    let id = space.id().0;
    let mut inner = admin.lock();
    let member = inner.member.key();
    let crate::db::Inner { view, spaces, .. } = &mut *inner;
    let state = spaces.get_mut(&id).expect("the admin holds the space");
    let next = crate::next::next(view, id, &state.checks, member.id());
    let generation = next.generation.expect("a generation");
    let space_key = state.checks.space_key(generation).expect("its key");
    let wrapped = wrap_person_key(space_key, id, generation, PERSON, &key);
    let signature = sign_record(&member, id, PERSON, generation, &wrapped);
    let entry = Entry::PersonKey(PersonKeyEntry {
        person_key: PERSON,
        commitment: key.commitment(id, PERSON),
        generation,
        record_hash: record_hash(&wrapped),
    });
    let change = membership::write(
        &member,
        id,
        next.sequence,
        generation,
        &ControlBody::new(super::control_heads(view, id), entry),
    );
    crate::space::fold(view, &mut state.checks, &change).unwrap();
    let record = crate::sync::Record {
        person_key: PERSON,
        generation,
        writer: member.id().0,
        wrapped: wrapped.to_vec(),
        signature,
    };
    crate::sync::put_person_keys(&mut inner, id, vec![record]);
    key
}

/// The commitment of the person key `PERSON` that `db` holds in `space`.
fn held(db: &Db, space: &Space) -> Option<[u8; 32]> {
    let inner = db.lock();
    let key = inner.spaces[&space.id().0].person_keys.get(&PERSON)?;
    Some(key.commitment(space.id().0, PERSON))
}

/// The records the log of `db` announces for `PERSON` in `space`, by
/// generation and record hash, and the commitment of the person key.
fn announced(db: &Db, space: &Space) -> (Vec<(u64, [u8; 32])>, Option<[u8; 32]>) {
    let inner = db.lock();
    let resolved = inner.spaces[&space.id().0].checks.resolved();
    let key = resolved.person_key(PERSON).expect("the person key exists");
    (key.records.iter().copied().collect(), key.commitment)
}

/// The space's current generation on `db`.
fn current(db: &Db, space: &Space) -> u64 {
    db.lock().spaces[&space.id().0]
        .checks
        .resolved()
        .current_generation()
        .expect("a current generation")
}

/// A removal rotates, and the rotation re-wraps the person key the admin
/// holds: a further person-key entry announces a record under the fresh
/// generation and repeats the commitment, and the admin still holds the key.
#[test]
fn a_removal_re_wraps_a_held_person_key_under_the_fresh_generation() {
    let account = SigningKey::from_bytes(&[9; 32]);
    let (admin, project, _, invitation) = joined(&account, Role::Writer);
    let key = held_person_key(&admin, &project);
    let before = current(&admin, &project);
    let commitment = key.commitment(project.id().0, PERSON);

    now(project.members().remove(MemberId(invitation.key.id()))).unwrap();

    let fresh = current(&admin, &project);
    assert_ne!(fresh, before);
    let (records, committed) = announced(&admin, &project);
    let generations: Vec<u64> = records.iter().map(|(generation, _)| *generation).collect();
    assert_eq!(generations.len(), 2, "{generations:?}");
    assert!(generations.contains(&before) && generations.contains(&fresh));
    assert_eq!(committed, Some(commitment));
    assert_eq!(held(&admin, &project), Some(commitment));
}

/// The record a rotation writes holds: its writer signed it, it unwraps
/// under the fresh generation, and its key matches the commitment. A
/// member that lost the key holds it again from that record alone.
#[test]
fn the_re_wrapped_record_unwraps_to_the_person_key_under_the_fresh_generation() {
    let account = SigningKey::from_bytes(&[9; 32]);
    let (admin, project, _, _) = joined(&account, Role::Writer);
    let key = held_person_key(&admin, &project);

    let (fresh, records) = rotate_on(&admin, &project);

    assert_eq!(records.len(), 1);
    let record = &records[0];
    assert_eq!(record.person_key, PERSON);
    assert_eq!(record.generation, fresh);
    assert_eq!(record.writer, admin.lock().member.key().id().0);
    let (announced, _) = announced(&admin, &project);
    assert!(announced.contains(&(fresh, record_hash(&record.wrapped))));
    let id = project.id().0;
    {
        let mut inner = admin.lock();
        inner.spaces.get_mut(&id).unwrap().person_keys.clear();
        crate::sync::put_person_keys(&mut inner, id, records);
    }
    assert_eq!(held(&admin, &project), Some(key.commitment(id, PERSON)));
}

/// A former member is added again after a rotation, which re-wraps the
/// person key the admin holds.
#[test]
fn a_former_member_of_a_space_whose_person_key_the_admin_holds_is_added_again() {
    let account = SigningKey::from_bytes(&[9; 32]);
    let (admin, project, _, invitation) = joined(&account, Role::Writer);
    held_person_key(&admin, &project);
    now(project.members().remove(MemberId(invitation.key.id()))).unwrap();

    now(project.members().add(&invitation, writer())).unwrap();

    let (records, _) = announced(&admin, &project);
    assert_eq!(records.len(), 3, "{records:?}");
    let fresh = current(&admin, &project);
    assert!(records.iter().any(|(generation, _)| *generation == fresh));
}

/// A live person key the admin does not hold cannot be re-wrapped: the
/// rotation is refused before anything is written.
#[test]
fn a_former_member_of_a_space_with_a_person_key_the_admin_does_not_hold_is_not_added_again() {
    let account = SigningKey::from_bytes(&[9; 32]);
    let (admin, project, _, invitation) = joined(&account, Role::Writer);
    person_key(&admin, &project);
    remove(&admin, &project, &invitation);
    let before = changes(&admin, &project);

    let refused = now(project.members().add(&invitation, writer())).unwrap_err();

    assert_eq!(refused.code(), ErrorCode::InvalidValue);
    assert_eq!(changes(&admin, &project), before);
}

/// `db` rotates `space` on its own, through the SDK's rotation. Returns
/// the fresh generation and the person-key records the rotation wrote.
fn rotate_on(db: &Db, space: &Space) -> (u64, Vec<crate::sync::Record>) {
    let mut inner = db.lock();
    let member = inner.member.key();
    let crate::db::Inner {
        view,
        spaces,
        member: secrets,
        ..
    } = &mut *inner;
    let state = spaces
        .get_mut(&space.id().0)
        .expect("the database holds the space");
    let mut batch = super::batch::Batch::new(view, state, &member, space.id().0);
    let fresh = super::rotate::rotate(
        &mut batch,
        state,
        &member,
        space.id().0,
        Default::default(),
        None,
    );
    let records = batch.commit(view, state, secrets).unwrap();
    (fresh, records)
}

/// The heads of the generation graph in `db`'s copy of `space`.
fn heads(db: &Db, space: &Space) -> std::collections::BTreeSet<u64> {
    db.lock().spaces[&space.id().0].checks.resolved().heads()
}

/// Two admins rotate at once, so the generation graph has two heads; the
/// next rotation lists both as previous, as a merge rotation does (RFC
/// 0008, "Two removals that rotate the key at once", step 5), and leaves
/// one head.
#[test]
fn a_rotation_chains_every_head_of_the_generation_graph() {
    let account = SigningKey::from_bytes(&[9; 32]);
    let (admin, project, other, _) = joined(&account, Role::Admin);
    let guest = super::tests::of_account(3, &account);
    let invitation = now(guest.invitation());
    now(project.members().add(&invitation, writer())).unwrap();
    super::tests::copy(&admin, &other, &project);
    let (ours, _) = rotate_on(&admin, &project);
    let (theirs, _) = rotate_on(&other, &project);
    super::tests::copy(&other, &admin, &project);
    assert_eq!(heads(&admin, &project), [ours, theirs].into());
    remove(&admin, &project, &invitation);

    now(project.members().add(&invitation, writer())).unwrap();

    let after = heads(&admin, &project);
    assert_eq!(after.len(), 1, "{after:?}");
    let current = *after.first().unwrap();
    let inner = admin.lock();
    let resolved = inner.spaces[&project.id().0].checks.resolved();
    assert!(resolved.descends(current, ours) && resolved.descends(current, theirs));
}

/// Gives `to` the control changes of `space` that `from` holds and that
/// `keep` keeps, as a partial pull would.
fn copy_some(from: &Db, to: &Db, space: &Space, keep: impl Fn(&Entry) -> bool) {
    let changes: Vec<Vec<u8>> = {
        let inner = from.lock();
        let log = inner.view.log(space.id().0).expect("the space's log");
        log.after(ncdb_core::log::Position::START)
            .filter(|(_, change)| {
                ControlBody::decode(&change.header().payload).is_ok_and(|body| keep(&body.entry))
            })
            .map(|(_, change)| change.bytes().to_vec())
            .collect()
    };
    let mut inner = to.lock();
    let crate::db::Inner { view, spaces, .. } = &mut *inner;
    let state = spaces
        .get_mut(&space.id().0)
        .expect("the database holds the space");
    for bytes in changes {
        view.receive(&bytes, &mut state.checks)
            .expect("the change is received");
    }
}

/// An admin that holds no key of one head cannot chain it: the rotation
/// is refused before anything is written. The admin rotated twice, so its
/// own generation is current, and it holds only the other admin's key
/// chain entry, not its envelope.
#[test]
fn a_rotation_is_refused_when_the_key_of_a_head_is_not_held() {
    let account = SigningKey::from_bytes(&[9; 32]);
    let (admin, project, other, _) = joined(&account, Role::Admin);
    let guest = super::tests::of_account(3, &account);
    let invitation = now(guest.invitation());
    now(project.members().add(&invitation, writer())).unwrap();
    super::tests::copy(&admin, &other, &project);
    rotate_on(&other, &project);
    rotate_on(&admin, &project);
    rotate_on(&admin, &project);
    copy_some(&other, &admin, &project, |entry| {
        matches!(entry, Entry::KeyChain(_))
    });
    assert_eq!(heads(&admin, &project).len(), 2);
    remove(&admin, &project, &invitation);
    let before = changes(&admin, &project);

    let refused = now(project.members().add(&invitation, writer())).unwrap_err();

    assert_eq!(refused.code(), ErrorCode::InvalidValue);
    assert_eq!(changes(&admin, &project), before);
}
