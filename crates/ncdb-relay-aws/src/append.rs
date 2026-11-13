//! An append, as RFC 0007 draws it ("Tables and keys"): one
//! `TransactWriteItems` of 2n + 1 items plus effects. The first item updates
//! the head in `spaces` on the condition that it is still the expected one;
//! each change puts its `log` item and its `changes` item, each on the
//! condition that no item has its key. A frame above
//! [`INLINE_LIMIT`](crate::INLINE_LIMIT) is written to S3 before the
//! transaction, so a `log` item never points to a missing object; the
//! orphan a failed transaction leaves goes with
//! [`sweep_orphans`](crate::AwsStore::sweep_orphans).
//!
//! A transaction a condition cancelled tells which one: the head's means
//! another append got there first, a `changes` item's that its change is a
//! duplicate. A transaction a conflict cancelled, another transaction on
//! the same items at the same moment, is sent again after a pause.

use std::collections::HashSet;

use aws_sdk_dynamodb::operation::transact_write_items::TransactWriteItemsError;
use aws_sdk_dynamodb::types::{Put, TransactWriteItem, Update};
use aws_sdk_s3::primitives::ByteStream;
use ncdb_core::ids::{ChangeId, SpaceId};
use ncdb_relay::store::{AppendOutcome, Effect, MemberRecord, NewEntry, Position, StoreError};

use crate::item::{self, ATTEMPTS, b, failed, get_n, n};
use crate::{Aws, INLINE_LIMIT, frame_key};

/// The most items DynamoDB takes in one transaction.
pub const TRANSACTION_ITEMS: usize = 100;

/// The head of a space, read consistently: none if the relay holds no such
/// space.
pub async fn head(aws: &Aws, space: SpaceId) -> Result<Option<Position>, StoreError> {
    let out = aws
        .db
        .get_item()
        .table_name(&aws.names.spaces)
        .key("space", b(&space.0))
        .consistent_read(true)
        .send()
        .await
        .map_err(failed("read a head"))?;
    match out.item {
        Some(item) => Ok(Some(get_n(&item, "spaces", "head")?)),
        None => Ok(None),
    }
}

/// The position of a change in a space's log, read consistently.
pub async fn find(
    aws: &Aws,
    space: SpaceId,
    change: ChangeId,
) -> Result<Option<Position>, StoreError> {
    let out = aws
        .db
        .get_item()
        .table_name(&aws.names.changes)
        .key("space", b(&space.0))
        .key("change", b(&change.0))
        .consistent_read(true)
        .send()
        .await
        .map_err(failed("find a change"))?;
    match out.item {
        Some(item) => Ok(Some(get_n(&item, "changes", "position")?)),
        None => Ok(None),
    }
}

/// The member rows of `effects`, the last row of each member only: the
/// effects apply in order, and one transaction writes an item once. And the
/// account whose one space this is, when the relay does not list it.
fn member_rows(effects: Vec<Effect>) -> (Vec<MemberRecord>, Option<[u8; 32]>) {
    let mut rows: Vec<MemberRecord> = Vec::new();
    let mut guest = None;
    for effect in effects {
        match effect {
            Effect::Member(record) => {
                rows.retain(|row: &MemberRecord| row.member != record.member);
                rows.push(record);
            }
            Effect::GuestSpace(account) => guest = Some(account),
        }
    }
    (rows, guest)
}

