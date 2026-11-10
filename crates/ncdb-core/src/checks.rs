//! The checks of steps 3 to 5 of RFC 0002, "What a member does
//! with a change it receives", with the real signature and
//! decryption of RFC 0003 ("How a member signs and encrypts a change"):
//! [`SealChecks`] verifies with [`seal::verify`] and decrypts with
//! [`seal::open`].
//!
//! Everything the checks know comes from the control changes of the space
//! (RFC 0008): the authors' public keys from the genesis and the add
//! entries, the membership from [`membership::resolve`], and the space keys
//! from the key envelopes to the checks' member, which [`envelope::open`]
//! unwraps, and from the key chain entries, which
//! [`membership::open_previous_key`] opens back to the first generation
//! (RFC 0003, "Key-chain subkey"). Each control change that passes
//! therefore checks the log's rejected changes again (D11).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::change::{Change, Kind, Payload};
use crate::envelope;
use crate::ids::{ChangeId, MemberId, SpaceId};
use crate::keys::{MemberKey, MemberPublicKey, SpaceKey};
use crate::log::{ParkReason, RejectReason};
use crate::membership::{
    self, Action, ControlBody, Entry, KeyChainEntry, MembershipEntry, Resolved,
};
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
/// **Space keys.** An envelope to the checks' member gives the key of its
/// generation, and that key wins over any other for it. A key chain entry
/// that the generation graph holds, valid when written, gives the keys of
/// its previous generations, once the key of its own generation is known,
/// so a member added after rotations reaches the first generation. The
/// key chain keys are derived again from scratch after every control
/// change, from the envelope keys and the set of entries alone: the walk
/// takes the entries in change id order, pass after pass until no entry
/// gives a new key, and a generation keeps the first key a pass opens for
/// it. Two entries that give different keys for one generation, which only
/// an admin that holds the new key could write, so leave every copy with
/// the same key, in whichever order they arrived.
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
    envelope_keys: BTreeMap<u64, SpaceKey>,
    chain_keys: BTreeMap<u64, SpaceKey>,
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
            envelope_keys: BTreeMap::new(),
            chain_keys: BTreeMap::new(),
            resolved: membership::resolve(space, []),
        }
    }

    /// The control state of the control changes that passed.
    pub fn resolved(&self) -> &Resolved {
        &self.resolved
    }

    /// The space key of `generation`, when an envelope to the checks'
    /// member unwrapped it or the key chain opened it: the key a writer
    /// seals under, and a reader opens with.
    pub fn space_key(&self, generation: u64) -> Option<&SpaceKey> {
        self.envelope_keys
            .get(&generation)
            .or_else(|| self.chain_keys.get(&generation))
    }

    /// The `model.id` the space's genesis names, opened with the space key
    /// of its first generation (RFC 0008, "What it names"). `None` while
    /// the checks hold no genesis or no key of the first generation, or
    /// when the sealed id does not open under that key.
    pub fn model_id(&self) -> Option<[u8; 16]> {
        let genesis = self.controls.get(&self.resolved.genesis()?)?;
        let body = ControlBody::decode(&genesis.header().payload).ok()?;
        let Entry::Genesis(genesis) = body.entry else {
            return None;
        };
        let key = self.space_key(genesis.first_generation)?;
        membership::open_model_id(key, self.space, genesis.creator.id(), &genesis.model_id)
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
            .space_key(change.header().key_generation)
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
                self.envelope_keys.insert(sealed.generation, key);
            }
        }
        self.controls.insert(change.id(), change.clone());
        self.resolved = membership::resolve(self.space, self.controls.values());
        self.walk_key_chain();
    }
}

impl SealChecks {
    /// Derives the key chain keys again from scratch: opens the previous
    /// keys of every key chain entry the generation graph holds whose own
    /// generation's key is known, in passes over the entries in change id
    /// order, until a pass gives no new key. A previous generation that an
    /// envelope gave a key for, or that an earlier entry or pass filled,
    /// keeps that key. The result depends only on the envelope keys and the
    /// set of entries, never on the order in which they arrived.
    fn walk_key_chain(&mut self) {
        self.chain_keys.clear();
        let chains: Vec<(ChangeId, KeyChainEntry)> = self
            .controls
            .iter()
            .filter_map(|(&id, change)| {
                match ControlBody::decode(&change.header().payload).ok()?.entry {
                    Entry::KeyChain(chain) => Some((id, chain)),
                    _ => None,
                }
            })
            .filter(|(id, chain)| {
                self.resolved
                    .generation(chain.generation)
                    .is_some_and(|generation| generation.entries.contains_key(id))
            })
            .collect();
        loop {
            let mut opened = false;
            for (_, chain) in &chains {
                let Some(new_key) = self.space_key(chain.generation) else {
                    continue;
                };
                let found: Vec<(u64, SpaceKey)> = chain
                    .previous
                    .iter()
                    .filter(|previous| self.space_key(previous.generation).is_none())
                    .filter_map(|previous| {
                        let key = membership::open_previous_key(
                            new_key,
                            self.space,
                            chain.generation,
                            previous.generation,
                            &previous.key,
                        )?;
                        Some((previous.generation, key))
                    })
                    .collect();
                opened |= !found.is_empty();
                self.chain_keys.extend(found);
            }
            if !opened {
                return;
            }
        }
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
            .field(
                "key_generations",
                &self
                    .envelope_keys
                    .keys()
                    .chain(self.chain_keys.keys())
                    .collect::<BTreeSet<_>>(),
            )
            .field("controls", &self.controls.len())
            .finish_non_exhaustive()
    }
}

/// Where a change stops when [`seal::verify`] or [`seal::open`] fails: the
/// step of RFC 0002, "What a member does with a change it receives", and
/// its reason. An unknown author and a
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
