//! The membership table, the person-key store and the relay epoch.
//!
//! A person-key put is one `TransactWriteItems` with one item per person
//! key, on the condition that the stored record still has the generation
//! the put replaces (RFC 0007, "Tables and keys"). Puts of one id in one
//! call apply in order: the transaction writes the last record, on the
//! condition the first put names, once the generations between them chain.
//! A put that a condition cancelled reads the stored generations back and
//! names the first put, in order, that no longer replaces its own.

use std::collections::HashMap;

use aws_sdk_dynamodb::operation::transact_write_items::TransactWriteItemsError;
use aws_sdk_dynamodb::types::{AttributeValue, Put, TransactWriteItem};
use ncdb_core::ids::{MemberId, SpaceId};
use ncdb_relay::store::{
    Epoch, MemberRecord, PersonKeyId, PersonKeyPut, PersonKeyRecord, PutOutcome, StoreError,
};

use crate::append::TRANSACTION_ITEMS;
use crate::item::{self, ATTEMPTS, b, failed, get_b, get_bool, get_id, get_n, n};
use crate::{Aws, EPOCH_ITEM};

/// The name of the global secondary index of `members` on member id.
pub const BY_MEMBER: &str = "by_member";

pub async fn member(
    aws: &Aws,
    space: SpaceId,
    member: MemberId,
) -> Result<Option<MemberRecord>, StoreError> {
    let out = aws
        .db
        .get_item()
        .table_name(&aws.names.members)
        .key("space", b(&space.0))
        .key("member", b(&member.0))
        .consistent_read(true)
        .send()
        .await
        .map_err(failed("read a member"))?;
    let Some(item) = out.item else {
        return Ok(None);
    };
    Ok(Some(MemberRecord {
        member,
        current: get_bool(&item, "current"),
        state: get_b(&item, "members", "state")?,
    }))
}

/// The spaces a member currently belongs to, in id order: a `Query` on the
/// index on member id, which sorts by space id.
pub async fn members_of(aws: &Aws, member: MemberId) -> Result<Vec<SpaceId>, StoreError> {
    let mut pages = aws
        .db
        .query()
        .table_name(&aws.names.members)
        .index_name(BY_MEMBER)
        .key_condition_expression("#member = :member")
        .filter_expression("#current = :yes")
        .expression_attribute_names("#member", "member")
        .expression_attribute_names("#current", "current")
        .expression_attribute_values(":member", b(&member.0))
        .expression_attribute_values(":yes", AttributeValue::Bool(true))
        .into_paginator()
        .send();
    let mut spaces = Vec::new();
    while let Some(page) = pages.next().await {
        let page = page.map_err(failed("read a member's spaces"))?;
        for item in page.items() {
            spaces.push(SpaceId(get_id(item, "members", "space")?));
        }
    }
    Ok(spaces)
}

/// A person-key record from its item.
fn record(item: &item::Item) -> Result<PersonKeyRecord, StoreError> {
    Ok(PersonKeyRecord {
        id: PersonKeyId(get_id(item, "person_keys", "id")?),
        generation: get_n(item, "person_keys", "generation")?,
        writer: MemberId(get_id(item, "person_keys", "writer")?),
        wrapped: get_b(item, "person_keys", "wrapped")?,
        signature: get_b(item, "person_keys", "signature")?,
    })
}

pub async fn person_keys(aws: &Aws, space: SpaceId) -> Result<Vec<PersonKeyRecord>, StoreError> {
    let mut pages = aws
        .db
        .query()
        .table_name(&aws.names.person_keys)
        .key_condition_expression("#space = :space")
        .expression_attribute_names("#space", "space")
        .expression_attribute_values(":space", b(&space.0))
        .consistent_read(true)
        .into_paginator()
        .send();
    let mut records = Vec::new();
    while let Some(page) = pages.next().await {
        let page = page.map_err(failed("read person keys"))?;
        for item in page.items() {
            records.push(record(item)?);
        }
    }
    Ok(records)
}