pub async fn append(
    aws: &Aws,
    space: SpaceId,
    expected_head: Position,
    entries: Vec<NewEntry>,
    effects: Vec<Effect>,
) -> Result<AppendOutcome, StoreError> {
    let (rows, guest) = member_rows(effects);
    if 2 * entries.len() + 1 + rows.len() + usize::from(guest.is_some()) > TRANSACTION_ITEMS {
        return Err(StoreError(format!(
            "an append of {} changes and {} member rows is more than one transaction holds",
            entries.len(),
            rows.len()
        )));
    }
    let mut changes = HashSet::new();
    if !entries.iter().all(|entry| changes.insert(entry.change)) {
        return Err(StoreError("an append names one change twice".into()));
    }
    for entry in entries.iter().filter(|e| e.frame.len() > INLINE_LIMIT) {
        aws.s3
            .put_object()
            .bucket(&aws.names.bucket)
            .key(frame_key(space, entry.change))
            .body(ByteStream::from(entry.frame.clone()))
            .send()
            .await
            .map_err(failed("write a frame to S3"))?;
    }
    let mut items = transaction(aws, space, expected_head, &entries, &rows)?;
    // The account's one space, when the relay does not list it: an item
    // in `spaces` under the account key, on the condition that none is
    // there (RFC 0007, "Who may create a space").
    let guest_at = items.len();
    if let Some(account) = guest {
        items.push(guest_item(aws, space, account)?);
    }
    let new_head = expected_head + entries.len() as u64;
    for attempt in 1..=ATTEMPTS {
        let result = aws
            .db
            .transact_write_items()
            .set_transact_items(Some(items.clone()))
            .send()
            .await;
        let error = match result {
            Ok(_) => return Ok(AppendOutcome::Appended { head: new_head }),
            Err(error) => error,
        };
        let codes: Vec<String> = match error.as_service_error() {
            Some(TransactWriteItemsError::TransactionCanceledException(cancelled)) => cancelled
                .cancellation_reasons()
                .iter()
                .map(|reason| reason.code().unwrap_or("None").to_owned())
                .collect(),
            Some(TransactWriteItemsError::TransactionInProgressException(_))
            | Some(TransactWriteItemsError::ProvisionedThroughputExceededException(_))
            | Some(TransactWriteItemsError::ThrottlingException(_)) => {
                item::back_off(attempt).await;
                continue;
            }
            _ => return Err(failed("append")(error)),
        };
        if let Some(outcome) = refused(aws, space, expected_head, &entries, &codes).await? {
            return Ok(outcome);
        }
        if guest.is_some()
            && codes
                .get(guest_at)
                .is_some_and(|code| code == "ConditionalCheckFailed")
        {
            return Ok(AppendOutcome::GuestSpaceTaken);
        }
        // A condition that failed but whose head or change read back does
        // not show it is retried too: the read saw a later state.
        if !codes
            .iter()
            .all(|code| retried(code) || code == "ConditionalCheckFailed")
        {
            return Err(StoreError(format!("append: cancelled for {codes:?}")));
        }
        item::back_off(attempt).await;
    }
    Err(StoreError("append: too many conflicts on the space".into()))
}

/// Whether a cancellation reason is one to send the transaction again for.
fn retried(code: &str) -> bool {
    matches!(
        code,
        "None" | "TransactionConflict" | "ThrottlingError" | "ProvisionedThroughputExceeded"
    )
}

/// What a transaction a condition cancelled means: the head moved, or a
/// change is held, at the position read back. None if no condition failed,
/// or the head or change read back does not show it, so the call tries
/// again.
async fn refused(
    aws: &Aws,
    space: SpaceId,
    expected_head: Position,
    entries: &[NewEntry],
    codes: &[String],
) -> Result<Option<AppendOutcome>, StoreError> {
    let failed_at = |i: usize| codes.get(i).is_some_and(|c| c == "ConditionalCheckFailed");
    if failed_at(0) {
        let actual = head(aws, space).await?.unwrap_or(0);
        if actual != expected_head {
            return Ok(Some(AppendOutcome::HeadMoved(actual)));
        }
        return Ok(None);
    }
    for (i, entry) in entries.iter().enumerate() {
        if failed_at(2 + 2 * i)
            && let Some(position) = find(aws, space, entry.change).await?
        {
            let change = entry.change;
            return Ok(Some(AppendOutcome::Duplicate { change, position }));
        }
    }
    Ok(None)
}

/// The item that records `space` as the one space of `account`, which the
/// relay does not list: in `spaces`, keyed by the 32-byte account key,
/// which no 16-byte space id and no 5-byte [`EPOCH_ITEM`](crate::EPOCH_ITEM)
/// can be, on the condition that the account has no such item yet.
fn guest_item(
    aws: &Aws,
    space: SpaceId,
    account: [u8; 32],
) -> Result<TransactWriteItem, StoreError> {
    let put = Put::builder()
        .table_name(&aws.names.spaces)
        .item("space", b(&account))
        .item("guest_space", b(&space.0))
        .condition_expression("attribute_not_exists(#space)")
        .expression_attribute_names("#space", "space")
        .build()
        .map_err(|e| StoreError(format!("append: a transaction item: {e}")))?;
    Ok(TransactWriteItem::builder().put(put).build())
}

