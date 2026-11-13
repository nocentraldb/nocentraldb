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
//! The whole answer is [`PairingAnswer`] (RFC 0010, "The pairing answer";
//! the owner's NOC-273 A, Carsten, 2026-11-12): a device of the account
//! builds it, and the new member reads it from its text and completes it.
//! It carries the certificate the account key signed, the account key
//! statement, for a device the sealed account key, and when the new member
//! had an account of its own, the account key's half of the link statement
//! (RFC 0010, "The link statement").

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use data_encoding::BASE64URL_NOPAD;
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use sha2::{Digest, Sha256};

use crate::cbor::{self, Key, Value};
use crate::change::ChangeError;
use crate::envelope::{self, EnvelopeError};
use crate::keys::{self, MemberKey, MemberPublicKey};
use crate::membership::{AccountKeyStatement, Certificate, MemberKind};

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

/// What the text of every pairing answer starts with.
pub const ANSWER_PREFIX: &str = "ncdb-pair-1:";

/// The context in front of what both keys of a link statement sign.
pub const LINK_CONTEXT: &[u8] = b"nocentraldb/account-link";

/// The layout's version byte.
const VERSION: u8 = 1;
/// Where the sealed key byte stands, and the lengths of the sealed key.
const SEALED_AT: usize = 202;
const SEALED_LEN: usize = 32 + 48;

/// What both keys of a link statement sign: the SHA-256 hash of
/// `"nocentraldb/account-link"` and the canonical map of key 1, the old
/// account key, and key 2, the recovery key of the account it joins (RFC
/// 0010, "The link statement").
pub fn link_digest(old_account_key: &[u8; 32], recovery_key: &[u8; 32]) -> [u8; 32] {
    let map = Value::Map(BTreeMap::from([
        (Key::Unsigned(1), Value::Bytes(old_account_key.to_vec())),
        (Key::Unsigned(2), Value::Bytes(recovery_key.to_vec())),
    ]));
    let mut hash = Sha256::new();
    hash.update(LINK_CONTEXT);
    hash.update(cbor::encode(&map));
    hash.finalize().into()
}

/// A pairing answer: what a device of the account hands a new device or an
/// agent, as text, `ncdb-pair-1:` and unpadded base64url of RFC 0010's
/// fixed layout. The new member rebuilds what the layout leaves out: its
/// member id, from its own keys, and the statement's account key, from the
/// certificate's.
///
/// Its `Debug` output shows the kind and which parts it carries, never a
/// key or a signature.
#[derive(Clone, PartialEq, Eq)]
pub struct PairingAnswer {
    kind: MemberKind,
    account_key: [u8; 32],
    certificate_signature: [u8; 64],
    recovery_key: [u8; 32],
    number: u64,
    statement_signature: [u8; 64],
    sealed: Option<SealedAccountKey>,
    link: Option<[u8; 64]>,
}

/// Why an answer could not be built.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BuildError {
    /// An answer is for a device or an agent; a sub-agent's parent
    /// certifies it.
    Kind,
    /// The statement does not name the account key that signs.
    Statement,
    /// The account key could not be sealed to the new device.
    Seal(EnvelopeError),
}

/// Why a text is not a pairing answer. Nothing is read from such a text;
/// the SDK refuses it with `invalidValue`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadError {
    /// It does not start with `ncdb-pair-1:`.
    Prefix,
    /// The rest is not unpadded base64url.
    Encoding,
    /// The version is not 1.
    Version,
    /// The kind is not 1, a device, or 2, an agent.
    Kind,
    /// The sealed key byte does not match the kind: 1 for a device, 0 for
    /// an agent.
    SealedKey,
    /// The account key number is 0.
    Number,
    /// The link byte is neither 0 nor 1.
    Link,
    /// The length is not the one the bytes give.
    Length,
}

/// Why completing an answer failed: it does not hold for this member. The
/// SDK refuses it with `invalidCertificate`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckError {
    /// The statement does not verify against its recovery key.
    Statement,
    /// The certificate does not verify for this member against the account
    /// key the statement names: it is for another member, of another kind,
    /// or not signed by that key.
    Certificate,
    /// A device's account key does not open with this member's key, or is
    /// not the one the statement names.
    AccountKey,
    /// The link signature does not verify for this member's old account
    /// key, or the answer carries one this member does not expect, or
    /// lacks one it does.
    Link,
}

/// What a completed answer gives the new member.
pub struct Paired {
    /// Its certificate, with the account key statement as key 6.
    pub certificate: Certificate,
    /// For a device, the account key; an agent never holds it.
    pub account: Option<SigningKey>,
    /// The account key's half of the link statement, when the member had
    /// an account of its own: the account key's signature of
    /// [`link_digest`] over that old account key and the recovery key.
    pub link: Option<[u8; 64]>,
}

