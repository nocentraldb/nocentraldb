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
//! therefore checks the log's rejected changes again (RFC 0002, "Rejected
//! changes").

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::change::{Change, HeaderFields, Kind, Payload};
use crate::envelope;
use crate::ids::{ChangeId, MemberId, SpaceId};
use crate::keys::{MemberKey, MemberPublicKey, SpaceKey};
use crate::log::{ParkReason, RejectReason};
use crate::membership::{
    self, Action, ControlBody, Entry, Genesis, KeyChainEntry, MembershipEntry, Resolved,
};
use crate::one_key;
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
/// **Space keys.** Every envelope to the checks' member gives a key of its
/// generation, and the checks keep every key their envelopes unwrap to,
/// since only one of them may be right (Carsten, 2026-11-20, NOC-389).
/// Envelope keys win over key chain keys. A key chain entry that the
/// generation graph holds, valid when written, gives the keys of its
/// previous generations, once a key of its own generation is known, so a
/// member added after rotations reaches the first generation. The key
/// chain keys are derived again from scratch after every call that adds
/// control changes, from the envelope keys and the set of entries alone: the walk takes the
/// entries in change id order, pass after pass until no entry gives a new
/// key, and a generation keeps the first key a pass opens for it. Two
/// entries that give different keys for one generation, which only an
/// admin that holds the new key could write, so leave every copy with the
/// same key, in whichever order they arrived.
///
/// **Several keys for a generation.** Each generation has one key the
/// checks open and seal with, [`SealChecks::space_key`], and they never
/// try another on a sealed change. When envelopes gave several, it is the
/// one under which the generation's own control change opens: the
/// genesis's sealed `model.id` for the first generation, its key chain
/// entry for a later one. Until that change arrives, the generation has
/// no key, and its sealed changes wait as "no key". The key therefore
/// depends on the set of changes held, never on the order in which the
/// envelopes arrived, and a member reads under it what every member that
/// holds only the right key reads. An envelope whose key that change does
/// not open under is a local report, [`SealChecks::wrong_envelopes`].
///
/// **Replaced keys.** A key chain key can therefore be replaced by a later
/// control change: by an entry with a lower change id, or by an envelope's
/// key. A sealed change that opened under the old key may then no longer
/// open; [`Checks::still_opens`] says so, and the log un-folds the change
/// (RFC 0006, "Un-folding"). The checks keep every key they held for a
/// generation, but use the replaced ones only in [`Checks::open_folded`],
/// so un-folding finds what the change folded; they never open a change
/// for its checks or its fold with a replaced key.
///
/// **What this does not do.** A space key from an envelope, once
/// unwrapped, stays known, even if a later control change rejects its
/// envelope. Whether a sealed change that passed still passes when a later
/// control change arrives is asked again through [`Checks::is_member`] and
/// [`Checks::still_opens`].
pub struct SealChecks {
    space: SpaceId,
    member: MemberKey,
    controls: BTreeMap<ChangeId, Change>,
    keys: BTreeMap<MemberId, MemberPublicKey>,
    /// Per generation, the key each envelope to the checks' member unwraps
    /// to, by the envelope's change id, with its author.
    envelope_keys: BTreeMap<u64, BTreeMap<ChangeId, (MemberId, SpaceKey)>>,
    chain_keys: BTreeMap<u64, SpaceKey>,
    /// Every key chain entry among the control changes added, by change
    /// id, decoded once.
    key_chain_entries: BTreeMap<ChangeId, KeyChainEntry>,
    /// The key chain entries the generation graph holds, in change id
    /// order, as the last walk found them.
    chains: Vec<(ChangeId, KeyChainEntry)>,
    /// Every key held for a generation, the current one included.
    held: BTreeMap<u64, Vec<SpaceKey>>,
    resolved: Resolved,
    /// `resolve` with a memory of the control changes added before, so
    /// that adding one does not judge the others again (NOC-424).
    resolver: membership::Resolver,
    /// [`SealChecks::resolves`].
    resolves: usize,
    /// [`SealChecks::model_id`], read again after every control change, so
    /// that the check of every sealed change costs no decryption.
    space_model: Option<[u8; 16]>,
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
            key_chain_entries: BTreeMap::new(),
            chains: Vec::new(),
            held: BTreeMap::new(),
            resolved: membership::resolve(space, []),
            resolver: membership::Resolver::new(space),
            resolves: 0,
            space_model: None,
        }
    }

    /// The control state of the control changes that passed.
    pub fn resolved(&self) -> &Resolved {
        &self.resolved
    }

    /// How many control changes the last control change added judged from
    /// the fold of their causal past (RFC 0008, step 1), rather than taking
    /// the judgement from the [`membership::Resolver`]'s memory: one, for a
    /// change written after every other (NOC-424).
    pub fn judged(&self) -> usize {
        self.resolver.judged()
    }

    /// How many times the checks resolved their control changes (RFC
    /// 0008, "The function") since they were made: once for each call
    /// that added a control change they did not hold (NOC-438).
    pub fn resolves(&self) -> usize {
        self.resolves
    }

    /// Adds every control change of `changes` that passed steps 1 to 4, as
    /// [`Checks::add_control`] adds one, and resolves the set once when
    /// any of them is new, instead of once after each: a reopened copy
    /// hands its checks every folded control change of its log at once
    /// (NOC-438). The checks then know what adding the same changes one by
    /// one gives them, since the control state and the keys derive from the
    /// set alone, except the keys a key chain entry gave only before
    /// another replaced them, which no folded change needs any more.
    pub fn add_controls<'a>(&mut self, changes: impl IntoIterator<Item = &'a Change>) {
        let mut added = false;
        for change in changes {
            added |= self.take_control(change);
        }
        if added {
            self.settle();
        }
    }

    /// The space key of `generation`: the key a writer seals under, and the
    /// only one a reader opens with. It is the one key the checks hold for
    /// it, from an envelope to the checks' member or from the key chain.
    /// When envelopes gave several, it is the one under which the
    /// generation's own control change opens, and `None` while the checks
    /// do not hold that change, or it opens under none or several of them.
    pub fn space_key(&self, generation: u64) -> Option<&SpaceKey> {
        match self.keys_of(generation).as_slice() {
            [] => None,
            [only] => Some(only),
            several => one_key::only_one(several, |key| {
                self.confirms(generation, key).filter(|&opens| opens)
            })
            .map(|(key, _)| key),
        }
    }

    /// The key envelopes to the checks' member whose key the generation's
    /// own control change does not open under, in generation and change id
    /// order: a local report, the same on every copy that holds the same
    /// changes (RFC 0008, "What the fold may read").
    pub fn wrong_envelopes(&self) -> Vec<WrongEnvelope> {
        self.envelope_keys
            .iter()
            .flat_map(|(&generation, envelopes)| {
                envelopes
                    .iter()
                    .filter(move |(_, (_, key))| self.confirms(generation, key) == Some(false))
                    .map(move |(&change, &(author, _))| WrongEnvelope {
                        change,
                        author,
                        generation,
                    })
            })
            .collect()
    }

    /// Every key the checks hold now for `generation`, each once: the
    /// envelope keys in change id order, then the key chain's.
    fn keys_of(&self, generation: u64) -> Vec<&SpaceKey> {
        let envelopes = self
            .envelope_keys
            .get(&generation)
            .into_iter()
            .flat_map(|envelopes| envelopes.values().map(|(_, key)| key));
        let mut keys: Vec<&SpaceKey> = Vec::new();
        for key in envelopes.chain(self.chain_keys.get(&generation)) {
            if !keys.iter().any(|held| held.bytes() == key.bytes()) {
                keys.push(key);
            }
        }
        keys
    }

    /// Whether the control change that made `generation` opens under `key`:
    /// the genesis's sealed `model.id` for the first generation, a key chain
    /// entry of the generation graph for a later one. `None` while the
    /// checks hold no such change.
    fn confirms(&self, generation: u64, key: &SpaceKey) -> Option<bool> {
        if let Some(genesis) = self.genesis()
            && genesis.first_generation == generation
        {
            let creator = genesis.creator.id();
            return Some(
                membership::open_model_id(key, self.space, creator, &genesis.model_id).is_some(),
            );
        }
        let mut entries = self
            .chains
            .iter()
            .filter(|(_, chain)| chain.generation == generation)
            .filter_map(|(_, chain)| chain.previous.first())
            .peekable();
        entries.peek()?;
        Some(entries.any(|previous| {
            membership::open_previous_key(
                key,
                self.space,
                generation,
                previous.generation,
                &previous.key,
            )
            .is_some()
        }))
    }

    /// The genesis entry of the space, once the checks hold it.
    fn genesis(&self) -> Option<Genesis> {
        let genesis = self.controls.get(&self.resolved.genesis()?)?;
        match ControlBody::decode(&genesis.header().payload).ok()?.entry {
            Entry::Genesis(genesis) => Some(genesis),
            _ => None,
        }
    }

    /// The `model.id` the space's genesis names, opened with the space key
    /// of its first generation (RFC 0008, "What it names"). `None` while
    /// the checks hold no genesis or no key of the first generation, or
    /// when the sealed id does not open under that key.
    pub fn model_id(&self) -> Option<[u8; 16]> {
        let genesis = self.genesis()?;
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

/// A key envelope to the checks' member that holds a wrong key: a local
/// report (RFC 0008, "What the fold may read"). It names the envelope, the
/// author whose signature covers it and the generation, never a key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WrongEnvelope {
    /// The envelope's change id.
    pub change: ChangeId,
    /// The envelope's author.
    pub author: MemberId,
    /// The generation it names.
    pub generation: u64,
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

    fn space_model_id(&self) -> Option<[u8; 16]> {
        self.space_model
    }

    fn verify(&self, change: &Change) -> Result<(), Stop> {
        Ok(seal::verify(change, &self.author(change)?)?)
    }

    /// A control change's verdict is `resolve`'s; a sealed change's is
    /// RFC 0008's membership verdict.
    fn is_member(&self, change: &Change) -> bool {
        change.header().kind == Kind::CONTROL || self.resolved.admits(change.header())
    }

    /// With the generation's [`SealChecks::space_key`] only, never with
    /// another key the checks hold for it.
    fn open(&self, change: &Change) -> Result<Payload, Stop> {
        let author = self.author(change)?;
        let key = self
            .space_key(change.header().key_generation)
            .ok_or(Stop::Rejected(RejectReason::NoKey))?;
        Ok(seal::open(change, &author, key)?)
    }

    /// Without opening the change while its generation has had one key
    /// only, the one held now.
    fn still_opens(&self, change: &Change) -> Result<(), Stop> {
        let generation = change.header().key_generation;
        let replaced = self
            .held
            .get(&generation)
            .is_some_and(|keys| keys.len() > 1);
        if !replaced && self.space_key(generation).is_some() {
            return Ok(());
        }
        self.open(change).map(drop)
    }

    /// Exactly the changes [`Checks::is_member`] does not admit, or whose
    /// generation [`Checks::still_opens`] cannot pass without opening the
    /// change: its key was replaced, or is not held.
    fn may_unfold(&self, fields: &HeaderFields) -> bool {
        let generation = fields.key_generation;
        let replaced = self
            .held
            .get(&generation)
            .is_some_and(|keys| keys.len() > 1);
        !self.resolved.admits(fields) || replaced || self.space_key(generation).is_none()
    }

    /// With the key held now, else with each key held for the generation
    /// before.
    fn open_folded(&self, change: &Change) -> Result<Payload, Stop> {
        let stop = match self.open(change) {
            Ok(payload) => return Ok(payload),
            Err(stop) => stop,
        };
        let author = self.author(change)?;
        let held = self.held.get(&change.header().key_generation);
        held.into_iter()
            .flatten()
            .find_map(|key| seal::open(change, &author, key).ok())
            .ok_or(stop)
    }

    fn add_control(&mut self, change: &Change) {
        self.add_controls([change]);
    }

    fn add_controls(&mut self, changes: &[Change]) {
        SealChecks::add_controls(self, changes);
    }
}

