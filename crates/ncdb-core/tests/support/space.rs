//! A space of simulated members that write real, signed control changes
//! (RFC 0008): a genesis, membership entries and opaque entries of the
//! other types. Each member is a small number; its key derives from it.
//!
//! Each test file includes this module with `#[path]`; not every file uses
//! every helper.
#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};

use ed25519_dalek::SigningKey;
use ncdb_core::cbor::Value;
use ncdb_core::change::{Change, Operation, Payload};
use ncdb_core::clock::Hlc;
use ncdb_core::envelope;
use ncdb_core::ids::{ChangeId, MemberId, SpaceId};
use ncdb_core::keys::{MemberKey, SpaceKey};
use ncdb_core::membership::{
    self, Action, Certificate, ControlBody, Entry, Genesis, MemberKind, MembershipEntry, Role,
};
use ncdb_core::seal;

/// A simulated member.
pub type Who = u8;

/// The member key of a simulated member.
pub fn key(who: Who) -> MemberKey {
    MemberKey::from_secrets([who; 32], [who ^ 0x80; 32])
}

/// The member id of a simulated member.
pub fn id(who: Who) -> MemberId {
    key(who).id()
}

/// The space key of the first generation of every simulated space.
pub fn space_key() -> SpaceKey {
    SpaceKey::from_bytes([3; 32])
}

/// The account every simulated member's certificate names.
pub fn account() -> SigningKey {
    SigningKey::from_bytes(&[0xAC; 32])
}

/// A space whose members write control changes. It counts each member's
/// author sequence, so that a change gets the next one.
pub struct Space {
    pub space: SpaceId,
    pub first_generation: u64,
    pub genesis: Change,
    sequences: BTreeMap<Who, u64>,
    kinds: BTreeMap<Who, MemberKind>,
}

impl Space {
    /// A space that `creator`, an agent, creates.
    pub fn new(creator: Who) -> Space {
        Space::with_nonce(creator, [7; 16])
    }

    /// A space from this nonce; two calls with the same creator and nonce
    /// give the same space id.
    pub fn with_nonce(creator: Who, nonce: [u8; 16]) -> Space {
        let (genesis, space, first_generation) = genesis(creator, nonce, [1; 16]);
        Space {
            space,
            first_generation,
            genesis,
            sequences: BTreeMap::from([(creator, 1)]),
            kinds: BTreeMap::from([(creator, MemberKind::Agent)]),
        }
    }

    /// The next author sequence of `who`.
    fn next(&mut self, who: Who) -> u64 {
        let sequence = self.sequences.entry(who).or_insert(0);
        *sequence += 1;
        *sequence
    }

    /// The highest sequence `who` has used.
    pub fn sequence(&self, who: Who) -> u64 {
        self.sequences.get(&who).copied().unwrap_or(0)
    }

    /// Lets `who` use up `n` sequences, as sealed changes would.
    pub fn skip(&mut self, who: Who, n: u64) {
        *self.sequences.entry(who).or_insert(0) += n;
    }

    /// A control change by `author` with this entry, after `deps`.
    pub fn write(&mut self, author: Who, entry: Entry, deps: &[&Change]) -> Change {
        let body = ControlBody::new(dependencies(deps), entry);
        let sequence = self.next(author);
        membership::write(
            &key(author),
            self.space,
            sequence,
            self.first_generation,
            &body,
        )
    }

    /// `author` adds `subject` with `role`. The subject is a device unless
    /// [`Space::agent`] made it an agent.
    pub fn add(&mut self, author: Who, subject: Who, role: Role, deps: &[&Change]) -> Change {
        let kind = *self.kinds.entry(subject).or_insert(MemberKind::Device);
        let certificate = Certificate::sign(&account(), id(subject), kind);
        self.add_certified(author, subject, role, certificate, deps)
    }

    /// `author` adds `subject` with this certificate.
    pub fn add_certified(
        &mut self,
        author: Who,
        subject: Who,
        role: Role,
        certificate: Certificate,
        deps: &[&Change],
    ) -> Change {
        let action = Action::Add {
            role,
            key: key(subject).public(),
            certificate: Box::new(certificate),
            from_generation: self.first_generation,
        };
        self.membership(author, subject, action, deps)
    }