impl PairingAnswer {
    /// The answer for the member with the public key `member`, of `kind`, a
    /// device or an agent, signed by the account key `account` whose
    /// statement is `statement`. For a device, the account key is sealed to
    /// the member; the ephemeral key comes from the platform's secure
    /// generator. `old_account_key` is the account key of the certificate
    /// the member's invitation carried, if any: the answer then carries the
    /// account key's half of the link statement.
    ///
    /// # Panics
    ///
    /// For a device, when the platform has no secure generator.
    pub fn build(
        account: &SigningKey,
        statement: &AccountKeyStatement,
        member: &MemberPublicKey,
        kind: MemberKind,
        old_account_key: Option<[u8; 32]>,
    ) -> Result<PairingAnswer, BuildError> {
        let ikm = keys::random::<32>();
        PairingAnswer::build_with_ephemeral(account, statement, member, kind, old_account_key, *ikm)
    }

    /// [`PairingAnswer::build`] with the ephemeral key derived from `ikm`
    /// (RFC 9180's ikmE), for test vectors. An ephemeral key must never be
    /// used twice; outside test vectors, use [`PairingAnswer::build`].
    pub fn build_with_ephemeral(
        account: &SigningKey,
        statement: &AccountKeyStatement,
        member: &MemberPublicKey,
        kind: MemberKind,
        old_account_key: Option<[u8; 32]>,
        ikm: [u8; 32],
    ) -> Result<PairingAnswer, BuildError> {
        if kind == MemberKind::SubAgent {
            return Err(BuildError::Kind);
        }
        let account_key = account.verifying_key().to_bytes();
        if statement.account_key != account_key {
            return Err(BuildError::Statement);
        }
        let certificate = Certificate::sign(account, member.id(), kind);
        let sealed = match kind {
            MemberKind::Device => Some(
                seal_account_key_with_ephemeral(account, member, ikm).map_err(BuildError::Seal)?,
            ),
            MemberKind::Agent | MemberKind::SubAgent => None,
        };
        let link = old_account_key.map(|old| {
            account
                .sign(&link_digest(&old, &statement.recovery_key))
                .to_bytes()
        });
        Ok(PairingAnswer {
            kind,
            account_key,
            certificate_signature: certificate.signature,
            recovery_key: statement.recovery_key,
            number: statement.number,
            statement_signature: statement.signature,
            sealed,
            link,
        })
    }

    /// Whether the answer is for a device or an agent.
    pub fn kind(&self) -> MemberKind {
        self.kind
    }

