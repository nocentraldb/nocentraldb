//! The account calls through the bridge (NOC-316; RFC 0009, "Accounts and
//! devices"; the shapes the owner decided on NOC-321, Carsten, 2026-11-15):
//! opening for pairing or for recovery, the recovery code until the person
//! confirmed it, and `recovered` (slice 1); pairing (slice 2); removing a
//! lost device (slice 3).

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

/// The positions `account.recoveryCode` answers in `challenge`.
fn challenge(bridge: &mut Bridge) -> [usize; 2] {
    let answer = ok(bridge, "account.recoveryCode", map(&[]));
    match field(&answer, "challenge") {
        Some(Value::Array(positions)) => match positions.as_slice() {
            [Value::Unsigned(a), Value::Unsigned(b)] => [*a as usize, *b as usize],
            other => panic!("challenge: {other:?}"),
        },
        other => panic!("challenge: {other:?}"),
    }
}

/// `account.confirmRecoveryCode`'s arguments: each word at its position.
fn answers(given: &[(usize, &str)]) -> Value {
    map(&[(
        "answers",
        Value::Array(
            given
                .iter()
                .map(|(position, word)| {
                    map(&[
                        ("position", Value::Unsigned(*position as u64)),
                        ("word", text(word)),
                    ])
                })
                .collect(),
        ),
    )])
}

#[test]
fn a_new_database_shows_its_recovery_code_until_two_of_its_words_come_back() {
    let mut bridge = opened();
    let shown = words(&mut bridge).expect("a code to show");
    assert_eq!(shown.len(), 12);
    assert_eq!(
        words(&mut bridge).as_ref(),
        Some(&shown),
        "shown again until confirmed"
    );
    let [a, b] = challenge(&mut bridge);
    assert!(1 <= a && a < b && b <= 12, "{a}, {b}");
    let typed = shown[b - 1].to_uppercase();
    assert_eq!(
        ok(
            &mut bridge,
            "account.confirmRecoveryCode",
            answers(&[(a, &shown[a - 1]), (b, &typed)])
        ),
        Value::Null
    );
    assert_eq!(words(&mut bridge), None);
}

#[test]
fn a_wrong_word_is_refused_and_the_code_stays() {
    let mut bridge = opened();
    let shown = words(&mut bridge).expect("a code");
    let [a, b] = challenge(&mut bridge);
    let wrong = if shown[b - 1] == "zoo" {
        "abandon"
    } else {
        "zoo"
    };
    let answer = send(
        &mut bridge,
        3,
        "account.confirmRecoveryCode",
        answers(&[(a, &shown[a - 1]), (b, wrong)]),
    );
    let error = field(&answer, "error").expect("refused");
    assert_eq!(field(error, "code"), Some(&text("invalidValue")));
    // The message names the position and no word: neither the typed word
    // nor the code's word. It is compared whole, because the code's word
    // is random and can be a word of the message itself, such as "word"
    // or "match" (NOC-535).
    let message = as_text(field(error, "message").expect("a message"));
    assert_eq!(
        message,
        format!(
            "word {b} does not match the recovery code, which stays until two of its words match"
        )
    );
    assert_eq!(words(&mut bridge), Some(shown));
}

#[test]
fn answers_that_are_not_two_words_at_two_positions_are_refused() {
    let mut bridge = opened();
    let shown = words(&mut bridge).expect("a code");
    for given in [
        answers(&[]),
        answers(&[(1, &shown[0])]),
        answers(&[(1, &shown[0]), (1, &shown[0])]),
        answers(&[(0, "x"), (1, &shown[0])]),
        answers(&[(1, &shown[0]), (13, "x")]),
        answers(&[(1, &shown[0]), (2, &shown[1]), (3, &shown[2])]),
        map(&[]),
    ] {
        let answer = send(&mut bridge, 4, "account.confirmRecoveryCode", given);
        let error = field(&answer, "error").expect("refused");
        assert_eq!(field(error, "code"), Some(&text("invalidValue")));
    }
    assert_eq!(words(&mut bridge), Some(shown), "the code stays");
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
        ok(
            &mut bridge,
            "account.confirmRecoveryCode",
            answers(&[(1, "abandon"), (2, "ability")])
        ),
        Value::Null
    );
}

// Slice 2: pairing (RFC 0010, "Adding a device or an agent to an account";
// NOC-321 points 3 to 5).

/// The invitation text of `bridge`'s database.
fn invitation(bridge: &mut Bridge) -> Value {
    field(&ok(bridge, "invitation", map(&[])), "text")
        .expect("a text")
        .clone()
}

/// The fingerprint of `bridge`'s account.
fn fingerprint(bridge: &mut Bridge) -> Value {
    field(&ok(bridge, "invitation", map(&[])), "fingerprint")
        .expect("a fingerprint")
        .clone()
}

#[test]
fn a_device_pairs_a_new_database_and_adds_it_to_its_spaces() {
    let mut laptop = opened();
    let space = ok(&mut laptop, "spaces.create", map(&[]));
    let (mut phone, _) = open_with(&[("pairing", Value::Bool(true))]);

    let paired = ok(
        &mut laptop,
        "account.pair",
        map(&[
            ("kind", text("device")),
            ("invitation", invitation(&mut phone)),
        ]),
    );
    let answer = field(&paired, "answer").expect("an answer").clone();
    assert!(as_text(&answer).starts_with("ncdb-pair-1:"));
    let Some(Value::Array(added)) = field(&paired, "added") else {
        panic!("added: {paired:?}")
    };
    let [entry] = added.as_slice() else {
        panic!("one space: {added:?}")
    };
    assert_eq!(field(entry, "space"), Some(&space));
    let notice = field(entry, "notice").expect("a notice");
    assert_eq!(field(notice, "code"), Some(&text("deviceAdded")));
    assert_eq!(field(&paired, "askAdmins"), Some(&Value::Array(vec![])));

    let completed = ok(
        &mut phone,
        "account.completePairing",
        map(&[("answer", answer)]),
    );
    let Some(Value::Array(notices)) = field(&completed, "notices") else {
        panic!("notices: {completed:?}")
    };
    assert_eq!(field(&notices[0], "code"), Some(&text("devicePaired")));
    assert_eq!(fingerprint(&mut phone), fingerprint(&mut laptop));
}

