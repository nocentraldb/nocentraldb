//! The control body of RFC 0008 ("The control bodies"): the payload of a
//! kind 1 change, a canonical CBOR map the relay reads.
//!
//! ```text
//! 1 entry type   3 control deps   8 ... the fields of the entry type
//! ```
//!
//! Key 2 (a clock) and keys 4 to 7 are reserved; keys 8 to 63 are "must
//! understand", keys from 64 up "may ignore", and kept to be encoded again.
//! This module reads the genesis (entry type 0), the membership entry
//! (entry type 1), the key envelope (entry type 2, [`KeyEnvelope`]) and the
//! key chain entry (entry type 3, [`KeyChainEntry`]). The bodies of entry
//! types 4 to 6 are other tickets'; it keeps their fields as they are. A
//! higher entry type, or a key this core does not understand, makes the
//! body [`BodyError::NotUnderstood`]: the change is kept, parked.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::cbor::{self, Value};
use crate::change::{Change, ChangeError, Header, Kind, LAST_MUST_UNDERSTAND_KEY};
use crate::envelope::{EnvelopeError, KeyEnvelope};
use crate::ids::{ChangeId, MemberId, SpaceId};
use crate::keys::{MemberKey, MemberPublicKey};
use crate::seal;

use super::certificate::Certificate;
use super::fields::{self, Fields};
use super::genesis::Genesis;
use super::key_chain::KeyChainEntry;

/// The depth of a control body in its change: inside the header's key 6.
const BODY_DEPTH: usize = 3;

/// Entry type 0: the genesis.
pub const GENESIS: u64 = 0;
/// Entry type 1: a membership entry.
pub const MEMBERSHIP: u64 = 1;
/// Entry type 2: a key envelope.
pub const KEY_ENVELOPE: u64 = 2;
/// Entry type 3: a key chain entry.
pub const KEY_CHAIN: u64 = 3;
/// Entry type 4: a person-key entry.
pub const PERSON_KEY: u64 = 4;
/// Entry type 5: a shred entry.
pub const SHRED: u64 = 5;
/// Entry type 6: an acknowledgement.
pub const ACKNOWLEDGEMENT: u64 = 6;

/// A role in one space (RFC 0008, "Roles and rights"), ordered by its
/// rights: every right of a reader is a writer's, every right of a writer
/// an admin's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Role {
    /// Reads the whole space and acknowledges shreds: 1.
    Reader,
    /// Also writes content, person keys and shreds: 2.
    Writer,
    /// Also changes membership and rotates the space key: 3.
    Admin,
}

impl Role {
    /// The number of the role in a membership entry.
    pub fn number(self) -> u64 {
        match self {
            Role::Reader => 1,
            Role::Writer => 2,
            Role::Admin => 3,
        }
    }

    fn from_number(number: u64) -> Option<Role> {
        match number {
            1 => Some(Role::Reader),
            2 => Some(Role::Writer),
            3 => Some(Role::Admin),
            _ => None,
        }
    }
}

/// A control body.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ControlBody {
    /// Key 3: the heads of the author's control DAG when it wrote, sorted.
    pub dependencies: BTreeSet<ChangeId>,
    /// Key 1 and keys 8 to 63: what the body says.
    pub entry: Entry,
    /// Keys from 64 up, which this core may ignore.
    extensions: BTreeMap<u64, Value>,
}

/// What a control body says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Entry {
    /// Entry type 0.
    Genesis(Genesis),
    /// Entry type 1.
    Membership(MembershipEntry),
    /// Entry type 2: one generation's space key, wrapped to one member.
    KeyEnvelope(KeyEnvelope),
    /// Entry type 3: a rotation to a new generation.
    KeyChain(KeyChainEntry),
    /// Entry types 4 to 6, which this module does not read: their fields
    /// from key 8 up, as they are.
    Other {
        /// The entry type, 4 to 6.
        entry_type: u64,
        /// The fields of the entry type.
        fields: BTreeMap<u64, Value>,
    },
}

/// A membership entry (entry type 1): one action about one member.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MembershipEntry {
    /// Key 9: the member the entry is about.
    pub subject: MemberId,
    /// Key 8 and the keys the action needs.
    pub action: Action,
}

/// The action of a membership entry (key 8).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// 1: gives a role to a member that is not one.
    Add {
        /// Key 10.
        role: Role,
        /// Keys 11 and 12: the subject's public keys, which hash to its id.
        key: MemberPublicKey,
        /// Key 13.
        certificate: Box<Certificate>,
        /// Key 14: from which generation the subject's sealed changes count.
        from_generation: u64,
    },
    /// 2: a grant of the role, and a revocation of every right above it
    /// that it did not see.
    SetRole {
        /// Key 10.
        role: Role,
        /// Key 14: present exactly when the role writes.
        from_generation: Option<u64>,
        /// Key 15: the subject's highest author sequence the author held.
        cutoff: u64,
    },
    /// 3: a revocation of every right.
    Remove {
        /// Key 15.
        cutoff: u64,
    },
}