impl SealChecks {
    /// Keeps a control change of the space the checks do not hold yet, and
    /// the key it names or opens; says whether it was new.
    fn take_control(&mut self, change: &Change) -> bool {
        if change.header().space != self.space || self.controls.contains_key(&change.id()) {
            return false;
        }
        if let Ok(body) = ControlBody::decode(&change.header().payload) {
            if let Some(key) = named_key(&body) {
                self.keys.insert(key.id(), key);
            }
            // An envelope to another member, or one that does not open,
            // gives no key.
            if let Entry::KeyChain(chain) = &body.entry {
                self.key_chain_entries.insert(change.id(), chain.clone());
            }
            if let Entry::KeyEnvelope(sealed) = &body.entry
                && let Ok(key) = envelope::open(sealed, self.space, &self.member)
            {
                self.envelope_keys
                    .entry(sealed.generation)
                    .or_default()
                    .insert(change.id(), (change.header().author, key));
            }
        }
        self.controls.insert(change.id(), change.clone());
        true
    }

    /// Resolves the control changes held, and derives the keys and the
    /// space's model from the result.
    fn settle(&mut self) {
        self.resolved = self.resolver.resolve(self.controls.values());
        self.resolves += 1;
        self.walk_key_chain();
        self.hold_keys();
        self.space_model = self.model_id();
    }

