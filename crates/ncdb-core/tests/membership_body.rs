//! The genesis and membership entries of RFC 0008 ("The control bodies"),
//! and the member certificate of RFC 0003 ("Member certificate"), against
//! the vectors of tests/vectors/membership/, which generate.py writes with
//! cbor2, cryptography and PyNaCl, independently of this crate.

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ed25519_dalek::SigningKey;
use ncdb_core::change::{Change, Kind};
use ncdb_core::ids::{ChangeId, SpaceId};
use ncdb_core::keys::{MemberKey, SpaceKey};
use ncdb_core::membership::{
    self, Action, Certificate, ControlBody, Entry, Genesis, MemberKind, MembershipEntry, Role,
};

const CERTIFICATE: &str = include_str!("vectors/membership/certificate.hex");
const SUB_AGENT_CERTIFICATE: &str = include_str!("vectors/membership/sub-agent-certificate.hex");
const SPACE_ID: &str = include_str!("vectors/membership/space-id.hex");
const FIRST_GENERATION: &str = include_str!("vectors/membership/first-generation.hex");
const MODEL_ID_FIELD: &str = include_str!("vectors/membership/model-id-field.hex");
const GENESIS_BODY: &str = include_str!("vectors/membership/genesis.body.hex");
const GENESIS_FRAME: &str = include_str!("vectors/membership/genesis.frame.hex");
const GENESIS_ID: &str = include_str!("vectors/membership/genesis.id.hex");
const ADD_BODY: &str = include_str!("vectors/membership/add.body.hex");
const ADD_FRAME: &str = include_str!("vectors/membership/add.frame.hex");
const ADD_ID: &str = include_str!("vectors/membership/add.id.hex");
const SET_ROLE_ADMIN_BODY: &str = include_str!("vectors/membership/set-role-admin.body.hex");
const SET_ROLE_READER_BODY: &str = include_str!("vectors/membership/set-role-reader.body.hex");
const REMOVE_BODY: &str = include_str!("vectors/membership/remove.body.hex");

/// The model of RFC 0005's example lock.
const MODEL_ID: [u8; 16] = [
    0x6f, 0x1c, 0x0e, 0x2a, 0x9b, 0x7d, 0x4c, 0x3e, 0x8a, 0x5f, 0x0b, 0x1d, 0x2c, 0x3e, 0x4f, 0x5a,
];