#[test]
fn an_answer_for_another_database_is_refused() {
    let mut laptop = opened();
    let (mut phone, _) = open_with(&[("pairing", Value::Bool(true))]);
    let (mut thief, _) = open_with(&[("pairing", Value::Bool(true))]);
    let paired = ok(
        &mut laptop,
        "account.pair",
        map(&[
            ("kind", text("device")),
            ("invitation", invitation(&mut phone)),
        ]),
    );
    let answer = field(&paired, "answer").expect("an answer").clone();
    assert_eq!(
        super::code(
            &mut thief,
            "account.completePairing",
            map(&[("answer", answer)])
        ),
        "invalidCertificate"
    );
}

#[test]
fn a_kind_that_is_not_one_and_a_text_that_is_no_answer_are_refused() {
    let mut laptop = opened();
    let (mut phone, _) = open_with(&[("pairing", Value::Bool(true))]);
    let invitation = invitation(&mut phone);
    for kind in ["subAgent", "Device", ""] {
        assert_eq!(
            super::code(
                &mut laptop,
                "account.pair",
                map(&[("kind", text(kind)), ("invitation", invitation.clone())])
            ),
            "invalidValue",
            "{kind}"
        );
    }
    let refused = send(
        &mut phone,
        9,
        "account.completePairing",
        map(&[("answer", text("ncdb-pair-1:AAAAsecretAAAA"))]),
    );
    let error = field(&refused, "error").expect("refused");
    assert_eq!(field(error, "code"), Some(&text("invalidValue")));
    let message = as_text(field(error, "message").expect("a message"));
    assert!(!message.contains("secret"), "{message}");
}

// Slice 3: removing a lost device (RFC 0010, "Losing a device"; NOC-321
// point 6).

/// A laptop with its recovery code's words, and a phone paired into its
/// account and added to the laptop's one space, whose id it answers.
fn paired_phone() -> (Bridge, String, Value) {
    let mut laptop = opened();
    let code = words(&mut laptop).expect("a code").join(" ");
    let space = ok(&mut laptop, "spaces.create", map(&[]));
    let (mut phone, _) = open_with(&[("pairing", Value::Bool(true))]);
    let paired = ok(
        &mut laptop,
        "account.pair",
        map(&[
            ("kind", text("device")),
            ("invitation", invitation(&mut phone)),
        ]),
    );
    let answer = field(&paired, "answer").expect("an answer").clone();
    ok(
        &mut phone,
        "account.completePairing",
        map(&[("answer", answer)]),
    );
    (laptop, code, space)
}

/// The member id of the one member of `space` that is not `bridge`'s own.
fn other_member(bridge: &mut Bridge, space: &Value) -> Value {
    let Value::Array(members) = ok(bridge, "members.list", map(&[("space", space.clone())])) else {
        panic!("members")
    };
    let others: Vec<_> = members
        .iter()
        .filter(|member| field(member, "self") == Some(&Value::Bool(false)))
        .collect();
    let [other] = others.as_slice() else {
        panic!("one other member: {members:?}")
    };
    field(other, "id").expect("an id").clone()
}

// Removing a member needs the lost device's records in the account space,
// which each device writes for itself and the others learn by syncing: the
// end-to-end test in Chromium removes one through the local relay
// (scripts/sdk-browser/account.e2e.test.mjs). Here, a phone whose records
// the laptop has not learnt yet.

#[test]
fn a_device_whose_records_the_account_space_lacks_is_not_found_and_nothing_is_removed() {
    let (mut laptop, code, space) = paired_phone();
    let phone = other_member(&mut laptop, &space);
    assert_eq!(
        super::code(
            &mut laptop,
            "account.removeMember",
            map(&[("member", phone.clone()), ("recover", text(&code))])
        ),
        "notFound"
    );
    assert_eq!(other_member(&mut laptop, &space), phone, "still a member");
}

#[test]
fn a_member_id_or_words_that_are_not_one_are_refused_without_naming_the_words() {
    let (mut laptop, code, space) = paired_phone();
    let phone = other_member(&mut laptop, &space);
    let refused = send(
        &mut laptop,
        8,
        "account.removeMember",
        map(&[("member", text("not-a-member")), ("recover", text(&code))]),
    );
    let error = field(&refused, "error").expect("refused");
    assert_eq!(field(error, "code"), Some(&text("invalidValue")));
    let message = as_text(field(error, "message").expect("a message"));
    assert!(message.contains("member id"), "{message}");
    let refused = send(
        &mut laptop,
        9,
        "account.removeMember",
        map(&[
            ("member", phone),
            (
                "recover",
                text(
                    "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon zebrafish",
                ),
            ),
        ]),
    );
    let error = field(&refused, "error").expect("refused");
    assert_eq!(field(error, "code"), Some(&text("invalidValue")));
    let message = as_text(field(error, "message").expect("a message"));
    assert!(!message.contains("zebrafish"), "{message}");
    assert!(message.contains("recovery code"), "{message}");
}
