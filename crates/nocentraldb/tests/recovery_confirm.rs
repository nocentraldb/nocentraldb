//! Confirming the recovery code asks for two of its words back (NOC-502,
//! option A, Carsten, 2026-11-23; RFC 0010, "The recovery code"):
//! `challenge()` gives two positions the SDK picked at random,
//! `confirm(answers)` checks the words typed at them, and a wrong word
//! keeps the code.

mod support;

use nocentraldb::{Db, ErrorCode, OpenOptions, RecoveryCode};
use support::{LOCK, TempDir, now};

fn at(dir: &TempDir, name: &str) -> OpenOptions {
    OpenOptions::new(LOCK)
        .path(dir.join(name))
        .key_file(dir.join(&format!("{name}.keys")))
}

fn code(db: &Db) -> Option<RecoveryCode> {
    now(db.account().recovery_code()).unwrap()
}

/// The words of `code` at the positions of its challenge, as the person
/// would type them back.
fn answers(code: &RecoveryCode) -> [&'static str; 2] {
    let words = code.words();
    code.challenge().map(|position| words[position - 1])
}

#[test]
fn the_challenge_is_two_different_positions_from_1_to_12() {
    let db = now(Db::open(OpenOptions::new(LOCK).in_memory())).unwrap();
    let code = code(&db).unwrap();
    let [first, second] = code.challenge();
    assert!((1..=12).contains(&first) && (1..=12).contains(&second));
    assert!(first < second, "two different positions, in order");
    assert_eq!(code.challenge(), [first, second], "the same for one code");
}

#[test]
fn the_challenge_is_picked_at_random() {
    let db = now(Db::open(OpenOptions::new(LOCK).in_memory())).unwrap();
    // 66 pairs are possible; 40 codes that all share one pair would mean
    // the positions are fixed (a chance of 66^-39 at random).
    let first = code(&db).unwrap().challenge();
    assert!((0..40).any(|_| code(&db).unwrap().challenge() != first));
}

#[test]
fn the_two_words_typed_back_confirm_and_the_code_is_forgotten() {
    let dir = TempDir::new("confirm-two-words");
    let db = now(Db::open(at(&dir, "laptop"))).unwrap();
    let code = code(&db).unwrap();
    let [first, second] = answers(&code);
    // Letter case and spacing do not matter, as for `recover`.
    let first = format!("  {} ", first.to_uppercase());
    now(code.confirm([&first, second])).unwrap();
    assert!(self::code(&db).is_none());
    drop(db);
    let db = now(Db::open(at(&dir, "laptop"))).unwrap();
    assert!(self::code(&db).is_none(), "forgotten for good");
}

#[test]
fn a_wrong_word_is_invalid_names_its_position_and_keeps_the_code() {
    let dir = TempDir::new("confirm-wrong-word");
    let db = now(Db::open(at(&dir, "laptop"))).unwrap();
    let code = code(&db).unwrap();
    let [first, second] = answers(&code);
    let wrong = if second == "zoo" { "abandon" } else { "zoo" };
    let error = now(code.confirm([first, wrong])).unwrap_err();
    assert_eq!(error.code(), ErrorCode::InvalidValue);
    // The text names the position and no word: neither the typed word nor
    // the code's word. It is compared whole, because the code's word is
    // random and can be a word of the text itself, such as "word" or
    // "match" (NOC-535).
    let position = code.challenge()[1];
    assert_eq!(
        error.to_string(),
        format!(
            "word {position} does not match the recovery code, which stays until two of its words match"
        )
    );
    // The code stays, also after the database is opened again, and the
    // same object takes a corrected answer.
    assert!(self::code(&db).is_some());
    drop(db);
    let db = now(Db::open(at(&dir, "laptop"))).unwrap();
    let code = self::code(&db).expect("a wrong word keeps the code");
    now(code.confirm(answers(&code))).unwrap();
    assert!(self::code(&db).is_none());
}

#[test]
fn the_words_at_other_positions_do_not_confirm() {
    let db = now(Db::open(OpenOptions::new(LOCK).in_memory())).unwrap();
    let code = code(&db).unwrap();
    let words = code.words();
    let asked = code.challenge();
    // Two words of the code, but not at the positions asked for.
    let others: Vec<&str> = (1..=12)
        .filter(|p| !asked.contains(p))
        .map(|p| words[p - 1])
        .filter(|w| !asked.iter().any(|a| words[a - 1] == *w))
        .take(2)
        .collect();
    let error = now(code.confirm([others[0], others[1]])).unwrap_err();
    assert_eq!(error.code(), ErrorCode::InvalidValue);
    assert!(self::code(&db).is_some());
}
