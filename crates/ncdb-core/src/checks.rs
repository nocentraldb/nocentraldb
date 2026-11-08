//! The checks of steps 3 and 5 of RFC 0002 Amendment 1 ("What a member
//! does with a change it receives") with the real signature and
//! decryption of RFC 0003 ("How a member signs and encrypts a change"):
//! [`SealChecks`] verifies with [`seal::verify`](crate::seal::verify) and decrypts with
//! [`seal::open`](crate::seal::open), given the member keys and space keys the member knows.
//!
//! Membership (step 4) and what a control change adds to the control state
//! come in through [`Membership`]; the membership rules of RFC 0008 and key
//! distribution are later tickets.

use std::collections::BTreeMap;
use std::fmt;

use crate::change::{Change, Payload};
use crate::ids::MemberId;
use crate::keys::{MemberPublicKey, SpaceKey};
use crate::log::{ParkReason, RejectReason};
use crate::receive::{Checks, Stop};
use crate::seal::OpenError;

/// Step 4 and the control state behind it: whether the author is a member
/// at the change's key generation, and what a control change that passed
/// steps 1 to 4 adds.
pub trait Membership {
    /// Whether the author is a member at the change's key generation.
    fn is_member(&self, change: &Change) -> bool;
    /// Adds a control change that passed steps 1 to 4.
    fn add_control(&mut self, change: &Change);
}

/// The checks of one space's log, with suite 1's real signature and
/// decryption. Its `Debug` output names member ids and key generations,
/// never a key.
pub struct SealChecks<M> {
    members: BTreeMap<MemberId, MemberPublicKey>,
    space_keys: BTreeMap<u64, SpaceKey>,
    membership: M,
}

impl<M> SealChecks<M> {
    /// Checks that know no member key and no space key yet.
    pub fn new(_membership: M) -> SealChecks<M> {
        todo!()
    }

    /// Learns a member's public keys, under the member id they hash to.
    pub fn add_member(&mut self, _key: MemberPublicKey) {
        todo!()
    }

    /// Learns the space key of a key generation.
    pub fn add_space_key(&mut self, _generation: u64, _key: SpaceKey) {
        todo!()
    }

    /// The membership behind step 4.
    pub fn membership(&self) -> &M {
        &self.membership
    }
}

impl<M: Membership> Checks for SealChecks<M> {
    fn knows_suite(&self, _suite: u64) -> bool {
        todo!()
    }

    fn verify(&self, _change: &Change) -> Result<(), Stop> {
        todo!()
    }

    fn is_member(&self, change: &Change) -> bool {
        self.membership.is_member(change)
    }

    fn open(&self, _change: &Change) -> Result<Payload, Stop> {
        todo!()
    }

    fn add_control(&mut self, change: &Change) {
        self.membership.add_control(change);
    }
}

impl<M> fmt::Debug for SealChecks<M> {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!()
    }
}

/// Where a change stops when [`seal::verify`](crate::seal::verify) or
/// [`seal::open`](crate::seal::open) fails:
/// the step of RFC 0002 Amendment 1 and its reason. An unknown author and a
/// missing space key do not come from here, since they depend on the keys
/// the member holds, not on the change: [`SealChecks`] finds them first.
///
/// | `OpenError`      | Step | Stop                            |
/// | ---------------- | ---- | ------------------------------- |
/// | `NotSealed`      | 2    | parked: unknown kind            |
/// | `UnknownSuite`   | 2    | parked: unknown suite           |
/// | `AuthorMismatch` | 3    | rejected: bad signature         |
/// | `AuthorKey`      | 3    | rejected: bad signature         |
/// | `Signature`      | 3    | rejected: bad signature         |
/// | `Decryption`     | 5    | rejected: undecryptable         |
/// | `Payload`        | 6    | parked: unknown payload format  |
impl From<OpenError> for Stop {
    fn from(error: OpenError) -> Stop {
        match error {
            // Only a sealed change has a payload to decrypt; a kind this
            // member does not decrypt is one it does not understand.
            OpenError::NotSealed(_) => Stop::Parked(ParkReason::UnknownKind),
            OpenError::UnknownSuite(_) => Stop::Parked(ParkReason::UnknownSuite),
            OpenError::AuthorMismatch | OpenError::AuthorKey | OpenError::Signature => {
                Stop::Rejected(RejectReason::BadSignature)
            }
            OpenError::Decryption => Stop::Rejected(RejectReason::Undecryptable),
            // A newer payload format may not decode as format 1 (NOC-28).
            OpenError::Payload(_) => Stop::Parked(ParkReason::UnknownPayloadFormat),
        }
    }
}
