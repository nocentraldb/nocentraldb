//! The recovery code (RFC 0010, "The recovery code"; RFC 0003, "Recovery
//! key"; NOC-263): twelve words from the BIP-39 English list that encode 128
//! random bits and a 4-bit checksum. The words are only an encoding: BIP-39's
//! PBKDF2 step to a wallet seed is not used. The recovery key derives from
//! the 128 bits ([`RecoveryCode::recovery_key`]), and the account's
//! fingerprint is computed over the recovery key ([`fingerprint`]).
//!
//! The code and its words are secrets: they are zeroized when dropped, and
//! no `Debug` or error shows a word or a byte of them.

use std::fmt;
use std::sync::OnceLock;

use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::keys::{RecoveryKey, random};
use crate::redact::Hidden;

/// The BIP-39 English word list, 2048 words in order, one per line. Its
/// SHA-256 is checked in tests/recovery_code.rs.
pub const WORD_LIST: &str = include_str!("recovery/bip39-english.txt");

/// The number of words of a recovery code.
const WORDS: usize = 12;

/// The context string in front of the recovery key in the fingerprint's
/// hash (RFC 0010, "The fingerprint").
pub const FINGERPRINT_CONTEXT: &[u8] = b"nocentraldb/account-fingerprint";

/// The 128 bits of an account's recovery code. Two codes compare by their
/// bits; `Debug` shows neither bits nor words.
#[derive(Clone, PartialEq, Eq)]
pub struct RecoveryCode(Zeroizing<[u8; 16]>);

/// Why words are not a recovery code. It names a word's position, never the
/// word, which may be most of a secret.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum CodeError {
    /// Not twelve words: how many there were.
    WordCount(usize),
    /// The word at this position, counted from 1, is not on the list.
    UnknownWord {
        /// The position of the word, counted from 1.
        position: usize,
    },
    /// Every word is on the list, but the checksum does not match: a word
    /// is wrong or out of order.
    Checksum,
}

impl fmt::Display for CodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CodeError::WordCount(count) => {
                write!(f, "a recovery code has 12 words, not {count}")
            }
            CodeError::UnknownWord { position } => {
                write!(f, "word {position} of the recovery code is not on the list")
            }
            CodeError::Checksum => f.write_str(
                "the recovery code's checksum does not match: a word is wrong or out of order",
            ),
        }
    }
}

impl std::error::Error for CodeError {}

/// The words of [`WORD_LIST`], in order, read once.
fn list() -> &'static [&'static str] {
    static LIST: OnceLock<Vec<&'static str>> = OnceLock::new();
    LIST.get_or_init(|| WORD_LIST.lines().collect())
}

/// The index of `word` in the list, which is sorted.
fn index(word: &str) -> Option<u16> {
    list()
        .binary_search(&word)
        .ok()
        .and_then(|i| u16::try_from(i).ok())
}

/// The first 4 bits of the SHA-256 hash of the bits: the checksum.
fn checksum(bits: &[u8; 16]) -> u8 {
    Sha256::digest(bits)[0] >> 4
}

impl RecoveryCode {
    /// A new recovery code from the platform's secure random generator.
    ///
    /// # Panics
    ///
    /// When the platform has no secure generator.
    pub fn generate() -> RecoveryCode {
        RecoveryCode(random())
    }

    /// The recovery code of these 128 bits. The caller's copy is not wiped.
    pub fn from_bits(bits: [u8; 16]) -> RecoveryCode {
        RecoveryCode(Zeroizing::new(bits))
    }

    /// Reads a recovery code from its twelve words. Case does not matter,
    /// and the words may be separated, led and followed by any whitespace.
    pub fn parse(text: &str) -> Result<RecoveryCode, CodeError> {
        let lowered = Zeroizing::new(text.to_lowercase());
        let words: Vec<&str> = lowered.split_whitespace().collect();
        if words.len() != WORDS {
            return Err(CodeError::WordCount(words.len()));
        }
        // 12 words of 11 bits each: the 128 bits, then the 4-bit checksum,
        // 132 bits in 17 bytes.
        let mut all = Zeroizing::new([0u8; 17]);
        for (position, word) in words.iter().enumerate() {
            let index = index(word).ok_or(CodeError::UnknownWord {
                position: position + 1,
            })?;
            for k in 0..11 {
                let bit = position * 11 + k;
                if index >> (10 - k) & 1 == 1 {
                    all[bit / 8] |= 0x80 >> (bit % 8);
                }
            }
        }
        let mut code = RecoveryCode(Zeroizing::new([0u8; 16]));
        code.0.copy_from_slice(&all[..16]);
        if all[16] >> 4 != checksum(&code.0) {
            return Err(CodeError::Checksum);
        }
        Ok(code)
    }

    /// The twelve words, separated by one space.
    pub fn words(&self) -> Zeroizing<String> {
        let mut all = Zeroizing::new([0u8; 17]);
        all[..16].copy_from_slice(self.0.as_ref());
        all[16] = checksum(&self.0) << 4;
        let list = list();
        let mut text = Zeroizing::new(String::with_capacity(12 * 9));
        for i in 0..WORDS {
            let mut index = 0usize;
            for k in 0..11 {
                let bit = i * 11 + k;
                index = index << 1 | usize::from(all[bit / 8] >> (7 - bit % 8) & 1);
            }
            if i > 0 {
                text.push(' ');
            }
            text.push_str(list[index]);
        }
        text
    }

    /// The account's recovery key, derived from the 128 bits (RFC 0003,
    /// "Recovery key").
    pub fn recovery_key(&self) -> RecoveryKey {
        RecoveryKey::from_code(&self.0)
    }
}

impl fmt::Debug for RecoveryCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("RecoveryCode").field(&Hidden).finish()
    }
}

/// The account's fingerprint: the SHA-256 hash of
/// `"nocentraldb/account-fingerprint"` and the recovery key (RFC 0010, "The
/// fingerprint").
pub fn fingerprint(recovery_key: &[u8; 32]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(FINGERPRINT_CONTEXT);
    hash.update(recovery_key);
    hash.finalize().into()
}

/// The fingerprint as people compare it: the hash read as one big-endian
/// number, taken modulo 10^60, written as 60 decimal digits with leading
/// zeros in twelve blocks of five separated by one space.
pub fn fingerprint_digits(recovery_key: &[u8; 32]) -> String {
    let mut number = fingerprint(recovery_key);
    // The 60 lowest decimal digits, by long division by 10, least
    // significant first.
    let mut digits = [0u8; 60];
    for digit in digits.iter_mut().rev() {
        let mut remainder = 0u16;
        for byte in number.iter_mut() {
            let value = remainder << 8 | u16::from(*byte);
            *byte = u8::try_from(value / 10).expect("a quotient below 256");
            remainder = value % 10;
        }
        *digit = b'0' + u8::try_from(remainder).expect("a digit");
    }
    digits
        .chunks(5)
        .map(|block| std::str::from_utf8(block).expect("ASCII digits"))
        .collect::<Vec<_>>()
        .join(" ")
}
