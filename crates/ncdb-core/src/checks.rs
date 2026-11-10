//! The checks of steps 3 to 5 of RFC 0002 Amendment 1 ("What a member
//! does with a change it receives") with the real signature and
//! decryption of RFC 0003 ("How a member signs and encrypts a change"):
//! [`SealChecks`] verifies with [`seal::verify`] and decrypts with
//! [`seal::open`].
//!
//! Everything the checks know comes from the control changes of the space
//! (RFC 0008): the authors' public keys from the genesis and the add
//! entries, the membership from [`membership::resolve`], and the space keys
//! from the key envelopes to the checks' member, which [`envelope::open`]
//! unwraps. Each control change that passes therefore checks the log's
//! rejected changes again (D11).

use std::collections::BTreeMap;
use std::fmt;

use crate::change::{Change, Kind, Payload};
use crate::envelope;
use crate::ids::{ChangeId, MemberId, SpaceId};
use crate::keys::{MemberKey, MemberPublicKey, SpaceKey};
use crate::log::{ParkReason, RejectReason};
use crate::membership::{self, Action, ControlBody, Entry, MembershipEntry, Resolved};
use crate::receive::{Checks, Stop};
use crate::seal::{self, OpenError};

/// The checks of one space's log, as one member holds them, with suite 1's
/// real signature and decryption. Its `Debug` output names member ids and
/// key generations, never a key.
///
/// **Control changes.** A control change passes once its signature verifies
/// with keys that a control change the checks hold names, or that it names
/// itself, as a genesis does. Every control change that passes goes into
/// the set [`membership::resolve`] folds, whatever its verdict there, since
/// the verdict is a function of the whole set; [`SealChecks::resolved`]
/// gives the verdicts.
///
/// **What this does not do.** A space key, once unwrapped, stays known,
/// even if a later control change rejects its envelope. Whether a sealed
/// change that passed still passes when a later control change arrives is
/// asked again through [`Checks::is_member`], and the log un-folds it when
/// it no longer does (RFC 0006, "Un-folding").
pub struct SealChecks {
    space: SpaceId,
    member: MemberKey,
    controls: BTreeMap<ChangeId, Change>,
    keys: BTreeMap<MemberId, MemberPublicKey>,
    space_keys: BTreeMap<u64, SpaceKey>,
    resolved: Resolved,
}

impl SealChecks {
    /// The checks of `space` for `member`, which hold no control change
    /// yet.
    pub fn new(space: SpaceId, member: MemberKey) -> SealChecks {
        SealChecks {
            space,
            member,
            controls: BTreeMap::new(),
            keys: BTreeMap::new(),
            space_keys: BTreeMap::new(),
            resolved: membership::resolve(space, []),
        }
    }

    /// The control state of the control changes that passed.
    pub fn resolved(&self) -> &Resolved {
        &self.resolved
    }

    /// The author's public keys; without them, the change stops at step 3
    /// as "unknown author". A control change may name its own author's keys.
    fn author(&self, change: &Change) -> Result<MemberPublicKey, Stop> {
        let author = change.header().author;
        let own = match change.header().kind {
            Kind::CONTROL => ControlBody::decode(&change.header().payload)
                .ok()
                .and_then(|body| named_key(&body)),
            _ => None,
        };
        self.keys
            .get(&author)
            .copied()
            .or(own.filter(|key| key.id() == author))
            .ok_or(Stop::Rejected(RejectReason::UnknownAuthor))
    }
}

/// The public keys a control body names: a genesis its creator's, an add
/// its subject's. They hash to the member id.
fn named_key(body: &ControlBody) -> Option<MemberPublicKey> {
    match &body.entry {
        Entry::Genesis(genesis) => Some(genesis.creator),
        Entry::Membership(MembershipEntry {
            action: Action::Add { key, .. },
            ..
        }) => Some(*key),
        _ => None,
    }
}

impl Checks for SealChecks {
    fn knows_suite(&self, suite: u64) -> bool {
        suite == seal::SUITE
    }

    fn verify(&self, change: &Change) -> Result<(), Stop> {
        Ok(seal::verify(change, &self.author(change)?)?)
    }

    /// A control change's verdict is `resolve`'s; a sealed change's is
    /// RFC 0008's membership verdict.
    fn is_member(&self, change: &Change) -> bool {
        change.header().kind == Kind::CONTROL || self.resolved.admits(change.header())
    }

    fn open(&self, change: &Change) -> Result<Payload, Stop> {
        let author = self.author(change)?;
        let key = self
            .space_keys
            .get(&change.header().key_generation)
            .ok_or(Stop::Rejected(RejectReason::NoKey))?;
        Ok(seal::open(change, &author, key)?)
    }

    fn add_control(&mut self, change: &Change) {
        if change.header().space != self.space || self.controls.contains_key(&change.id()) {
            return;
        }
        if let Ok(body) = ControlBody::decode(&change.header().payload) {
            if let Some(key) = named_key(&body) {
                self.keys.insert(key.id(), key);
            }
            // An envelope to another member, or one that does not open,
            // gives no key.
            if let Entry::KeyEnvelope(sealed) = &body.entry
                && let Ok(key) = envelope::open(sealed, self.space, &self.member)
            {
                self.space_keys.insert(sealed.generation, key);
            }
        }
        self.controls.insert(change.id(), change.clone());
        self.resolved = membership::resolve(self.space, self.controls.values());
    }
}

impl fmt::Debug for SealChecks {
    /// The checks' member, the member ids whose keys they know, the key
    /// generations of the space keys they hold and how many control changes
    /// they hold.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SealChecks")
            .field("member", &self.member.id())
            .field("members", &self.keys.keys())
            .field("key_generations", &self.space_keys.keys())
            .field("controls", &self.controls.len())
            .finish_non_exhaustive()
    }
}

/// Where a change stops when [`seal::verify`] or [`seal::open`] fails: the
/// step of RFC 0002 Amendment 1 and its reason. An unknown author and a
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