/// Why a control body cannot be folded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyError {
    /// Not a body of the format: rejected.
    Malformed(ChangeError),
    /// A higher entry type, or a "must understand" key this core does not
    /// know: kept, parked, as RFC 0002 keeps what it does not understand.
    NotUnderstood,
}

impl From<ChangeError> for BodyError {
    fn from(error: ChangeError) -> BodyError {
        BodyError::Malformed(error)
    }
}

impl From<cbor::DecodeError> for BodyError {
    fn from(error: cbor::DecodeError) -> BodyError {
        BodyError::Malformed(error.into())
    }
}

impl fmt::Display for BodyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BodyError::Malformed(error) => write!(f, "malformed control body: {error}"),
            BodyError::NotUnderstood => f.write_str("a control body this core does not understand"),
        }
    }
}

impl std::error::Error for BodyError {}

impl ControlBody {
    /// A body without keys from 64 up.
    pub fn new(dependencies: BTreeSet<ChangeId>, entry: Entry) -> ControlBody {
        ControlBody {
            dependencies,
            entry,
            extensions: BTreeMap::new(),
        }
    }

    /// The entry type (key 1).
    pub fn entry_type(&self) -> u64 {
        match &self.entry {
            Entry::Genesis(_) => GENESIS,
            Entry::Membership(_) => MEMBERSHIP,
            Entry::KeyEnvelope(_) => KEY_ENVELOPE,
            Entry::KeyChain(_) => KEY_CHAIN,
            Entry::Other { entry_type, .. } => *entry_type,
        }
    }

    /// The keys from 64 up, in key order.
    pub fn extensions(&self) -> impl Iterator<Item = (u64, &Value)> {
        self.extensions.iter().map(|(key, value)| (*key, value))
    }

    /// Adds a "may ignore" key, as a newer client would write it. Returns
    /// `false`, and changes nothing, for a key below 64.
    pub fn insert_extension(&mut self, key: u64, value: Value) -> bool {
        if key <= LAST_MUST_UNDERSTAND_KEY {
            return false;
        }
        self.extensions.insert(key, value);
        true
    }

    /// The canonical bytes of the body.
    pub fn encode(&self) -> Vec<u8> {
        let ids = self.dependencies.iter().map(|id| fields::bytes(&id.0));
        let mut entries = vec![
            (1, Value::Unsigned(self.entry_type())),
            (3, Value::Array(ids.collect())),
        ];
        match &self.entry {
            Entry::Genesis(genesis) => entries.extend(genesis.fields()),
            Entry::Membership(entry) => entries.extend(entry.fields()),
            Entry::KeyEnvelope(envelope) => entries.extend(
                envelope
                    .fields()
                    .into_iter()
                    .filter_map(|(key, value)| match key {
                        cbor::Key::Unsigned(key) => Some((key, value)),
                        cbor::Key::Text(_) => None,
                    }),
            ),
            Entry::KeyChain(chain) => entries.extend(chain.fields()),
            Entry::Other { fields, .. } => entries.extend(fields.clone()),
        }
        entries.extend(self.extensions.clone());
        cbor::encode(&fields::map(entries))
    }

    /// Decodes the canonical bytes of a body: the payload of a control
    /// change.
    pub fn decode(bytes: &[u8]) -> Result<ControlBody, BodyError> {
        let mut map = Fields::of(cbor::decode_at(bytes, BODY_DEPTH)?)?;
        let entry_type = fields::unsigned(map.required(1)?)?;
        let dependencies = fields::change_ids(map.required(3)?)?;
        let extensions = map.0.split_off(&(LAST_MUST_UNDERSTAND_KEY + 1));
        let entry = match entry_type {
            GENESIS => Entry::Genesis(Genesis::take(&mut map)?),
            MEMBERSHIP => Entry::Membership(MembershipEntry::take(&mut map)?),
            KEY_ENVELOPE => Entry::KeyEnvelope(take_envelope(&mut map)?),
            KEY_CHAIN..=ACKNOWLEDGEMENT => Entry::Other {
                entry_type,
                fields: map.0.split_off(&8),
            },
            _ => return Err(BodyError::NotUnderstood),
        };
        // What is left is key 0, 2, 4 to 7, or a key 8 to 63 the entry type
        // does not define: reserved or newer, so not understood.
        if !map.0.is_empty() {
            return Err(BodyError::NotUnderstood);
        }
        if entry_type == GENESIS && !dependencies.is_empty() {
            return Err(BodyError::Malformed(ChangeError::InvalidValue(3)));
        }
        if entry_type != GENESIS && dependencies.is_empty() {
            return Err(BodyError::Malformed(ChangeError::MissingKey(3)));
        }
        Ok(ControlBody {
            dependencies,
            entry,
            extensions,
        })
    }
}