/// The items of an append's transaction: the head, then the `log` and
/// `changes` item of each change in order, then the member rows.
fn transaction(
    aws: &Aws,
    space: SpaceId,
    expected_head: Position,
    entries: &[NewEntry],
    rows: &[MemberRecord],
) -> Result<Vec<TransactWriteItem>, StoreError> {
    let names = &aws.names;
    let built = |e: aws_sdk_dynamodb::error::BuildError| {
        StoreError(format!("append: a transaction item: {e}"))
    };
    let new_head = expected_head + entries.len() as u64;
    let condition = if expected_head == 0 {
        "attribute_not_exists(#head) OR #head = :expected"
    } else {
        "#head = :expected"
    };
    let head = Update::builder()
        .table_name(&names.spaces)
        .key("space", b(&space.0))
        .update_expression("SET #head = :new")
        .condition_expression(condition)
        .expression_attribute_names("#head", "head")
        .expression_attribute_values(":expected", n(expected_head))
        .expression_attribute_values(":new", n(new_head))
        .build()
        .map_err(built)?;
    let mut items = vec![TransactWriteItem::builder().update(head).build()];
    for (i, entry) in entries.iter().enumerate() {
        let position = expected_head + 1 + i as u64;
        let mut log = Put::builder()
            .table_name(&names.log)
            .item("space", b(&space.0))
            .item("position", n(position))
            .item("change", b(&entry.change.0))
            .item("accepted_at", n(entry.accepted_at))
            .item("author", b(&entry.author.0))
            .item("kind", n(entry.kind.0))
            .item("size", n(entry.frame.len() as u64))
            .condition_expression("attribute_not_exists(#position)")
            .expression_attribute_names("#position", "position");
        log = if entry.frame.len() > INLINE_LIMIT {
            let key = frame_key(space, entry.change);
            log.item("frame_key", aws_sdk_dynamodb::types::AttributeValue::S(key))
        } else {
            log.item("frame", b(&entry.frame))
        };
        let change = Put::builder()
            .table_name(&names.changes)
            .item("space", b(&space.0))
            .item("change", b(&entry.change.0))
            .item("position", n(position))
            .condition_expression("attribute_not_exists(#change)")
            .expression_attribute_names("#change", "change");
        items.push(
            TransactWriteItem::builder()
                .put(log.build().map_err(built)?)
                .build(),
        );
        items.push(
            TransactWriteItem::builder()
                .put(change.build().map_err(built)?)
                .build(),
        );
    }
    for row in rows {
        let member = Put::builder()
            .table_name(&names.members)
            .item("space", b(&space.0))
            .item("member", b(&row.member.0))
            .item(
                "current",
                aws_sdk_dynamodb::types::AttributeValue::Bool(row.current),
            )
            .item("state", b(&row.state))
            .build()
            .map_err(built)?;
        items.push(TransactWriteItem::builder().put(member).build());
    }
    Ok(items)
}

/// Deletes the objects under `frames/` last modified before `cutoff` that
/// no `changes` item holds, and returns how many: one whole pass, with no
/// budget and no cursor. The Lambda runs [`crate::sweep::run`] instead,
/// which resumes across runs (NOC-293).
pub async fn sweep_orphans(aws: &Aws, cutoff: u64) -> Result<u32, StoreError> {
    let mut deleted = 0;
    let mut pages = aws
        .s3
        .list_objects_v2()
        .bucket(&aws.names.bucket)
        .prefix("frames/")
        .into_paginator()
        .send();
    while let Some(page) = pages.next().await {
        let page = page.map_err(failed("list frames in S3"))?;
        for object in page.contents() {
            if visit(aws, object, cutoff).await? {
                deleted += 1;
            }
        }
    }
    Ok(deleted)
}

/// Deletes `object` if it is under `frames/`, was last modified before
/// `cutoff` and no `changes` item holds it; whether it deleted it.
pub(crate) async fn visit(
    aws: &Aws,
    object: &aws_sdk_s3::types::Object,
    cutoff: u64,
) -> Result<bool, StoreError> {
    let (Some(key), Some(modified)) = (object.key(), object.last_modified()) else {
        return Ok(false);
    };
    let old = modified.to_millis().is_ok_and(|ms| ms < cutoff as i64);
    let Some((space, change)) = parse_frame_key(key) else {
        return Ok(false);
    };
    if !old || find(aws, space, change).await?.is_some() {
        return Ok(false);
    }
    aws.s3
        .delete_object()
        .bucket(&aws.names.bucket)
        .key(key)
        .send()
        .await
        .map_err(failed("delete an orphan frame"))?;
    Ok(true)
}

/// The space and change of a key [`frame_key`] wrote.
fn parse_frame_key(key: &str) -> Option<(SpaceId, ChangeId)> {
    let (space, change) = key.strip_prefix("frames/")?.split_once('/')?;
    let space = SpaceId(item::unhex(space)?.try_into().ok()?);
    let change = ChangeId(item::unhex(change)?.try_into().ok()?);
    Some((space, change))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ncdb_core::ids::MemberId;

    fn member(n: u8) -> MemberId {
        MemberId([n; 32])
    }

    #[test]
    fn a_frame_key_parses_back() {
        let (space, change) = (SpaceId([0xAB; 16]), ChangeId([0x01; 32]));
        assert_eq!(
            parse_frame_key(&frame_key(space, change)),
            Some((space, change))
        );
        assert_eq!(parse_frame_key("frames/zz/01"), None);
        assert_eq!(parse_frame_key("uploads/ab"), None);
    }

    #[test]
    fn the_last_row_of_a_member_wins() {
        let row = |m: u8, state: u8| MemberRecord {
            member: member(m),
            current: true,
            state: vec![state],
        };
        let effects = vec![
            Effect::Member(row(1, 1)),
            Effect::Member(row(2, 2)),
            Effect::Member(row(1, 3)),
        ];
        assert_eq!(member_rows(effects).0, [row(2, 2), row(1, 3)]);
    }
}