    /// Marks `who` as an agent, for its certificate.
    pub fn agent(&mut self, who: Who) {
        self.kinds.insert(who, MemberKind::Agent);
    }

    /// `author` sets the role of `subject`, with this cutoff.
    pub fn set_role(
        &mut self,
        author: Who,
        subject: Who,
        role: Role,
        cutoff: u64,
        deps: &[&Change],
    ) -> Change {
        let from_generation = (role >= Role::Writer).then_some(self.first_generation);
        let action = Action::SetRole {
            role,
            from_generation,
            cutoff,
        };
        self.membership(author, subject, action, deps)
    }

    /// `author` removes `subject`, with this cutoff.
    pub fn remove(&mut self, author: Who, subject: Who, cutoff: u64, deps: &[&Change]) -> Change {
        self.membership(author, subject, Action::Remove { cutoff }, deps)
    }

    fn membership(
        &mut self,
        author: Who,
        subject: Who,
        action: Action,
        deps: &[&Change],
    ) -> Change {
        let entry = Entry::Membership(MembershipEntry {
            subject: id(subject),
            action,
        });
        self.write(author, entry, deps)
    }

    /// `author` wraps the first generation's space key to `recipient`, in a
    /// key envelope. The ephemeral key derives from the two members, so the
    /// same call gives the same change.
    pub fn envelope(&mut self, author: Who, recipient: Who, deps: &[&Change]) -> Change {
        let sealed = envelope::seal_with_ephemeral(
            &space_key(),
            self.space,
            self.first_generation,
            &key(recipient).public(),
            [author ^ (recipient << 4) ^ 0x5e; 32],
        )
        .expect("a simulated member's key takes an envelope");
        self.write(author, Entry::KeyEnvelope(sealed), deps)
    }

    /// A sealed change by `author` under `generation`, with these
    /// operations, at its next author sequence.
    pub fn sealed_under(
        &mut self,
        author: Who,
        generation: u64,
        operations: Vec<Operation>,
    ) -> Change {
        let sequence = self.next(author);
        let payload = Payload::new(Hlc(sequence << 16), BTreeSet::new(), operations);
        seal::seal(
            &key(author),
            &space_key(),
            self.space,
            sequence,
            generation,
            &payload,
        )
    }

    /// A sealed change by `author` under the first generation.
    pub fn sealed(&mut self, author: Who, operations: Vec<Operation>) -> Change {
        self.sealed_under(author, self.first_generation, operations)
    }

    /// A control change of another entry type, with one opaque field.
    pub fn other(&mut self, author: Who, entry_type: u64, deps: &[&Change]) -> Change {
        let fields = BTreeMap::from([(8, Value::Unsigned(u64::from(author)))]);
        self.write(author, Entry::Other { entry_type, fields }, deps)
    }
}

/// The ids of these changes.
pub fn dependencies(deps: &[&Change]) -> BTreeSet<ChangeId> {
    deps.iter().map(|change| change.id()).collect()
}

/// A genesis by `creator`, an agent, with this nonce and model id: the
/// change, its space id and its first generation.
pub fn genesis(creator: Who, nonce: [u8; 16], model_id: [u8; 16]) -> (Change, SpaceId, u64) {
    let creator_key = key(creator);
    let space = membership::space_id(creator_key.id(), &nonce);
    let first_generation = membership::generation_id(space, creator_key.id(), &[]);
    let field = membership::seal_model_id(&space_key(), space, creator_key.id(), model_id);
    let genesis = Genesis {
        nonce,
        creator: creator_key.public(),
        certificate: Certificate::sign(&account(), creator_key.id(), MemberKind::Agent),
        first_generation,
        model_id: field,
    };
    let body = ControlBody::new(BTreeSet::new(), Entry::Genesis(genesis));
    let change = membership::write(&creator_key, space, 1, first_generation, &body);
    (change, space, first_generation)
}
