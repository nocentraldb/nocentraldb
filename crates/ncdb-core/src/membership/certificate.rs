//! The member certificate of RFC 0003 ("Member certificate"): an account
//! vouches for a device or an agent, and a parent agent for a sub-agent.
//!
//! The signer signs `H(context || canonical map of keys 1 to 4)`, with the
//! context `"nocentraldb/member-cert"` for a device or an agent, signed by
//! the account key, and `"nocentraldb/sub-agent-cert"` for a sub-agent,
//! signed by its parent's member signing key. Key 1 of a sub-agent's
//! certificate still carries its parent's account key.

use std::fmt;

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use sha2::{Digest, Sha256};

use crate::cbor::{self, Value};
use crate::change::ChangeError;
use crate::ids::MemberId;
use crate::keys::MemberKey;
use crate::redact;

use super::fields::{self, Fields};

/// What a certified member is (RFC 0003, key 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MemberKind {
    /// A person's device: 1.
    Device,
    /// An agent: 2.
    Agent,
    /// A sub-agent, certified by its parent agent: 3.
    SubAgent,
}

impl MemberKind {
    fn number(self) -> u64 {
        match self {
            MemberKind::Device => 1,
            MemberKind::Agent => 2,
            MemberKind::SubAgent => 3,
        }
    }

    fn from_number(number: u64) -> Option<MemberKind> {
        match number {
            1 => Some(MemberKind::Device),
            2 => Some(MemberKind::Agent),
            3 => Some(MemberKind::SubAgent),
            _ => None,
        }
    }
}

/// A member certificate. Public keys and ids are not secret, so its `Debug`
/// output shows them as hex.
#[derive(Clone, PartialEq, Eq)]
pub struct Certificate {
    /// Key 1: the account's Ed25519 public key; a sub-agent's parent's.
    pub account_key: [u8; 32],
    /// Key 2: the certified member.
    pub member: MemberId,
    /// Key 3: device, agent or sub-agent.
    pub kind: MemberKind,
    /// Key 4: the parent member, for a sub-agent only.
    pub parent: Option<MemberId>,
    /// Key 5: the Ed25519 signature.
    pub signature: [u8; 64],
}

impl Certificate {
    /// The context of a device's or an agent's certificate.
    pub const MEMBER_CONTEXT: &'static [u8] = b"nocentraldb/member-cert";
    /// The context of a sub-agent's certificate.
    pub const SUB_AGENT_CONTEXT: &'static [u8] = b"nocentraldb/sub-agent-cert";

    /// A device's or an agent's certificate, signed by the account key.
    ///
    /// # Panics
    ///
    /// When `kind` is [`MemberKind::SubAgent`]: its parent signs it, with
    /// [`Certificate::sign_sub_agent`].
    pub fn sign(account: &SigningKey, member: MemberId, kind: MemberKind) -> Certificate {
        assert_ne!(kind, MemberKind::SubAgent, "a sub-agent's parent signs it");
        let mut certificate = Certificate {
            account_key: account.verifying_key().to_bytes(),
            member,
            kind,
            parent: None,
            signature: [0; 64],
        };
        certificate.signature = account.sign(&certificate.digest()).to_bytes();
        certificate
    }

    /// A sub-agent's certificate, signed by its parent's member key. The
    /// account key is the parent's.
    pub fn sign_sub_agent(
        parent: &MemberKey,
        account_key: [u8; 32],
        member: MemberId,
    ) -> Certificate {
        let mut certificate = Certificate {
            account_key,
            member,
            kind: MemberKind::SubAgent,
            parent: Some(parent.id()),
            signature: [0; 64],
        };
        certificate.signature = parent.signing().sign(&certificate.digest()).to_bytes();
        certificate
    }

    /// What the signer signs: the SHA-256 hash of the context of the kind
    /// and the canonical map of keys 1 to 4.
    pub fn digest(&self) -> [u8; 32] {
        let context = match self.kind {
            MemberKind::SubAgent => Certificate::SUB_AGENT_CONTEXT,
            MemberKind::Device | MemberKind::Agent => Certificate::MEMBER_CONTEXT,
        };
        let mut hash = Sha256::new();
        hash.update(context);
        hash.update(cbor::encode(&fields::map(self.unsigned_fields())));
        hash.finalize().into()
    }

    /// Whether the signature verifies, strictly, with `signer`: the account
    /// key for a device or an agent, the parent's signing key for a
    /// sub-agent. Checking that the signer is the right one is the caller's.
    pub fn verify(&self, signer: &[u8; 32]) -> bool {
        let Ok(key) = VerifyingKey::from_bytes(signer) else {
            return false;
        };
        key.verify_strict(&self.digest(), &Signature::from_bytes(&self.signature))
            .is_ok()
    }

    /// The canonical bytes of the certificate map.
    pub fn encode(&self) -> Vec<u8> {
        cbor::encode(&self.to_value())
    }

    /// Decodes canonical bytes of a certificate map.
    pub fn decode(bytes: &[u8]) -> Result<Certificate, ChangeError> {
        Certificate::from_value(cbor::decode(bytes)?)
    }

    fn unsigned_fields(&self) -> Vec<(u64, Value)> {
        let mut entries = vec![
            (1, fields::bytes(&self.account_key)),
            (2, fields::bytes(&self.member.0)),
            (3, Value::Unsigned(self.kind.number())),
        ];
        if let Some(parent) = self.parent {
            entries.push((4, fields::bytes(&parent.0)));
        }
        entries
    }

    pub(crate) fn to_value(&self) -> Value {
        let mut entries = self.unsigned_fields();
        entries.push((5, fields::bytes(&self.signature)));
        fields::map(entries)
    }

    /// A certificate from its map. Every key is required by its kind, and
    /// no other key may appear: RFC 0003 defines no extensions for it.
    pub(crate) fn from_value(value: Value) -> Result<Certificate, ChangeError> {
        let mut map = Fields::of(value)?;
        let account_key = fields::fixed(map.required(1)?)?;
        let member = MemberId(fields::fixed(map.required(2)?)?);
        let kind = MemberKind::from_number(fields::unsigned(map.required(3)?)?)
            .ok_or(ChangeError::InvalidValue(3))?;
        let parent = match kind {
            MemberKind::SubAgent => Some(MemberId(fields::fixed(map.required(4)?)?)),
            MemberKind::Device | MemberKind::Agent => {
                map.absent(4)?;
                None
            }
        };
        let signature = fields::fixed(map.required(5)?)?;
        if let Some(key) = map.0.keys().next() {
            return Err(ChangeError::InvalidValue(*key));
        }
        Ok(Certificate {
            account_key,
            member,
            kind,
            parent,
            signature,
        })
    }
}

impl fmt::Debug for Certificate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        struct Hex<'a>(&'a [u8]);
        impl fmt::Debug for Hex<'_> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                redact::hex(f, self.0)
            }
        }
        f.debug_struct("Certificate")
            .field("account_key", &Hex(&self.account_key))
            .field("member", &self.member)
            .field("kind", &self.kind)
            .field("parent", &self.parent)
            .finish_non_exhaustive()
    }
}