impl MembershipEntry {
    /// The role the entry sets: its role, or none for a remove.
    pub fn role(&self) -> Option<Role> {
        match &self.action {
            Action::Add { role, .. } | Action::SetRole { role, .. } => Some(*role),
            Action::Remove { .. } => None,
        }
    }

    /// The cutoff of a set role or a remove.
    pub fn cutoff(&self) -> Option<u64> {
        match &self.action {
            Action::Add { .. } => None,
            Action::SetRole { cutoff, .. } | Action::Remove { cutoff } => Some(*cutoff),
        }
    }

    fn fields(&self) -> Vec<(u64, Value)> {
        let subject = fields::bytes(&self.subject.0);
        match &self.action {
            Action::Add {
                role,
                key,
                certificate,
                from_generation,
            } => vec![
                (8, Value::Unsigned(1)),
                (9, subject),
                (10, Value::Unsigned(role.number())),
                (11, fields::bytes(&key.signing)),
                (12, fields::bytes(&key.agreement)),
                (13, certificate.to_value()),
                (14, Value::Unsigned(*from_generation)),
            ],
            Action::SetRole {
                role,
                from_generation,
                cutoff,
            } => {
                let mut entries = vec![
                    (8, Value::Unsigned(2)),
                    (9, subject),
                    (10, Value::Unsigned(role.number())),
                ];
                entries.extend(from_generation.map(|g| (14, Value::Unsigned(g))));
                entries.push((15, Value::Unsigned(*cutoff)));
                entries
            }
            Action::Remove { cutoff } => vec![
                (8, Value::Unsigned(3)),
                (9, subject),
                (15, Value::Unsigned(*cutoff)),
            ],
        }
    }

    /// Takes the keys of the action out of the map. A key the action does
    /// not use stays in it, and the body is then not understood.
    fn take(map: &mut Fields) -> Result<MembershipEntry, ChangeError> {
        let action = fields::unsigned(map.required(8)?)?;
        let subject = MemberId(fields::fixed(map.required(9)?)?);
        let mut role = || -> Result<Role, ChangeError> {
            Role::from_number(fields::unsigned(map.required(10)?)?)
                .ok_or(ChangeError::InvalidValue(10))
        };
        let action = match action {
            1 => {
                let role = role()?;
                let key = MemberPublicKey {
                    signing: fields::fixed(map.required(11)?)?,
                    agreement: fields::fixed(map.required(12)?)?,
                };
                if key.id() != subject {
                    return Err(ChangeError::InvalidValue(11));
                }
                let certificate = Box::new(
                    Certificate::from_value(map.required(13)?.1)
                        .map_err(|_| ChangeError::InvalidValue(13))?,
                );
                let from_generation = fields::unsigned(map.required(14)?)?;
                map.absent(15)?;
                Action::Add {
                    role,
                    key,
                    certificate,
                    from_generation,
                }
            }
            2 => {
                let role = role()?;
                let from_generation = match role {
                    Role::Reader => {
                        map.absent(14)?;
                        None
                    }
                    Role::Writer | Role::Admin => Some(fields::unsigned(map.required(14)?)?),
                };
                let cutoff = fields::unsigned(map.required(15)?)?;
                (11..=13).try_for_each(|key| map.absent(key))?;
                Action::SetRole {
                    role,
                    from_generation,
                    cutoff,
                }
            }
            3 => {
                let cutoff = fields::unsigned(map.required(15)?)?;
                (10..=14).try_for_each(|key| map.absent(key))?;
                Action::Remove { cutoff }
            }
            _ => return Err(ChangeError::InvalidValue(8)),
        };
        Ok(MembershipEntry { subject, action })
    }
}

/// Takes keys 8 to 11 of an envelope out of the map; a missing or
/// mistyped one is malformed.
fn take_envelope(map: &mut Fields) -> Result<KeyEnvelope, ChangeError> {
    let fields = (8..=11)
        .filter_map(|key| map.optional(key))
        .map(|(key, value)| (cbor::Key::Unsigned(key), value))
        .collect();
    KeyEnvelope::from_fields(&fields).map_err(|error| match error {
        EnvelopeError::Malformed(key) => ChangeError::InvalidValue(key),
        // Reading the fields fails only as malformed; the recipient stands
        // for the others.
        _ => ChangeError::InvalidValue(8),
    })
}

/// Writes a control change: the body as the payload of a kind 1 header of
/// suite 1, signed by the author.
pub fn write(
    author: &MemberKey,
    space: SpaceId,
    sequence: u64,
    key_generation: u64,
    body: &ControlBody,
) -> Change {
    let header = Header::new(
        space,
        author.id(),
        sequence,
        key_generation,
        Kind::CONTROL,
        body.encode(),
        seal::SUITE,
    );
    seal::sign(author, header)
}
