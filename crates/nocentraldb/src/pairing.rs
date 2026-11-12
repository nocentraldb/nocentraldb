//! Accounts in the Rust SDK (RFC 0010, "Adding a device or an agent to an
//! account" and "The recovery code"; RFC 0009, "Accounts and devices"; the
//! owner's NOC-273 A, Carsten, 2026-11-12): the recovery code the app shows
//! until the person stored it, and the pairing answer a device of the
//! account hands a new device or an agent.
//!
//! **What this does not do yet.** Where a device keeps its recovery code
//! and its account key statement, and the invitation of a database opened
//! for pairing, wait on the owner (NOC-285); so do `db.account()` and its
//! calls, which store them.

use std::fmt;
use std::str::FromStr;

use ncdb_core::pairing::{self as core, CheckError};
use ncdb_core::recovery::{RecoveryCode as CoreCode, WORD_LIST};

use crate::error::{Error, ErrorCode};
use crate::id::SpaceId;
use crate::members::Notice;

/// What a device of the account pairs: a person's device, which receives
/// the account key, or an agent, which receives a certificate only.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PairKind {
    /// A person's device.
    Device,
    /// An agent.
    Agent,
}

/// What pairing returns on the device of the account: the answer to show
/// the new device, one notice per space the SDK added the new device to,
/// and the spaces where no device of the account is an admin, whose admins
/// the app asks. For an agent, `added` and `ask_admins` are empty.
#[derive(Clone, Debug)]
pub struct Paired {
    /// The answer, to show as a QR code or text.
    pub answer: PairingAnswer,
    /// The spaces the new device was added to, each with its notice.
    pub added: Vec<(SpaceId, Notice)>,
    /// The spaces whose admins the app asks to add the new device.
    pub ask_admins: Vec<SpaceId>,
}

/// A pairing answer (RFC 0010, "The pairing answer"): public data with a
/// text form, as an invitation is. `Display` writes `ncdb-pair-1:` and
/// unpadded base64url of its layout, and `FromStr` reads it, failing with
/// `InvalidValue`. It carries the account key sealed to one device, which
/// only that device opens, so it is not redacted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PairingAnswer(pub(crate) core::PairingAnswer);

impl fmt::Display for PairingAnswer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Reads an answer's text. A text off the layout is refused with
/// `InvalidValue`, and nothing is read from it; the message names what is
/// wrong, never a byte.
impl FromStr for PairingAnswer {
    type Err = Error;

    fn from_str(text: &str) -> Result<PairingAnswer, Error> {
        text.parse()
            .map(PairingAnswer)
            .map_err(|why: core::ReadError| {
                Error::new(
                    ErrorCode::InvalidValue,
                    format!("the text is not a pairing answer: {why}"),
                )
            })
    }
}

/// The error of an answer that does not hold for this member:
/// `InvalidCertificate`, which may mean a forged or swapped answer.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn check_error(why: CheckError) -> Error {
    Error::new(
        ErrorCode::InvalidCertificate,
        format!("the pairing answer does not hold for this database: {why}"),
    )
}

/// The account's recovery code, held until the person stored it (RFC
/// 0010, "The recovery code"). `Display` and `Debug` print
/// `RecoveryCode(redacted)`, and it implements no serde trait, so no
/// serializer can put it into a log. [`RecoveryCode::words`] is the one way
/// to read it.
pub struct RecoveryCode {
    code: CoreCode,
}

impl RecoveryCode {
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn new(code: CoreCode) -> RecoveryCode {
        RecoveryCode { code }
    }

    /// The twelve words, from the BIP-39 English list, for the person to
    /// write down or store in a password manager.
    pub fn words(&self) -> [&'static str; 12] {
        let words = self.code.words();
        let mut out = [""; 12];
        for (slot, word) in out.iter_mut().zip(words.split(' ')) {
            *slot = WORD_LIST
                .lines()
                .find(|listed| *listed == word)
                .expect("a recovery code's words are on the list");
        }
        out
    }
}

impl fmt::Display for RecoveryCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RecoveryCode(redacted)")
    }
}

impl fmt::Debug for RecoveryCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RecoveryCode(redacted)")
    }
}

#[cfg(test)]
#[path = "pairing_tests.rs"]
mod tests;