    /// The answer's bytes, in RFC 0010's layout.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(348);
        bytes.push(VERSION);
        bytes.push(match self.kind {
            MemberKind::Device => 1,
            MemberKind::Agent | MemberKind::SubAgent => 2,
        });
        bytes.extend(self.account_key);
        bytes.extend(self.certificate_signature);
        bytes.extend(self.recovery_key);
        bytes.extend(self.number.to_be_bytes());
        bytes.extend(self.statement_signature);
        match &self.sealed {
            Some(sealed) => {
                bytes.push(1);
                bytes.extend(&sealed.encapsulated_key);
                bytes.extend(&sealed.ciphertext);
            }
            None => bytes.push(0),
        }
        match &self.link {
            Some(signature) => {
                bytes.push(1);
                bytes.extend(signature);
            }
            None => bytes.push(0),
        }
        bytes
    }

    /// Reads an answer's bytes. It checks the layout only; completing the
    /// answer checks what it carries ([`PairingAnswer::complete`]).
    pub fn from_bytes(bytes: &[u8]) -> Result<PairingAnswer, ReadError> {
        // The fixed part, up to and including the sealed key byte.
        let fixed = bytes.get(..=SEALED_AT).ok_or(ReadError::Length)?;
        if fixed[0] != VERSION {
            return Err(ReadError::Version);
        }
        let kind = match fixed[1] {
            1 => MemberKind::Device,
            2 => MemberKind::Agent,
            _ => return Err(ReadError::Kind),
        };
        let number = u64::from_be_bytes(fixed[130..138].try_into().expect("8 bytes"));
        if number == 0 {
            return Err(ReadError::Number);
        }
        let mut at = SEALED_AT + 1;
        let sealed = match (fixed[SEALED_AT], kind) {
            (1, MemberKind::Device) => {
                let sealed = bytes.get(at..at + SEALED_LEN).ok_or(ReadError::Length)?;
                at += SEALED_LEN;
                Some(SealedAccountKey {
                    encapsulated_key: sealed[..32].to_vec(),
                    ciphertext: sealed[32..].to_vec(),
                })
            }
            (0, MemberKind::Agent) => None,
            _ => return Err(ReadError::SealedKey),
        };
        let link = match bytes.get(at).ok_or(ReadError::Length)? {
            0 => None,
            1 => {
                let signature = bytes.get(at + 1..at + 65).ok_or(ReadError::Length)?;
                at += 64;
                Some(signature.try_into().expect("64 bytes"))
            }
            _ => return Err(ReadError::Link),
        };
        if bytes.len() != at + 1 {
            return Err(ReadError::Length);
        }
        Ok(PairingAnswer {
            kind,
            account_key: fixed[2..34].try_into().expect("32 bytes"),
            certificate_signature: fixed[34..98].try_into().expect("64 bytes"),
            recovery_key: fixed[98..130].try_into().expect("32 bytes"),
            number,
            statement_signature: fixed[138..202].try_into().expect("64 bytes"),
            sealed,
            link,
        })
    }

    /// Completes the answer on the new member `own`: the certificate must
    /// be its own, the statement must verify against the recovery key, the
    /// certificate against the account key the statement names, and for a
    /// device the account key must open and be the one the statement names.
    /// `old_account_key` is the member's own account key from before it
    /// paired, if it had one: the answer must then carry the account key's
    /// half of the link statement for it, and otherwise none.
    ///
    /// Whether the recovery key is the account the person meant is not
    /// checked here: the person compared its fingerprint.
    pub fn complete(
        &self,
        own: &MemberKey,
        old_account_key: Option<&[u8; 32]>,
    ) -> Result<Paired, CheckError> {
        let statement = AccountKeyStatement {
            recovery_key: self.recovery_key,
            account_key: self.account_key,
            number: self.number,
            signature: self.statement_signature,
        };
        if !statement.verify() {
            return Err(CheckError::Statement);
        }
        let certificate = Certificate {
            account_key: self.account_key,
            member: own.id(),
            kind: self.kind,
            parent: None,
            signature: self.certificate_signature,
            statement: None,
        }
        .with_statement(statement);
        if !certificate.verify(&self.account_key) {
            return Err(CheckError::Certificate);
        }
        let account = match &self.sealed {
            Some(sealed) => {
                let key = open_account_key(sealed, own).map_err(|_| CheckError::AccountKey)?;
                if key.verifying_key().to_bytes() != self.account_key {
                    return Err(CheckError::AccountKey);
                }
                Some(key)
            }
            None => None,
        };
        let link = match (self.link, old_account_key) {
            (None, None) => None,
            (Some(signature), Some(old)) => {
                let key =
                    VerifyingKey::from_bytes(&self.account_key).map_err(|_| CheckError::Link)?;
                key.verify_strict(
                    &link_digest(old, &self.recovery_key),
                    &Signature::from_bytes(&signature),
                )
                .map_err(|_| CheckError::Link)?;
                Some(signature)
            }
            _ => return Err(CheckError::Link),
        };
        Ok(Paired {
            certificate,
            account,
            link,
        })
    }
}

/// `ncdb-pair-1:` and unpadded base64url of the layout.
impl fmt::Display for PairingAnswer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{ANSWER_PREFIX}{}",
            BASE64URL_NOPAD.encode(&self.to_bytes())
        )
    }
}

/// Reads an answer's text: the prefix, strict unpadded base64url, and the
/// layout ([`PairingAnswer::from_bytes`]).
impl FromStr for PairingAnswer {
    type Err = ReadError;

    fn from_str(text: &str) -> Result<PairingAnswer, ReadError> {
        let body = text.strip_prefix(ANSWER_PREFIX).ok_or(ReadError::Prefix)?;
        let bytes = BASE64URL_NOPAD
            .decode(body.as_bytes())
            .map_err(|_| ReadError::Encoding)?;
        PairingAnswer::from_bytes(&bytes)
    }
}

impl fmt::Debug for PairingAnswer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PairingAnswer")
            .field("kind", &self.kind)
            .field("number", &self.number)
            .field("sealed_key", &self.sealed.is_some())
            .field("link", &self.link.is_some())
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for Paired {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Paired")
            .field("kind", &self.certificate.kind)
            .field("account_key", &self.account.is_some())
            .field("link", &self.link.is_some())
            .finish_non_exhaustive()
    }
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BuildError::Kind => f.write_str("a pairing answer is for a device or an agent"),
            BuildError::Statement => {
                f.write_str("the account key statement does not name the account key")
            }
            BuildError::Seal(_) => f.write_str("the account key could not be sealed to the device"),
        }
    }
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ReadError::Prefix => "it does not start with ncdb-pair-1:",
            ReadError::Encoding => "it is not unpadded base64url",
            ReadError::Version => "its version is not one this database knows",
            ReadError::Kind => "its kind is neither a device nor an agent",
            ReadError::SealedKey => "its sealed key does not match its kind",
            ReadError::Number => "its account key number is 0",
            ReadError::Link => "its link byte is neither 0 nor 1",
            ReadError::Length => "its length is not the one its bytes give",
        })
    }
}