fn hex(text: &str) -> Vec<u8> {
    let text: String = text.split_whitespace().collect();
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

fn creator() -> MemberKey {
    MemberKey::from_secrets([0x21; 32], [0x22; 32])
}

fn subject() -> MemberKey {
    MemberKey::from_secrets([0x61; 32], [0x62; 32])
}

fn account() -> SigningKey {
    SigningKey::from_bytes(&[0x11; 32])
}

fn nonce() -> [u8; 16] {
    core::array::from_fn(|i| 0x31 + i as u8)
}

fn space() -> SpaceId {
    SpaceId(hex(SPACE_ID).try_into().unwrap())
}

fn first_generation() -> u64 {
    u64::from_be_bytes(hex(FIRST_GENERATION).try_into().unwrap())
}

fn genesis_id() -> ChangeId {
    ChangeId(hex(GENESIS_ID).try_into().unwrap())
}

#[test]
fn the_space_id_and_the_first_generation_derive_as_the_vector() {
    assert_eq!(membership::space_id(creator().id(), &nonce()), space());
    assert_eq!(
        membership::generation_id(space(), creator().id(), &[]),
        first_generation()
    );
    assert_eq!(
        first_generation() >> 48,
        1,
        "the ordinal of the genesis's generation"
    );
}

#[test]
fn the_certificate_decodes_encodes_and_verifies_as_the_vector() {
    let bytes = hex(CERTIFICATE);
    let certificate = Certificate::decode(&bytes).unwrap();
    assert_eq!(certificate.encode(), bytes);
    assert_eq!(
        certificate.account_key,
        account().verifying_key().to_bytes()
    );
    assert_eq!(certificate.member, creator().id());
    assert_eq!(certificate.kind, MemberKind::Agent);
    assert_eq!(certificate.parent, None);
    assert!(certificate.verify(&certificate.account_key));

    // The account key signs with the account context, so the same bytes
    // signed with the sub-agent context would not verify.
    let mut changed = certificate.clone();
    changed.kind = MemberKind::Device;
    assert!(!changed.verify(&changed.account_key));
    assert!(!certificate.verify(&creator().public().signing));
}

#[test]
fn a_certificate_signed_here_equals_the_vector() {
    let certificate = Certificate::sign(&account(), creator().id(), MemberKind::Agent);
    assert_eq!(certificate.encode(), hex(CERTIFICATE));
}

#[test]
fn the_sub_agent_certificate_verifies_against_its_parent() {
    let bytes = hex(SUB_AGENT_CERTIFICATE);
    let certificate = Certificate::decode(&bytes).unwrap();
    assert_eq!(certificate.encode(), bytes);
    assert_eq!(certificate.kind, MemberKind::SubAgent);
    assert_eq!(certificate.parent, Some(creator().id()));
    assert_eq!(
        certificate.account_key,
        account().verifying_key().to_bytes()
    );
    // Its parent's signing key signs it, under the sub-agent context.
    assert!(certificate.verify(&creator().public().signing));
    assert!(!certificate.verify(&certificate.account_key));

    let sub_agent = MemberKey::from_secrets([0x71; 32], [0x72; 32]);
    let signed = Certificate::sign_sub_agent(&creator(), certificate.account_key, sub_agent.id());
    assert_eq!(signed.encode(), bytes);
}

#[test]
fn a_certificate_with_a_parent_but_not_a_sub_agent_does_not_decode() {
    // kind 1 with key 4: device certificates name no parent.
    let mut bytes = hex(SUB_AGENT_CERTIFICATE);
    let kind = bytes.iter().position(|b| *b == 0x03).unwrap();
    assert_eq!(bytes[kind + 1], 0x03, "key 3 holds kind 3");
    bytes[kind + 1] = 0x01;
    assert!(Certificate::decode(&bytes).is_err());
}

#[test]
fn the_model_id_seals_and_opens_as_the_vector() {
    let key = SpaceKey::from_bytes([0x41; 32]);
    let nonce: [u8; 24] = core::array::from_fn(|i| 0x51 + i as u8);
    let field =
        membership::seal_model_id_with_nonce(&key, space(), creator().id(), MODEL_ID, nonce);
    assert_eq!(field.to_vec(), hex(MODEL_ID_FIELD));
    assert_eq!(
        membership::open_model_id(&key, space(), creator().id(), &field),
        Some(MODEL_ID)
    );
    // Bound to its space and its creator, and to generation 1's key.
    assert_eq!(
        membership::open_model_id(&key, SpaceId([0; 16]), creator().id(), &field),
        None
    );
    assert_eq!(
        membership::open_model_id(&key, space(), subject().id(), &field),
        None
    );
    let other = SpaceKey::from_bytes([0x42; 32]);
    assert_eq!(
        membership::open_model_id(&other, space(), creator().id(), &field),
        None
    );
    // A random nonce: two seals differ, and both open.
    let a = membership::seal_model_id(&key, space(), creator().id(), MODEL_ID);
    let b = membership::seal_model_id(&key, space(), creator().id(), MODEL_ID);
    assert_ne!(a, b);
    assert_eq!(
        membership::open_model_id(&key, space(), creator().id(), &a),
        Some(MODEL_ID)
    );
}

#[test]
fn the_genesis_body_decodes_and_encodes_as_the_vector() {
    let bytes = hex(GENESIS_BODY);
    let body = ControlBody::decode(&bytes).unwrap();
    assert_eq!(body.encode(), bytes);
    assert_eq!(body.entry_type(), membership::GENESIS);
    assert!(body.dependencies.is_empty());
    let Entry::Genesis(genesis) = &body.entry else {
        panic!("not a genesis: {body:?}");
    };
    assert_eq!(genesis.nonce, nonce());
    assert_eq!(genesis.creator, creator().public());
    assert_eq!(genesis.certificate.encode(), hex(CERTIFICATE));
    assert_eq!(genesis.first_generation, first_generation());
    assert_eq!(genesis.model_id.to_vec(), hex(MODEL_ID_FIELD));
    assert_eq!(genesis.space_id(), space());
}

#[test]
fn the_genesis_change_written_here_equals_the_vector() {
    let field: [u8; 56] = hex(MODEL_ID_FIELD).try_into().unwrap();
    let genesis = Genesis {
        nonce: nonce(),
        creator: creator().public(),
        certificate: Certificate::decode(&hex(CERTIFICATE)).unwrap(),
        first_generation: first_generation(),
        model_id: field,
    };
    let body = ControlBody::new(Default::default(), Entry::Genesis(genesis));
    assert_eq!(body.encode(), hex(GENESIS_BODY));
    let change = membership::write(&creator(), space(), 1, first_generation(), &body);
    assert_eq!(change.bytes(), hex(GENESIS_FRAME));
    assert_eq!(change.id(), genesis_id());
    assert_eq!(change.header().kind, Kind::CONTROL);

    let decoded = Change::decode(&hex(GENESIS_FRAME)).unwrap();
    assert_eq!(
        ControlBody::decode(&decoded.header().payload).unwrap(),
        body
    );
}

#[test]
fn the_add_entry_decodes_and_encodes_as_the_vector() {
    let bytes = hex(ADD_BODY);
    let body = ControlBody::decode(&bytes).unwrap();
    assert_eq!(body.encode(), bytes);
    assert_eq!(body.entry_type(), membership::MEMBERSHIP);
    assert_eq!(
        body.dependencies.iter().copied().collect::<Vec<_>>(),
        [genesis_id()]
    );
    let Entry::Membership(MembershipEntry {
        subject: who,
        action,
    }) = &body.entry
    else {
        panic!("not a membership entry: {body:?}");
    };
    assert_eq!(*who, subject().id());
    let Action::Add {
        role,
        key,
        certificate,
        from_generation,
    } = action
    else {
        panic!("not an add: {action:?}");
    };
    assert_eq!(*role, Role::Writer);
    assert_eq!(*key, subject().public());
    assert_eq!(certificate.member, subject().id());
    assert_eq!(certificate.kind, MemberKind::Device);
    assert_eq!(*from_generation, first_generation());

    let change = membership::write(&creator(), space(), 2, first_generation(), &body);
    assert_eq!(change.bytes(), hex(ADD_FRAME));
    assert_eq!(change.id(), ChangeId(hex(ADD_ID).try_into().unwrap()));
}

#[test]
fn set_role_and_remove_decode_and_encode_as_the_vectors() {
    let admin = ControlBody::decode(&hex(SET_ROLE_ADMIN_BODY)).unwrap();
    assert_eq!(admin.encode(), hex(SET_ROLE_ADMIN_BODY));
    assert_eq!(
        admin.entry,
        Entry::Membership(MembershipEntry {
            subject: subject().id(),
            action: Action::SetRole {
                role: Role::Admin,
                from_generation: Some(first_generation()),
                cutoff: 4,
            },
        })
    );

    let reader = ControlBody::decode(&hex(SET_ROLE_READER_BODY)).unwrap();
    assert_eq!(reader.encode(), hex(SET_ROLE_READER_BODY));
    assert_eq!(
        reader.entry,
        Entry::Membership(MembershipEntry {
            subject: subject().id(),
            action: Action::SetRole {
                role: Role::Reader,
                from_generation: None,
                cutoff: 0
            },
        })
    );

    let remove = ControlBody::decode(&hex(REMOVE_BODY)).unwrap();
    assert_eq!(remove.encode(), hex(REMOVE_BODY));
    assert_eq!(
        remove.entry,
        Entry::Membership(MembershipEntry {
            subject: subject().id(),
            action: Action::Remove { cutoff: 7 },
        })
    );
}