/// The generation of the stored record of a person key, read consistently.
async fn stored(aws: &Aws, space: SpaceId, id: PersonKeyId) -> Result<Option<u64>, StoreError> {
    let out = aws
        .db
        .get_item()
        .table_name(&aws.names.person_keys)
        .key("space", b(&space.0))
        .key("id", b(&id.0))
        .consistent_read(true)
        .send()
        .await
        .map_err(failed("read a person key"))?;
    match out.item {
        Some(item) => Ok(Some(get_n(&item, "person_keys", "generation")?)),
        None => Ok(None),
    }
}

/// The first put, in order, that does not replace the generation it names,
/// given the stored generations and the puts before it: the outcome of a
/// put that stores nothing.
async fn first_moved(
    aws: &Aws,
    space: SpaceId,
    puts: &[PersonKeyPut],
) -> Result<Option<PutOutcome>, StoreError> {
    let mut held: HashMap<PersonKeyId, Option<u64>> = HashMap::new();
    for put in puts {
        let id = put.record.id;
        let generation = match held.get(&id) {
            Some(generation) => *generation,
            None => stored(aws, space, id).await?,
        };
        if generation != put.replaces {
            return Ok(Some(PutOutcome::Moved {
                id,
                stored: generation,
            }));
        }
        held.insert(id, Some(put.record.generation));
    }
    Ok(None)
}

/// One write per person key: the last record of its id, on the condition
/// of the first put of its id. None if the puts of one id do not chain.
fn writes(puts: &[PersonKeyPut]) -> Option<Vec<(PersonKeyRecord, Option<u64>)>> {
    let mut writes: Vec<(PersonKeyRecord, Option<u64>)> = Vec::new();
    for put in puts {
        match writes.iter_mut().find(|(r, _)| r.id == put.record.id) {
            Some((last, _)) if Some(last.generation) != put.replaces => return None,
            Some((last, _)) => *last = put.record.clone(),
            None => writes.push((put.record.clone(), put.replaces)),
        }
    }
    Some(writes)
}

pub async fn put_person_keys(
    aws: &Aws,
    space: SpaceId,
    puts: Vec<PersonKeyPut>,
) -> Result<PutOutcome, StoreError> {
    let Some(writes) = writes(&puts) else {
        let moved = first_moved(aws, space, &puts).await?;
        return moved.ok_or_else(|| StoreError("put person keys: the puts do not chain".into()));
    };
    if writes.is_empty() {
        return Ok(PutOutcome::Stored);
    }
    if writes.len() > TRANSACTION_ITEMS {
        return Err(StoreError(format!(
            "put person keys: {} records are more than one transaction holds",
            writes.len()
        )));
    }
    let built = |e: aws_sdk_dynamodb::error::BuildError| {
        StoreError(format!("put person keys: a transaction item: {e}"))
    };
    let mut items = Vec::with_capacity(writes.len());
    for (record, replaces) in &writes {
        let put = Put::builder()
            .table_name(&aws.names.person_keys)
            .item("space", b(&space.0))
            .item("id", b(&record.id.0))
            .item("generation", n(record.generation))
            .item("writer", b(&record.writer.0))
            .item("wrapped", b(&record.wrapped))
            .item("signature", b(&record.signature));
        let put = match replaces {
            None => put
                .condition_expression("attribute_not_exists(#id)")
                .expression_attribute_names("#id", "id"),
            Some(generation) => put
                .condition_expression("#generation = :replaces")
                .expression_attribute_names("#generation", "generation")
                .expression_attribute_values(":replaces", n(*generation)),
        };
        items.push(
            TransactWriteItem::builder()
                .put(put.build().map_err(built)?)
                .build(),
        );
    }
    for attempt in 1..=ATTEMPTS {
        let result = aws
            .db
            .transact_write_items()
            .set_transact_items(Some(items.clone()))
            .send()
            .await;
        let error = match result {
            Ok(_) => return Ok(PutOutcome::Stored),
            Err(error) => error,
        };
        match error.as_service_error() {
            Some(TransactWriteItemsError::TransactionCanceledException(cancelled)) => {
                let condition = cancelled
                    .cancellation_reasons()
                    .iter()
                    .any(|r| r.code() == Some("ConditionalCheckFailed"));
                if condition && let Some(moved) = first_moved(aws, space, &puts).await? {
                    return Ok(moved);
                }
            }
            Some(TransactWriteItemsError::TransactionInProgressException(_))
            | Some(TransactWriteItemsError::ProvisionedThroughputExceededException(_))
            | Some(TransactWriteItemsError::ThrottlingException(_)) => {}
            _ => return Err(failed("put person keys")(error)),
        }
        item::back_off(attempt).await;
    }
    Err(StoreError("put person keys: too many conflicts".into()))
}

