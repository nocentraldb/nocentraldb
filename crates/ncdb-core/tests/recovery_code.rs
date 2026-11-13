//! The recovery code, the recovery key and the fingerprint (RFC 0010, "The
//! recovery code" and "The fingerprint"; RFC 0003, "Recovery key"; NOC-263):
//! twelve words from the BIP-39 English list encode 128 bits and a 4-bit
//! checksum, the recovery key derives from the bits with HKDF-SHA256, it
//! signs the first account key statement with number 1, and the fingerprint
//! is computed over it. The vectors of docs/rfc/vectors/0010/, which
//! generate.py writes independently of this crate, pin every step.

// On wasm32, wasm-bindgen-test runs only #[wasm_bindgen_test] functions;
// this routes #[test] there, so every test runs on both targets (NOC-10).
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test as test;

use ed25519_dalek::SigningKey;
use ncdb_core::membership::AccountKeyStatement;
use ncdb_core::recovery::{CodeError, RecoveryCode, WORD_LIST, fingerprint_digits};
use sha2::{Digest, Sha256};

macro_rules! vector {
    ($name:literal, $file:literal) => {
        include_str!(concat!(
            "../../../docs/rfc/vectors/0010/",
            $name,
            ".",
            $file
        ))
        .trim()
    };
}

/// One vector: its bits, words, recovery key, fingerprint and statement.
struct Vector {
    bits: &'static str,
    words: &'static str,
    recovery_key: &'static str,
    fingerprint: &'static str,
    statement: &'static str,
}

macro_rules! named {
    ($name:literal) => {
        Vector {
            bits: vector!($name, "bits.hex"),
            words: vector!($name, "words.txt"),
            recovery_key: vector!($name, "recovery-key.hex"),
            fingerprint: vector!($name, "fingerprint.txt"),
            statement: vector!($name, "statement.hex"),
        }
    };
}

fn vectors() -> [Vector; 4] {
    [named!("zero"), named!("7f"), named!("a0"), named!("ff")]
}

fn hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

fn bits(text: &str) -> [u8; 16] {
    hex(text).try_into().expect("16 bytes")
}

#[test]
fn the_word_list_is_the_bip_39_english_list() {
    let digest = Sha256::digest(WORD_LIST.as_bytes());
    assert_eq!(
        hex("2f5eed53a4727b4bf8880d8f3f199efc90e58503646d9ff8eff3a2ed3b24dbda"),
        digest.to_vec()
    );
}

#[test]
fn the_bits_encode_to_the_vectors_words_and_back() {
    for vector in vectors() {
        let code = RecoveryCode::from_bits(bits(vector.bits));
        assert_eq!(code.words().as_str(), vector.words);
        let parsed = RecoveryCode::parse(vector.words).expect("the vector's words");
        assert_eq!(parsed.words().as_str(), vector.words);
        assert_eq!(parsed, code);
    }
}

#[test]
fn the_recovery_key_and_its_fingerprint_match_the_vectors() {
    for vector in vectors() {
        let key = RecoveryCode::parse(vector.words).unwrap().recovery_key();
        assert_eq!(key.public().to_vec(), hex(vector.recovery_key));
        assert_eq!(fingerprint_digits(&key.public()), vector.fingerprint);
    }
}

#[test]
fn the_first_statement_has_number_1_and_matches_the_vectors() {
    let account = SigningKey::from_bytes(&[0x11; 32])
        .verifying_key()
        .to_bytes();
    for vector in vectors() {
        let key = RecoveryCode::parse(vector.words).unwrap().recovery_key();
        let statement = AccountKeyStatement::first(&key, account);
        assert_eq!(statement.number, 1);
        assert!(statement.verify());
        assert_eq!(statement.encode(), hex(vector.statement));
    }
}

#[test]
fn words_compare_without_case_and_surrounding_whitespace() {
    let messy = "  Abandon ABANDON abandon\tabandon abandon abandon\nabandon abandon abandon abandon abandon About  ";
    let code = RecoveryCode::parse(messy).expect("the same code");
    assert_eq!(code, RecoveryCode::from_bits([0; 16]));
}

#[test]
fn a_wrong_count_a_word_off_the_list_or_a_wrong_checksum_is_refused() {
    let eleven = ["abandon"; 11].join(" ");
    assert_eq!(RecoveryCode::parse(&eleven), Err(CodeError::WordCount(11)));

    let unknown = format!("{} notaword", ["abandon"; 11].join(" "));
    assert_eq!(
        RecoveryCode::parse(&unknown),
        Err(CodeError::UnknownWord { position: 12 })
    );

    // Twelve times "abandon" is 132 zero bits; the checksum of 128 zero
    // bits is 0011, so the last word must be "about".
    let twelve = ["abandon"; 12].join(" ");
    assert_eq!(RecoveryCode::parse(&twelve), Err(CodeError::Checksum));
}

#[test]
fn a_generated_code_round_trips_through_its_words() {
    let code = RecoveryCode::generate();
    let again = RecoveryCode::parse(&code.words()).expect("its own words");
    assert_eq!(again, code);
    assert_ne!(RecoveryCode::generate(), code, "128 random bits");
}

#[test]
fn debug_shows_no_word_and_no_byte() {
    let code = RecoveryCode::from_bits([0x7f; 16]);
    let shown = format!("{code:?} {:?}", code.recovery_key());
    for word in code.words().split(' ') {
        assert!(!shown.contains(word), "{shown} shows {word}");
    }
    assert!(!shown.contains("7f") && !shown.contains("127"), "{shown}");

    let error = RecoveryCode::parse(
        "legal winner thank year wave sausage worth useful legal winner thank secretword",
    )
    .unwrap_err();
    assert!(!format!("{error:?} {error}").contains("secretword"));
}

#[test]
fn a_code_gives_back_the_bits_it_was_made_from() {
    for vector in vectors() {
        let bits = bits(vector.bits);
        assert_eq!(*RecoveryCode::from_bits(bits).bits(), bits);
    }
    let code = RecoveryCode::generate();
    assert_eq!(RecoveryCode::from_bits(*code.bits()), code);
}
