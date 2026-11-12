//! Which waiting person-key records a session puts (RFC 0007, "Keys and the
//! person-key store"; NOC-206): only those whose person-key entry the relay
//! has confirmed, since the relay refuses with `bad_record`, and stores
//! none of the request, any record whose entry its log does not hold yet.
//! An entry the member wrote while a push was on the wire reaches the relay
//! only with the next push, so its record waits for it.

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so these tests run on both targets (NOC-175).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ed25519_dalek::SigningKey;
use ncdb_core::keys::PersonKey;
use ncdb_core::membership::{
    self, ControlBody, Entry, PersonKeyEntry, record_hash, sign_record, wrap_person_key,
};

use super::person_keys::ready;
use super::wire::Record;
use crate::db::Db;
use crate::members::tests::{of_account, space};
use crate::space::Space;

const PERSON: [u8; 16] = [0x9e; 16];

/// The admin writes the person-key entry of a new key into `space` and
/// returns its record and the entry's change id.
fn entry(admin: &Db, space: &Space) -> (Record, ncdb_core::ids::ChangeId) {
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
    let change = membership::write(
        &member,
        id,
        next.sequence,
        generation,
        &ControlBody::new(
            crate::members::control_heads(view, id),
            Entry::PersonKey(PersonKeyEntry {
                person_key: PERSON,
                commitment: key.commitment(id, PERSON),
                generation,
                record_hash: record_hash(&wrapped),
            }),
        ),
    );
    crate::space::fold(view, &mut state.checks, &change).unwrap();
    let record = Record {
        person_key: PERSON,
        generation,
        writer: member.id().0,
        wrapped: wrapped.to_vec(),
        signature,
    };
    (record, change.id())
}

#[test]
fn a_record_waits_until_the_relay_confirmed_the_entry_that_announces_it() {
    let admin = of_account(1, &SigningKey::from_bytes(&[9; 32]));
    let project = space(&admin);
    let (record, change) = entry(&admin, &project);
    let id = project.id().0;

    let (send, keep) = ready(&admin.lock(), id, vec![record.clone()]);
    assert!(send.is_empty());
    assert_eq!(keep.len(), 1);

    admin.lock().view.confirm(id, change, 9).unwrap();
    let (send, keep) = ready(&admin.lock(), id, vec![record]);
    assert_eq!(send.len(), 1);
    assert!(keep.is_empty());
}

#[test]
fn a_record_no_entry_announces_is_put_and_left_to_the_relay() {
    let admin = of_account(1, &SigningKey::from_bytes(&[9; 32]));
    let project = space(&admin);
    let (mut record, _) = entry(&admin, &project);
    record.wrapped[0] ^= 1;

    // No entry announces these bytes: waiting would never end, and the
    // relay refuses the record, which drops it.
    let (send, keep) = ready(&admin.lock(), project.id().0, vec![record]);

    assert_eq!(send.len(), 1);
    assert!(keep.is_empty());
}
