//! The highest account key statement number the relay has seen per
//! identity (RFC 0015, "The account's identity"; NOC-420): an item in
//! `spaces` under [`statement_key`], with the number as `statement`.
//!
//! An append raises it with an update in its own transaction, on the
//! condition that no higher or equal number is stored. When that condition
//! fails, another append stored such a number, so the append sends its
//! transaction again without the update: the number never falls, and the
//! append holds whatever number it brings.

use aws_sdk_dynamodb::types::{TransactWriteItem, Update};
use ncdb_relay::store::StoreError;

use crate::Aws;
use crate::item::{b, failed, get_n, n};

/// The key of an identity's statement item in `spaces`: its 32 bytes and
/// the byte `s`, 33 bytes, which no 16-byte space id, no 32-byte guest
/// item and no reserved item can be.
pub fn statement_key(identity: [u8; 32]) -> Vec<u8> {
    let mut key = identity.to_vec();
    key.push(b's');
    key
}

/// The highest statement number stored for `identity`, if any.
pub async fn statement(aws: &Aws, identity: [u8; 32]) -> Result<Option<u64>, StoreError> {
    let out = aws
        .db
        .get_item()
        .table_name(&aws.names.spaces)
        .key("space", b(&statement_key(identity)))
        .consistent_read(true)
        .send()
        .await
        .map_err(failed("read a statement number"))?;
    out.item
        .map(|item| get_n(&item, "spaces", "statement"))
        .transpose()
}

/// The update that raises the number of `identity` to `number`, on the
/// condition that the stored number is lower or none is stored.
pub fn raise(aws: &Aws, identity: [u8; 32], number: u64) -> Result<TransactWriteItem, StoreError> {
    let update = Update::builder()
        .table_name(&aws.names.spaces)
        .key("space", b(&statement_key(identity)))
        .update_expression("SET #statement = :number")
        .condition_expression("attribute_not_exists(#statement) OR #statement < :number")
        .expression_attribute_names("#statement", "statement")
        .expression_attribute_values(":number", n(number))
        .build()
        .map_err(|e| StoreError(format!("append: a transaction item: {e}")))?;
    Ok(TransactWriteItem::builder().update(update).build())
}
