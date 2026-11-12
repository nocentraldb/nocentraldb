//! The member key, the recovery key and the space key, as RFC 0003 defines
//! them ("The keys").
//!
//! A member key is two keys: an Ed25519 signing key, which signs every
//! change the member writes, and an X25519 key-agreement key, to which
//! other members wrap space keys. The member id is the SHA-256 hash of
//! `"nocentraldb/member-id"` and both public keys. The recovery key, the
//! account's identity, is derived from the recovery code when it is needed
//! (RFC 0003, "Recovery key"). A space key is 32 random
//! bytes per generation; members never use it directly but derive a subkey
//! per purpose from it (see [`crate::seal`]).
//!
//! Secret types print as `<redacted>`, have no `Display`, and wipe their
//! bytes from memory when dropped (RFC 0003, "Keys never reach logs, errors
//! or telemetry").

use std::fmt;

use curve25519_dalek::montgomery::MontgomeryPoint;
use ed25519_dalek::SigningKey;
use hkdf::Hkdf;
use sha2::{Digest, Sha256};
use zeroize::{ZeroizeOnDrop, Zeroizing};

use crate::ids::MemberId;
use crate::redact::{self, Hidden};

/// A member's private keys: the Ed25519 signing key and the X25519
/// key-agreement key. It never leaves the member.
pub struct MemberKey {
    signing: SigningKey,
    agreement: Zeroizing<[u8; 32]>,
}

/// The public halves of a member key: the member's public identity.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct MemberPublicKey {
    /// The Ed25519 public key, which verifies the member's signatures.
    pub signing: [u8; 32],
    /// The X25519 public key, to which space keys are wrapped.
    pub agreement: [u8; 32],
}

/// The account's recovery key: an Ed25519 key whose seed derives from the
/// recovery code's 128 bits. No device keeps it; it is derived when needed
/// and dropped, and it signs only account key statements.
pub struct RecoveryKey(SigningKey);

/// One generation of a space key: 32 random bytes.
pub struct SpaceKey(Zeroizing<[u8; 32]>);

/// A person key (RFC 0003, "The person key"): 32 random bytes, one per
/// person per space, which encrypts that person's personal fields. It lives
/// in the space's person-key store, wrapped under a generation of the space
/// key (`membership::wrap_person_key`), never in the log.
#[derive(Clone)]
pub struct PersonKey(Zeroizing<[u8; 32]>);

impl MemberKey {
    /// A new member key from the platform's secure random generator.
    ///
    /// # Panics
    ///
    /// When the platform has no secure generator; a key from a weaker
    /// source is no key.
    pub fn generate() -> MemberKey {
        MemberKey {
            signing: SigningKey::from_bytes(&random::<32>()),
            agreement: random(),
        }
    }

    /// The member key of these secret halves: the Ed25519 seed and the
    /// X25519 secret. The caller's copies of the bytes are not wiped.
    pub fn from_secrets(signing: [u8; 32], agreement: [u8; 32]) -> MemberKey {
        MemberKey {
            signing: SigningKey::from_bytes(&signing),
            agreement: Zeroizing::new(agreement),
        }
    }

    /// The public halves.
    pub fn public(&self) -> MemberPublicKey {
        MemberPublicKey {
            signing: self.signing.verifying_key().to_bytes(),
            agreement: MontgomeryPoint::mul_base_clamped(*self.agreement).to_bytes(),
        }
    }

    /// The member id of this key.
    pub fn id(&self) -> MemberId {
        self.public().id()
    }

    pub(crate) fn signing(&self) -> &SigningKey {
        &self.signing
    }

    pub(crate) fn agreement(&self) -> &[u8; 32] {
        &self.agreement
    }
}

impl MemberPublicKey {
    /// The context string in front of the hashed public keys.
    pub const ID_CONTEXT: &'static [u8] = b"nocentraldb/member-id";