pub async fn delete_person_keys(
    aws: &Aws,
    space: SpaceId,
    ids: Vec<PersonKeyId>,
) -> Result<(), StoreError> {
    for id in ids {
        aws.db
            .delete_item()
            .table_name(&aws.names.person_keys)
            .key("space", b(&space.0))
            .key("id", b(&id.0))
            .send()
            .await
            .map_err(failed("delete a person key"))?;
    }
    Ok(())
}

fn random_epoch() -> Result<Epoch, StoreError> {
    let mut epoch = [0u8; 16];
    getrandom::fill(&mut epoch)
        .map_err(|_| StoreError("no secure random source for an epoch".into()))?;
    Ok(Epoch(epoch))
}

/// The relay epoch from its reserved item in `spaces`. The first call on
/// new tables draws one, on the condition that no other call drew one
/// first.
pub async fn epoch(aws: &Aws) -> Result<Epoch, StoreError> {
    loop {
        let out = aws
            .db
            .get_item()
            .table_name(&aws.names.spaces)
            .key("space", b(EPOCH_ITEM))
            .consistent_read(true)
            .send()
            .await
            .map_err(failed("read the epoch"))?;
        if let Some(item) = out.item {
            return Ok(Epoch(get_id(&item, "spaces", "epoch")?));
        }
        let drawn = random_epoch()?;
        let put = aws
            .db
            .put_item()
            .table_name(&aws.names.spaces)
            .item("space", b(EPOCH_ITEM))
            .item("epoch", b(&drawn.0))
            .condition_expression("attribute_not_exists(#space)")
            .expression_attribute_names("#space", "space")
            .send()
            .await;
        match put {
            Ok(_) => return Ok(drawn),
            Err(error)
                if error
                    .as_service_error()
                    .is_some_and(|e| e.is_conditional_check_failed_exception()) => {}
            Err(error) => return Err(failed("draw the epoch")(error)),
        }
    }
}

/// Draws a new relay epoch and stores it in the reserved item.
pub async fn new_epoch(aws: &Aws) -> Result<Epoch, StoreError> {
    let drawn = random_epoch()?;
    aws.db
        .put_item()
        .table_name(&aws.names.spaces)
        .item("space", b(EPOCH_ITEM))
        .item("epoch", b(&drawn.0))
        .send()
        .await
        .map_err(failed("draw a new epoch"))?;
    Ok(drawn)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(id: u8, generation: u64, replaces: Option<u64>) -> PersonKeyPut {
        PersonKeyPut {
            record: PersonKeyRecord {
                id: PersonKeyId([id; 16]),
                generation,
                writer: MemberId([id; 32]),
                wrapped: vec![id],
                signature: vec![id],
            },
            replaces,
        }
    }

    #[test]
    fn puts_of_one_id_become_one_write_on_the_first_condition() {
        let puts = [put(1, 1, None), put(2, 4, Some(3)), put(1, 2, Some(1))];
        let writes = writes(&puts).expect("the puts chain");
        let got: Vec<(u8, u64, Option<u64>)> = writes
            .iter()
            .map(|(r, replaces)| (r.id.0[0], r.generation, *replaces))
            .collect();
        assert_eq!(got, [(1, 2, None), (2, 4, Some(3))]);
    }

    #[test]
    fn puts_of_one_id_that_do_not_chain_write_nothing() {
        assert!(writes(&[put(1, 2, Some(1)), put(1, 3, Some(1))]).is_none());
    }
}
