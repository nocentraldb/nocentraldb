//! Attribute values in and out of DynamoDB items, and SDK errors as store
//! errors. A store error names the step and the error's code, never an
//! item's content.

use std::collections::HashMap;
use std::fmt;
use std::time::Duration;

use aws_sdk_dynamodb::error::ProvideErrorMetadata;
use aws_sdk_dynamodb::primitives::Blob;
use aws_sdk_dynamodb::types::AttributeValue;
use ncdb_relay::store::StoreError;

/// A DynamoDB item.
pub type Item = HashMap<String, AttributeValue>;

/// Bytes in lowercase hex.
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// A binary attribute.
pub fn b(bytes: &[u8]) -> AttributeValue {
    AttributeValue::B(Blob::new(bytes))
}

/// A number attribute.
pub fn n(number: u64) -> AttributeValue {
    AttributeValue::N(number.to_string())
}

/// A store error for a step whose SDK call failed: the step and the error's
/// code, or the kind of failure when the service never answered.
pub fn failed<E>(step: &'static str) -> impl Fn(E) -> StoreError
where
    E: ProvideErrorMetadata + fmt::Debug,
{
    move |error| match error.code() {
        Some(code) => StoreError(format!("{step}: {code}")),
        None => StoreError(format!("{step}: no answer from the service")),
    }
}

/// A store error for an item that lacks an attribute or holds a wrong one.
fn malformed(table: &str, name: &str) -> StoreError {
    StoreError(format!("an item of {table} has no valid {name}"))
}

/// The bytes of a binary attribute.
pub fn get_b(item: &Item, table: &str, name: &str) -> Result<Vec<u8>, StoreError> {
    match item.get(name) {
        Some(AttributeValue::B(blob)) => Ok(blob.as_ref().to_vec()),
        _ => Err(malformed(table, name)),
    }
}

/// A fixed-size id from a binary attribute.
pub fn get_id<const N: usize>(item: &Item, table: &str, name: &str) -> Result<[u8; N], StoreError> {
    get_b(item, table, name)?
        .try_into()
        .map_err(|_| malformed(table, name))
}

/// A number attribute.
pub fn get_n(item: &Item, table: &str, name: &str) -> Result<u64, StoreError> {
    match item.get(name) {
        Some(AttributeValue::N(number)) => number.parse().map_err(|_| malformed(table, name)),
        _ => Err(malformed(table, name)),
    }
}

/// A boolean attribute, false when the item lacks it.
pub fn get_bool(item: &Item, name: &str) -> bool {
    matches!(item.get(name), Some(AttributeValue::Bool(true)))
}

/// A string attribute, if the item has one.
pub fn get_s<'a>(item: &'a Item, name: &str) -> Option<&'a str> {
    match item.get(name) {
        Some(AttributeValue::S(s)) => Some(s),
        _ => None,
    }
}

/// How long to wait before attempt `attempt` (from 1) of a call that
/// another transaction got in the way of: a short, growing pause with
/// jitter, so contending handles spread out.
pub async fn back_off(attempt: u32) {
    let mut jitter = [0u8; 1];
    let _ = getrandom::fill(&mut jitter);
    let base = 5u64 << attempt.min(6);
    let pause = base + u64::from(jitter[0]) % base.max(1);
    tokio::time::sleep(Duration::from_millis(pause.min(500))).await;
}

/// How often a call retries a transaction that a conflict, not a
/// condition, cancelled.
pub const ATTEMPTS: u32 = 60;

/// A time in milliseconds as the seconds a DynamoDB TTL attribute holds.
pub fn ttl(ms: u64) -> AttributeValue {
    n(ms / 1000 + 1)
}

/// Lowercase or uppercase hex as bytes, if it is hex.
pub fn unhex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok())
        .collect()
}
