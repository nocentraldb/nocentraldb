//! The member key and the space key, as RFC 0003 defines them ("The keys").
//!
//! A member key is two keys: an Ed25519 signing key, which signs every
//! change the member writes, and an X25519 key-agreement key, to which
//! other members wrap space keys. The member id is the SHA-256 hash of
//! `"nocentraldb/member-id"` and both public keys. A space key is 32 random
//! bytes per generation; members never use it directly but derive a subkey
//! per purpose from it (see [`crate::seal`]).
//!
//! Secret types print as `<redacted>`, have no `Display`, and wipe their
//! bytes from memory when dropped (RFC 0003, "Keys never reach logs, errors
//! or telemetry").

use std::fmt;

use curve25519_dalek::montgomery::MontgomeryPoint;
use ed25519_dalek::SigningKey;
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

/// One generation of a space key: 32 random bytes.
pub struct SpaceKey(Zeroizing<[u8; 32]>);

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

// The signing key wipes itself (ed25519-dalek's `zeroize` feature); the
// other secrets sit in `Zeroizing`.
impl ZeroizeOnDrop for MemberKey {}
impl ZeroizeOnDrop for SpaceKey {}

impl fmt::Debug for MemberKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("MemberKey").field(&Hidden).finish()
    }
}

impl fmt::Debug for SpaceKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("SpaceKey").field(&Hidden).finish()
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
