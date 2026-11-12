//! Membership, roles and the first change of a space, as RFC 0008
//! (`docs/rfc/0008-membership.md`) defines them.
//!
//! A space starts with its genesis ([`Genesis`]), whose creator is its
//! first admin, and its membership comes from membership entries
//! ([`MembershipEntry`]) in control changes ([`ControlBody`]). Members and
//! the relay fold the control changes of a space they hold into one control
//! state with [`resolve`], the same on every copy, in any order of arrival.
//! The fold reads only control bodies and headers, so the relay runs the
//! same function (RFC 0007, "What the relay checks").
//!
//! The member certificate of RFC 0003 ("Member certificate") is
//! [`Certificate`].

mod body;
mod certificate;
mod current;
mod fields;
mod generations;
mod genesis;
mod input;
mod key_chain;
mod person_key;
mod person_key_wrap;
mod person_keys;
mod resolve;
mod rights;
mod stable;
mod state;
mod statement;

pub use body::{
    ACCOUNT_KEY_STATEMENT, ACKNOWLEDGEMENT, Action, BodyError, ControlBody, Entry, GENESIS, KEY_CHAIN, KEY_ENVELOPE,
    MEMBERSHIP, MembershipEntry, PERSON_KEY, Role, SHRED, write,
};
pub use certificate::{Certificate, MemberKind};
pub use current::MergeRotation;
pub use generations::Generation;
pub use genesis::{
    GENERATION_CONTEXT, Genesis, MODEL_ID_CONTEXT, MODEL_ID_FIELD_LEN, SPACE_ID_CONTEXT,
    generation_id, open_model_id, seal_model_id, seal_model_id_with_nonce, space_id,
};
pub use key_chain::{
    KeyChainEntry, PREVIOUS_KEY_LEN, PreviousKey, open_previous_key, seal_previous_key,
    seal_previous_key_with_nonce,
};
pub use person_key::{
    PersonKeyEntry, RECORD_CONTEXT, SIGN_OFF_CONTEXT, ShredEntry, SignOff, person_key_commitment,
    record_hash, sign_record, verify_record,
};
pub use person_key_wrap::{
    WRAPPED_PERSON_KEY_LEN, unwrap_person_key, wrap_person_key, wrap_person_key_with_nonce,
};
pub use person_keys::PersonKey;
pub use resolve::{Resolved, resolve};
pub use stable::Stability;
pub use state::{Grant, MemberState, Rejection, Revocation, Verdict};
pub use statement::AccountKeyStatement;
