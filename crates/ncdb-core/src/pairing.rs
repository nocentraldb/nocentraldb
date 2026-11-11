//! The account key in a pairing answer, as RFC 0003 defines it ("Pairing
//! answer"): when a device joins an account, the answer carries the
//! account key sealed with HPKE to the new device's X25519 key.
//!
//! It is the construction of a key envelope ([`crate::envelope`]): RFC
//! 0003's suite, base mode, no associated data, the 32-byte secret as the
//! plaintext. Only the HPKE `info` differs: the label
//! `"nocentraldb/account-key"` alone, with no space id, generation or
//! recipient member id after it, because an account key belongs to no space
//! and is sealed to one device's key. The label keeps an account key from
//! ever being opened as a space key. The plaintext is the account key's
//! 32-byte Ed25519 secret.
//!
//! The rest of the pairing answer, the certificate and the account key
//! statement it carries, comes with RFC 0010's build.

use ed25519_dalek::SigningKey;

use crate::envelope::{self, EnvelopeError};
use crate::keys::{self, MemberKey, MemberPublicKey};

/// The HPKE `info` of a sealed account key: this label alone.
pub const ACCOUNT_KEY_CONTEXT: &[u8] = b"nocentraldb/account-key";

/// An account key sealed to one device: HPKE's encapsulated key and the
/// ciphertext of the 32-byte account key with its 16-byte tag.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SealedAccountKey {
    /// HPKE's encapsulated key: an X25519 public key, 32 bytes.
    pub encapsulated_key: Vec<u8>,
    /// The HPKE ciphertext, 48 bytes.
    pub ciphertext: Vec<u8>,
}

/// Seals the account key to the recipient's X25519 key. The ephemeral key
/// comes from the platform's secure generator.
///
/// # Panics
///
/// When the platform has no secure generator.
pub fn seal_account_key(
    account: &SigningKey,
    recipient: &MemberPublicKey,
) -> Result<SealedAccountKey, EnvelopeError> {
    let ikm = keys::random::<32>();
    seal_account_key_with_ephemeral(account, recipient, *ikm)
}

/// [`seal_account_key`] with the ephemeral key derived from `ikm` (RFC
/// 9180's ikmE), for test vectors. An ephemeral key must never be used
/// twice; outside test vectors, use [`seal_account_key`].
pub fn seal_account_key_with_ephemeral(
    account: &SigningKey,
    recipient: &MemberPublicKey,
    ikm: [u8; 32],
) -> Result<SealedAccountKey, EnvelopeError> {
    let secret = zeroize::Zeroizing::new(account.to_bytes());
    let (encapsulated_key, ciphertext) =
        envelope::seal_bytes(&secret, ACCOUNT_KEY_CONTEXT, recipient, ikm)?;
    Ok(SealedAccountKey {
        encapsulated_key,
        ciphertext,
    })
}

/// Opens an account key sealed to this member.
pub fn open_account_key(
    sealed: &SealedAccountKey,
    recipient: &MemberKey,
) -> Result<SigningKey, EnvelopeError> {
    let secret = envelope::open_bytes(
        &sealed.encapsulated_key,
        &sealed.ciphertext,
        ACCOUNT_KEY_CONTEXT,
        recipient,
    )?;
    Ok(SigningKey::from_bytes(&secret))
}