    /// Derives the key chain keys again from scratch: opens the previous
    /// keys of every key chain entry the generation graph holds whose own
    /// generation's key is known, in passes over the entries in change id
    /// order, until a pass gives no new key. An entry opens under its
    /// generation's [`SealChecks::space_key`] only. A previous generation that an envelope gave a key for, or that an earlier entry
    /// or pass filled, keeps that key. The result depends only on the
    /// envelope keys and the set of entries, never on the order in which
    /// they arrived.
    fn walk_key_chain(&mut self) {
        self.chain_keys.clear();
        let chains: Vec<(ChangeId, KeyChainEntry)> = self
            .key_chain_entries
            .iter()
            .map(|(&id, chain)| (id, chain.clone()))
            .filter(|(id, chain)| {
                self.resolved
                    .generation(chain.generation)
                    .is_some_and(|generation| generation.entries.contains_key(id))
            })
            .collect();
        // The walk confirms each generation's key against its entries.
        self.chains = chains.clone();
        loop {
            let mut opened = false;
            for (_, chain) in &chains {
                let Some(new_key) = self.space_key(chain.generation) else {
                    continue;
                };
                let found: Vec<(u64, SpaceKey)> = chain
                    .previous
                    .iter()
                    .filter(|previous| self.keys_of(previous.generation).is_empty())
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

impl SealChecks {
    /// Adds every key the checks hold now to the keys held for its
    /// generation, once.
    fn hold_keys(&mut self) {
        let generations: BTreeSet<u64> = self
            .envelope_keys
            .keys()
            .chain(self.chain_keys.keys())
            .copied()
            .collect();
        let now: Vec<(u64, SpaceKey)> = generations
            .into_iter()
            .flat_map(|generation| {
                self.keys_of(generation)
                    .into_iter()
                    .map(move |key| (generation, SpaceKey::from_bytes(*key.bytes())))
            })
            .collect();
        for (generation, key) in now {
            let held = self.held.entry(generation).or_default();
            if !held.iter().any(|k| k.bytes() == key.bytes()) {
                held.push(key);
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
