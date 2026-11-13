//! A new account key after a lost device (RFC 0010, "Losing a device",
//! step 3; NOC-268), the account space's side. The device that removed
//! the lost member draws a new account key, signs its statement with the
//! next number under the recovery key, and records in the account space
//! the statement and, for each remaining device, the key sealed to that
//! device's agreement key, as a pairing answer seals it (RFC 0003, "Pairing
//! answer"). Only that device opens it: the space key, which every member
//! of the account space holds, opens none, so a device that joins later
//! cannot read an account key the account had.
//!
//! A record's id derives from what it records, as the member records'
//! ids do: a statement's from its number, a sealed key's from its device
//! and number.
//!
//! `Account::remove_member` (RFC 0009) calls it once the `Account` handle
//! is built (NOC-267); until then only the tests do.
#![cfg_attr(not(test), allow(dead_code))]

use ed25519_dalek::SigningKey;
use ncdb_core::ids::{MemberId as CoreMemberId, ObjectId as CoreObjectId};
use ncdb_core::keys::RecoveryKey;
use ncdb_core::membership::AccountKeyStatement;
use ncdb_core::pairing::{SealedAccountKey, open_account_key, seal_account_key};

use super::{
    Kind, STATEMENTS, Values, WRAPPED_KEYS, member_object, member_rows, model, record_id, rows,
    space, text,
};
use crate::db::Inner;
use crate::error::{Error, ErrorCode};
use crate::id::ObjectId;
use crate::tx::Transaction;
use crate::value::{Repr, Value};

/// The length of HPKE's encapsulated key in a sealed account key.
const ENCAPSULATED: usize = 32;
/// The length of the sealed seed with its tag.
const CIPHERTEXT: usize = 48;

fn statement_object(number: u64) -> CoreObjectId {
    record_id("nocentraldb/account-statement", &[&number.to_be_bytes()])
}

fn wrapped_object(member: [u8; 32], number: u64) -> CoreObjectId {
    record_id(
        "nocentraldb/account-wrapped-key",
        &[&member, &number.to_be_bytes()],
    )
}

fn base64(bytes: &[u8]) -> String {
    data_encoding::BASE64URL_NOPAD.encode(bytes)
}

fn number_value(number: u64) -> Result<Value, Error> {
    i64::try_from(number).map(Value::from).map_err(|_| {
        Error::new(
            ErrorCode::InvalidValue,
            "the statement's number is too high",
        )
    })
}

fn number(values: &Values) -> Option<u64> {
    match values.get("number")? {
        Some(Value(Repr::Integer(n))) => u64::try_from(*n).ok(),
        _ => None,
    }
}

/// Draws a new account key, signs its statement with `number` under
/// `recovery`, and records both in the account space: the statement, and
/// the key sealed to each remaining device of the account, every device
/// whose record does not say `removed` and which is a member of the
/// account space, other than this one. Returns the statement and the key.
///
/// A device without an account space is refused with `NotFound`, one that
/// cannot write into it yet with the code of that write; a number of 0 or
/// one the account space already records, with `InvalidValue`. Nothing is
/// written when a call is refused.
pub(crate) fn renew(
    inner: &mut Inner,
    recovery: &RecoveryKey,
    number: u64,
) -> Result<(AccountKeyStatement, SigningKey), Error> {
    let Some(account) = space(inner) else {
        return Err(Error::new(
            ErrorCode::NotFound,
            "the device holds no account space",
        ));
    };
    crate::space::writable_by(inner, account, super::id())?;
    if number == 0 || statements(inner)?.iter().any(|s| s.number == number) {
        return Err(Error::new(
            ErrorCode::InvalidValue,
            "the account space already records an account key with this number",
        ));
    }
    let key = SigningKey::from_bytes(&crate::keys::random());
    let statement = AccountKeyStatement::sign(recovery, key.verifying_key().to_bytes(), number);
    let me = inner.member.key().id();
    let mut tx = Transaction::reserved(model());
    tx.create_at(
        STATEMENTS,
        statement_object(number),
        [
            ("number", number_value(number)?),
            (
                "statement",
                Value::from(base64(&statement.encode()).as_str()),
            ),
        ],
    )?;
    let resolved = inner.spaces[&account].checks.resolved();
    for record in member_rows(inner)?.into_values() {
        let device = CoreMemberId(record.member);
        if record.removed || record.kind != Kind::Device || device == me {
            continue;
        }
        let (Some(_), Some(state)) = (resolved.role(device), resolved.member(device)) else {
            continue;
        };
        let sealed = seal_account_key(&key, &state.key)
            .expect("a member's public key takes a sealed account key");
        let mut bytes = sealed.encapsulated_key;
        bytes.extend_from_slice(&sealed.ciphertext);
        tx.create_at(
            WRAPPED_KEYS,
            wrapped_object(record.member, number),
            [
                (
                    "member",
                    Value::from(ObjectId(member_object(record.member))),
                ),
                ("number", number_value(number)?),
                ("sealed", Value::from(base64(&bytes).as_str())),
            ],
        )?;
    }
    super::write(inner, account, tx)?;
    Ok((statement, key))
}