impl fmt::Display for CheckError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            CheckError::Statement => "the account key statement does not verify",
            CheckError::Certificate => {
                "the certificate is not this member's, or not signed by the account key"
            }
            CheckError::AccountKey => "the account key does not open, or is not the account's",
            CheckError::Link => "the link statement does not hold for this member",
        })
    }
}

impl std::error::Error for BuildError {}
impl std::error::Error for ReadError {}
impl std::error::Error for CheckError {}

/// A link statement (RFC 0010, "The link statement"): a per-database
/// account, `old_account_key`, is part of the account whose identity is
/// `recovery_key`. The old key and the account key `statement` names both
/// sign [`link_digest`]. Its keys and signatures are public, so its
/// `Debug` output names none of them.
#[derive(Clone, PartialEq, Eq)]
pub struct LinkStatement {
    /// Key 1: the per-database account's key.
    pub old_account_key: [u8; 32],
    /// Key 2: the recovery key of the account it joins.
    pub recovery_key: [u8; 32],
    /// Key 3: the old key's signature.
    pub old_signature: [u8; 64],
    /// Key 4: the statement that names the account key of key 5.
    pub statement: AccountKeyStatement,
    /// Key 5: the account key's signature, as a pairing answer carries it.
    pub account_signature: [u8; 64],
}

impl LinkStatement {
    /// The whole statement: `old` signs the joining database's half, beside
    /// the account key's half from the pairing answer and the statement
    /// that names that account key.
    pub fn sign(
        old: &SigningKey,
        statement: &AccountKeyStatement,
        account_signature: [u8; 64],
    ) -> LinkStatement {
        let old_account_key = old.verifying_key().to_bytes();
        let digest = link_digest(&old_account_key, &statement.recovery_key);
        LinkStatement {
            old_account_key,
            recovery_key: statement.recovery_key,
            old_signature: old.sign(&digest).to_bytes(),
            statement: statement.clone(),
            account_signature,
        }
    }

    /// Whether it holds: the statement is of key 2's recovery key and
    /// verifies, and both halves verify, strictly, with the old key and
    /// with the account key the statement names.
    pub fn verify(&self) -> bool {
        let digest = link_digest(&self.old_account_key, &self.recovery_key);
        let holds = |key: &[u8; 32], signature: &[u8; 64]| {
            VerifyingKey::from_bytes(key).is_ok_and(|key| {
                key.verify_strict(&digest, &Signature::from_bytes(signature))
                    .is_ok()
            })
        };
        self.statement.recovery_key == self.recovery_key
            && self.statement.verify()
            && holds(&self.old_account_key, &self.old_signature)
            && holds(&self.statement.account_key, &self.account_signature)
    }

    /// The canonical bytes of the statement's map.
    pub fn encode(&self) -> Vec<u8> {
        let bytes = |bytes: &[u8]| Value::Bytes(bytes.to_vec());
        cbor::encode(&Value::Map(BTreeMap::from([
            (Key::Unsigned(1), bytes(&self.old_account_key)),
            (Key::Unsigned(2), bytes(&self.recovery_key)),
            (Key::Unsigned(3), bytes(&self.old_signature)),
            (Key::Unsigned(4), self.statement.to_value()),
            (Key::Unsigned(5), bytes(&self.account_signature)),
        ])))
    }

    /// Decodes canonical bytes of the map: all five keys, of their lengths,
    /// and no other.
    pub fn decode(bytes: &[u8]) -> Result<LinkStatement, ChangeError> {
        let Value::Map(mut map) = cbor::decode(bytes)? else {
            return Err(ChangeError::InvalidValue(0));
        };
        let mut take = |key: u64| {
            map.remove(&Key::Unsigned(key))
                .ok_or(ChangeError::InvalidValue(key))
        };
        fn fixed<const N: usize>(key: u64, value: Value) -> Result<[u8; N], ChangeError> {
            match value {
                Value::Bytes(bytes) => bytes.try_into().map_err(|_| ChangeError::InvalidValue(key)),
                _ => Err(ChangeError::InvalidValue(key)),
            }
        }
        let link = LinkStatement {
            old_account_key: fixed(1, take(1)?)?,
            recovery_key: fixed(2, take(2)?)?,
            old_signature: fixed(3, take(3)?)?,
            statement: AccountKeyStatement::from_value(take(4)?)?,
            account_signature: fixed(5, take(5)?)?,
        };
        match map.is_empty() {
            true => Ok(link),
            false => Err(ChangeError::InvalidValue(6)),
        }
    }
}

impl fmt::Debug for LinkStatement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LinkStatement")
            .field("number", &self.statement.number)
            .finish_non_exhaustive()
    }
}
