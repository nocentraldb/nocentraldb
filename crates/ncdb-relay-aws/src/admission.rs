//! An identity's admission (RFC 0015, "How the relay takes it"; NOC-469):
//! an item in `spaces` under [`admission_key`], with the latest admission's
//! `until`, `spaces` and `admission_key`, and `created`, the count of
//! spaces the identity created under its admissions.
//!
//! `admit` is one `UpdateItem` that sets the first three and keeps
//! `created`, starting it at 0 for a new identity. An append raises
//! `created` with an `ADD` in its own transaction, on the condition that
//! the item exists and `created` is below `spaces`, so two appends at once
//! cannot exceed it.

use aws_sdk_dynamodb::types::{TransactWriteItem, Update};
use ncdb_relay::store::{AdmissionRecord, Admitted, StoreError};

use crate::Aws;
use crate::item::{b, failed, get_id, get_n, n};

/// The key of an identity's admission item in `spaces`: its 32 bytes and
/// the byte `a`, 33 bytes, which no 16-byte space id, no 32-byte guest
/// item, no reserved item and, by its last byte, no statement item can be.
pub fn admission_key(identity: [u8; 32]) -> Vec<u8> {
    let mut key = identity.to_vec();
    key.push(b'a');
    key
}

/// Records `admitted` for `identity`, keeping its count.
pub async fn admit(aws: &Aws, identity: [u8; 32], admitted: Admitted) -> Result<(), StoreError> {
    aws.db
        .update_item()
        .table_name(&aws.names.spaces)
        .key("space", b(&admission_key(identity)))
        .update_expression(
            "SET #until = :until, #spaces = :spaces, #key = :key, \
             #created = if_not_exists(#created, :zero)",
        )
        .expression_attribute_names("#until", "until")
        .expression_attribute_names("#spaces", "spaces")
        .expression_attribute_names("#key", "admission_key")
        .expression_attribute_names("#created", "created")
        .expression_attribute_values(":until", n(admitted.until))
        .expression_attribute_values(":spaces", n(admitted.spaces))
        .expression_attribute_values(":key", b(&admitted.key))
        .expression_attribute_values(":zero", n(0))
        .send()
        .await
        .map_err(failed("admit"))?;
    Ok(())
}

/// The identity's admission, if `admit` recorded one.
pub async fn admission(
    aws: &Aws,
    identity: [u8; 32],
) -> Result<Option<AdmissionRecord>, StoreError> {
    let out = aws
        .db
        .get_item()
        .table_name(&aws.names.spaces)
        .key("space", b(&admission_key(identity)))
        .consistent_read(true)
        .send()
        .await
        .map_err(failed("read an admission"))?;
    out.item
        .map(|item| {
            Ok(AdmissionRecord {
                until: get_n(&item, "spaces", "until")?,
                spaces: get_n(&item, "spaces", "spaces")?,
                key: get_id(&item, "spaces", "admission_key")?,
                created: get_n(&item, "spaces", "created")?,
            })
        })
        .transpose()
}

/// The update that raises the identity's count by one, on the condition
/// that it has an admission and its count is below its spaces. It is an
/// `ADD`, not `SET created = created + 1`: DynamoDB refuses the latter with
/// a `ValidationException`, before it checks the condition, when the
/// identity has no item, where the append must answer `AdmissionSpent`.
pub fn count(aws: &Aws, identity: [u8; 32]) -> Result<TransactWriteItem, StoreError> {
    let update = Update::builder()
        .table_name(&aws.names.spaces)
        .key("space", b(&admission_key(identity)))
        .update_expression("ADD #created :one")
        .condition_expression("attribute_exists(#created) AND #created < #spaces")
        .expression_attribute_names("#created", "created")
        .expression_attribute_names("#spaces", "spaces")
        .expression_attribute_values(":one", n(1))
        .build()
        .map_err(|e| StoreError(format!("append: a transaction item: {e}")))?;
    Ok(TransactWriteItem::builder().update(update).build())
}