    /// The member id: SHA-256 of `"nocentraldb/member-id"`, the Ed25519
    /// public key and the X25519 public key (RFC 0003, "The member key").
    pub fn id(&self) -> MemberId {
        let mut hash = Sha256::new();
        hash.update(MemberPublicKey::ID_CONTEXT);
        hash.update(self.signing);
        hash.update(self.agreement);
        MemberId(hash.finalize().into())
    }
}

impl RecoveryKey {
    /// The HKDF `info` of the recovery key's seed.
    pub const CONTEXT: &'static [u8] = b"nocentraldb/recovery-key";

    /// The recovery key of the recovery code's 128 bits: its Ed25519 seed is
    /// HKDF-SHA256(ikm = the 16 bytes, salt = none, info =
    /// `"nocentraldb/recovery-key"`), 32 bytes (RFC 0003, "Recovery key").
    /// The caller's copy of the bits is not wiped.
    pub fn from_code(code: &[u8; 16]) -> RecoveryKey {
        let mut seed = Zeroizing::new([0; 32]);
        Hkdf::<Sha256>::new(None, code)
            .expand(RecoveryKey::CONTEXT, seed.as_mut())
            .expect("32 bytes is a valid HKDF-SHA256 output length");
        RecoveryKey(SigningKey::from_bytes(&seed))
    }

    /// The Ed25519 public key: the account's identity.
    pub fn public(&self) -> [u8; 32] {
        self.0.verifying_key().to_bytes()
    }

    pub(crate) fn signing(&self) -> &SigningKey {
        &self.0
    }
}

impl SpaceKey {
    /// A new space key from the platform's secure random generator.
    ///
    /// # Panics
    ///
    /// When the platform has no secure generator.
    pub fn generate() -> SpaceKey {
        SpaceKey(random())
    }

    /// The space key of these bytes. The caller's copy is not wiped.
    pub fn from_bytes(bytes: [u8; 32]) -> SpaceKey {
        SpaceKey(Zeroizing::new(bytes))
    }

    pub(crate) fn bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl PersonKey {
    /// A new person key from the platform's secure random generator.
    ///
    /// # Panics
    ///
    /// When the platform has no secure generator.
    pub fn generate() -> PersonKey {
        PersonKey(random())
    }

    /// The person key of these bytes. The caller's copy is not wiped.
    pub fn from_bytes(bytes: [u8; 32]) -> PersonKey {
        PersonKey(Zeroizing::new(bytes))
    }

    /// The commitment to this key that every person-key entry for
    /// `person_key` in `space` carries (RFC 0003, "The person key").
    pub fn commitment(&self, space: crate::ids::SpaceId, person_key: [u8; 16]) -> [u8; 32] {
        crate::membership::person_key_commitment(space, person_key, &self.0)
    }

    pub(crate) fn bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

// The signing key wipes itself (ed25519-dalek's `zeroize` feature); the
// other secrets sit in `Zeroizing`.
impl ZeroizeOnDrop for MemberKey {}
impl ZeroizeOnDrop for RecoveryKey {}
impl ZeroizeOnDrop for SpaceKey {}
impl ZeroizeOnDrop for PersonKey {}

impl fmt::Debug for MemberKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("MemberKey").field(&Hidden).finish()
    }
}

impl fmt::Debug for RecoveryKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("RecoveryKey").field(&Hidden).finish()
    }
}

impl fmt::Debug for SpaceKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("SpaceKey").field(&Hidden).finish()
    }
}

impl fmt::Debug for PersonKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("PersonKey").field(&Hidden).finish()
    }
}

impl fmt::Debug for MemberPublicKey {
    /// Public keys are not secret; they show as hex.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("MemberPublicKey(")?;
        redact::hex(f, &self.signing)?;
        f.write_str(", ")?;
        redact::hex(f, &self.agreement)?;
        f.write_str(")")
    }
}

/// Random bytes from the platform's secure generator.
///
/// # Panics
///
/// When the platform has no secure generator.
pub(crate) fn random<const N: usize>() -> Zeroizing<[u8; N]> {
    let mut bytes = Zeroizing::new([0; N]);
    getrandom::fill(bytes.as_mut()).expect("the platform's secure random generator failed");
    bytes
}
