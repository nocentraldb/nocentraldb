//! The audit of a space's erasures (RFC 0009, "Erasures"; Carsten,
//! 2026-11-12, NOC-246): per shred its state, its performer and the
//! account that signed it off, and every member of the shred's causal past
//! with whether it is still a member and what it acknowledged.

use ncdb_core::keys::PersonKey;
use ncdb_core::membership::{self, Entry};

use super::person_keys::{GUEST, person_key, shared, shred_signed, shredded, write};
use super::support::*;
use crate::db::{Db, lock};
use crate::erasures::{Acknowledged, Erasure, ErasureMember, ErasureState};
use crate::id::ChangeId;
use crate::members::{AddMember, Fingerprint, MemberId, Role};
use crate::space::Space;

fn id(db: &Db) -> MemberId {
    MemberId(lock(&db.inner).member.key().id())
}

fn fingerprint(db: &Db) -> Fingerprint {
    now(db.invitation()).fingerprint()
}

fn list(space: &Space) -> Vec<Erasure> {
    now(space.erasures().list()).expect("the list")
}

fn listed(erasure: &Erasure, member: MemberId) -> Option<ErasureMember> {
    erasure.members.iter().find(|m| m.id == member).copied()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_space_without_erasures_lists_none() {
    let relay = LocalRelay::start().await;
    let (_admin, _member, project) = shared(&relay).await;

    assert!(list(&project).is_empty());
}

/// A final shred lists its id, performer, sign-off and the three members
/// of its causal past; the member that held the key acknowledges
/// "deleted", which the performer's copy shows once it syncs.
#[tokio::test(flavor = "multi_thread")]
async fn a_final_shred_lists_its_performer_its_sign_off_and_who_deleted_the_key() {
    let relay = LocalRelay::start().await;
    let (admin, member, project, shred) = shredded(&relay).await;
    // The signer's account, opened with `open_as`: its recovery key derives
    // from its account key, and its fingerprint is over the recovery key.
    let bits: [u8; 16] = space::other_account(1).to_bytes()[..16].try_into().unwrap();
    let recovery = ncdb_core::recovery::RecoveryCode::from_bits(bits).recovery_key();
    let signer = Fingerprint(recovery.public());

    eventually("the admin's copy shows the member deleted the key", || {
        list(&project)
            .first()
            .and_then(|e| member_of(e, id(&member)))
            == Some(Acknowledged::Deleted)
    })
    .await;

    let erasures = list(&project);
    assert_eq!(erasures.len(), 1);
    let erasure = &erasures[0];
    assert_eq!(erasure.shred, ChangeId(shred));
    assert_eq!(erasure.state, ErasureState::Final);
    assert_eq!(erasure.performer, id(&admin));
    assert_eq!(erasure.performer_account, fingerprint(&admin));
    assert_eq!(erasure.sign_off, signer);
    assert_eq!(erasure.members.len(), 3, "admin, member and signer");
    assert!(erasure.members.iter().all(|m| m.current));
    let mine = listed(erasure, id(&admin)).expect("the performer is listed");
    assert_eq!(mine.account, fingerprint(&admin));
}

fn member_of(erasure: &Erasure, id: MemberId) -> Option<Acknowledged> {
    listed(erasure, id).map(|m| m.acknowledged)
}

/// A device removed after the shred that never syncs again kept the key
/// as far as anyone knows: it is listed, not current, and missing.
#[tokio::test(flavor = "multi_thread")]
async fn a_removed_member_that_never_answered_is_listed_as_missing() {
    let relay = LocalRelay::start().await;
    let (admin, _member, project) = shared(&relay).await;
    let guest = open(GUEST, &relay.url()).await;
    let invitation = now(guest.invitation());
    let guest_id = id(&guest);
    drop(guest);
    let options = AddMember {
        role: Role::Reader,
        fingerprint: None,
    };
    now(project.members().add(&invitation, options)).unwrap();
    let key = PersonKey::generate();
    person_key(&admin, &project, &key, &key);
    shred_signed(&admin, &project, &account());
    now(project.members().remove(guest_id)).unwrap();

    let erasure = list(&project).pop().expect("one erasure");

    let guest = listed(&erasure, guest_id).expect("the removed guest is listed");
    assert!(!guest.current);
    assert_eq!(guest.acknowledged, Acknowledged::Missing);
}

/// A provisional shred is listed as provisional until a decision ends it;
/// a take-back lists it as taken back.
#[tokio::test(flavor = "multi_thread")]
async fn a_provisional_shred_is_listed_as_provisional_then_taken_back() {
    let relay = LocalRelay::start().await;
    let (admin, _member, project) = shared(&relay).await;
    let key = PersonKey::generate();
    person_key(&admin, &project, &key, &key);
    let shred = shred_signed(&admin, &project, &account());

    assert_eq!(list(&project)[0].state, ErasureState::Provisional);
    assert_eq!(list(&project)[0].sign_off, fingerprint(&admin));

    let decision = Entry::ErasureDecision(membership::ErasureDecision {
        shred,
        finalize: false,
    });
    write(&admin, &project, decision);

    assert_eq!(list(&project)[0].state, ErasureState::TakenBack);
}

/// Every member may read the audit: a reader's copy lists the shred too.
#[tokio::test(flavor = "multi_thread")]
async fn a_reader_lists_the_erasures_too() {
    let relay = LocalRelay::start().await;
    let (admin, _member, project) = shared(&relay).await;
    let reader = open(GUEST, &relay.url()).await;
    let options = AddMember {
        role: Role::Reader,
        fingerprint: None,
    };
    now(project.members().add(&now(reader.invitation()), options)).unwrap();
    let key = PersonKey::generate();
    person_key(&admin, &project, &key, &key);
    let shred = shred_signed(&admin, &project, &account());
    synced(&admin).await;

    eventually("the reader lists the shred", || {
        now(reader.spaces().get(project.id()))
            .and_then(|space| now(space.erasures().list()))
            .is_ok_and(|e| e.first().is_some_and(|e| e.shred == ChangeId(shred)))
    })
    .await;
}

/// Of two shreds of one person key, the first not taken back takes effect
/// (Carsten, 2026-11-12, NOC-242): after a take-back, a later shred erases
/// again, and the audit lists both, the first as taken back.
#[tokio::test(flavor = "multi_thread")]
async fn a_shred_after_a_take_back_is_listed_beside_the_one_taken_back() {
    let relay = LocalRelay::start().await;
    let (admin, _member, project) = shared(&relay).await;
    let key = PersonKey::generate();
    person_key(&admin, &project, &key, &key);
    let first = shred_signed(&admin, &project, &account());
    let decision = Entry::ErasureDecision(membership::ErasureDecision {
        shred: first,
        finalize: false,
    });
    write(&admin, &project, decision);
    let second = shred_signed(&admin, &project, &account());

    let erasures = list(&project);

    let states: Vec<_> = erasures.iter().map(|e| (e.shred, e.state)).collect();
    let mut expected = vec![
        (ChangeId(first), ErasureState::TakenBack),
        (ChangeId(second), ErasureState::Provisional),
    ];
    expected.sort_by_key(|(shred, _)| *shred);
    assert_eq!(states, expected);
    assert!(erasures.iter().all(|e| e.performer == id(&admin)));
}
