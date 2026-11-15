//! The account calls of slice 1 through the bridge (NOC-316; RFC 0009,
//! "Accounts and devices"; the shapes the owner decided on NOC-321,
//! Carsten, 2026-11-15): opening for pairing or for recovery, the recovery
//! code until the person confirmed it, and `recovered`.

use ncdb_core::cbor::Value;

use super::{Bridge, LOCK, as_text, field, map, ok, opened, send, text};

/// The twelve words `account.recoveryCode` answers, or `None`.
fn words(bridge: &mut Bridge) -> Option<Vec<String>> {
    match ok(bridge, "account.recoveryCode", map(&[])) {
        Value::Null => None,
        answer => match field(&answer, "words") {
            Some(Value::Array(words)) => {
                Some(words.iter().map(|w| as_text(w).to_owned()).collect())
            }
            other => panic!("words: {other:?}"),
        },
    }
}

/// A database in memory opened with `extra` beside the lock.
fn open_with(extra: &[(&str, Value)]) -> (Bridge, Value) {
    let mut bridge = Bridge::new();
    let mut args = vec![("lock", text(LOCK)), ("inMemory", Value::Bool(true))];
    args.extend(extra.iter().cloned());
    let answer = send(&mut bridge, 1, "open", map(&args));
    (bridge, answer)
}

#[test]
fn a_new_database_shows_its_recovery_code_until_the_person_confirmed_it() {
    let mut bridge = opened();
    let shown = words(&mut bridge).expect("a code to show");
    assert_eq!(shown.len(), 12);
    assert_eq!(
        words(&mut bridge).as_ref(),
        Some(&shown),
        "shown again until confirmed"
    );
    assert_eq!(
        ok(&mut bridge, "account.confirmRecoveryCode", map(&[])),
        Value::Null
    );
    assert_eq!(words(&mut bridge), None);
}

#[test]
fn recovered_is_null_on_an_open_that_recovered_nothing() {
    let mut bridge = opened();
    assert_eq!(ok(&mut bridge, "account.recovered", map(&[])), Value::Null);
}

#[test]
fn an_open_with_the_recovery_code_recovers_the_account() {
    let mut first = opened();
    let code = words(&mut first).expect("a code").join(" ");
    let (mut bridge, answer) = open_with(&[("recover", text(&code.to_uppercase()))]);
    assert!(field(&answer, "ok").is_some(), "{answer:?}");
    let recovered = ok(&mut bridge, "account.recovered", map(&[]));
    assert_eq!(field(&recovered, "code"), Some(&text("accountRecovered")));
    assert!(
        as_text(field(&recovered, "text").expect("a text")).starts_with("Your account is back")
    );
}

#[test]
fn words_that_are_no_recovery_code_are_refused_without_naming_them() {
    let (_bridge, answer) = open_with(&[(
        "recover",
        text(
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon zebrafish",
        ),
    )]);
    let error = field(&answer, "error").expect("refused");
    assert_eq!(field(error, "code"), Some(&text("invalidValue")));
    let message = as_text(field(error, "message").expect("a message"));
    assert!(!message.contains("zebrafish"), "{message}");
}

#[test]
fn a_database_opened_for_pairing_has_no_recovery_code() {
    let (mut bridge, answer) = open_with(&[("pairing", Value::Bool(true))]);
    assert!(field(&answer, "ok").is_some(), "{answer:?}");
    assert_eq!(words(&mut bridge), None);
    // Nothing to confirm: another tab may have confirmed it already.
    assert_eq!(
        ok(&mut bridge, "account.confirmRecoveryCode", map(&[])),
        Value::Null
    );
}