/// The account key statements the account space records whose signature
/// verifies, in the order of their numbers.
pub(crate) fn statements(inner: &Inner) -> Result<Vec<AccountKeyStatement>, Error> {
    let mut statements: Vec<AccountKeyStatement> = rows(inner, STATEMENTS, "byNumber")?
        .into_iter()
        .filter_map(|(_, values)| {
            let bytes = data_encoding::BASE64URL_NOPAD
                .decode(text(&values, "statement")?.as_bytes())
                .ok()?;
            let statement = AccountKeyStatement::decode(&bytes).ok()?;
            (statement.verify() && Some(statement.number) == number(&values)).then_some(statement)
        })
        .collect();
    statements.sort_by_key(|s| s.number);
    Ok(statements)
}

/// The sealed account keys the account space holds: for each, the member
/// it is sealed to, its number, and the sealed bytes as the record holds
/// them.
fn wrapped(inner: &Inner) -> Result<Vec<([u8; 32], u64, String)>, Error> {
    let members = member_rows(inner)?;
    let mut wrapped = Vec::new();
    for (_, values) in rows(inner, WRAPPED_KEYS, "byNumber")? {
        let member = match values.get("member") {
            Some(Some(Value(Repr::Id(object)))) => members.get(&object.0),
            _ => None,
        };
        if let (Some(member), Some(number), Some(sealed)) =
            (member, number(&values), text(&values, "sealed"))
        {
            wrapped.push((member.member, number, sealed));
        }
    }
    wrapped.sort();
    Ok(wrapped)
}

/// The member and number of each sealed account key the account space
/// holds, in order.
pub(crate) fn wrapped_to(inner: &Inner) -> Result<Vec<([u8; 32], u64)>, Error> {
    Ok(wrapped(inner)?
        .into_iter()
        .map(|(member, number, _)| (member, number))
        .collect())
}

/// The newest account key the account space hands this device, with its
/// statement: the sealed key of the highest number that opens with this
/// device's agreement key and is the key a statement the account space
/// records names. None when it holds none; a sealed key that does not
/// open or match is passed over.
pub(crate) fn new_key(inner: &Inner) -> Result<Option<(AccountKeyStatement, SigningKey)>, Error> {
    let me = inner.member.key();
    let own = me.id().0;
    let statements = statements(inner)?;
    let mut best: Option<(AccountKeyStatement, SigningKey)> = None;
    for (member, number, sealed) in wrapped(inner)? {
        if member != own || best.as_ref().is_some_and(|(s, _)| s.number >= number) {
            continue;
        }
        let Some(statement) = statements.iter().find(|s| s.number == number) else {
            continue;
        };
        let Ok(bytes) = data_encoding::BASE64URL_NOPAD.decode(sealed.as_bytes()) else {
            continue;
        };
        if bytes.len() != ENCAPSULATED + CIPHERTEXT {
            continue;
        }
        let sealed = SealedAccountKey {
            encapsulated_key: bytes[..ENCAPSULATED].to_vec(),
            ciphertext: bytes[ENCAPSULATED..].to_vec(),
        };
        let Ok(key) = open_account_key(&sealed, &me) else {
            continue;
        };
        if key.verifying_key().to_bytes() == statement.account_key {
            best = Some((statement.clone(), key));
        }
    }
    Ok(best)
}
