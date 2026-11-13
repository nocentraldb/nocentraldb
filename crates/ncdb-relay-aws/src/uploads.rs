//! Uploads (RFC 0007, "Large changes"; "How large changes go to S3"). An
//! upload is an item in `uploads` under its id; its frame goes straight
//! into S3 under [`upload_key`], through a presigned PUT URL that fixes its
//! size. The backend keeps no tokens: it serves no URLs of its own, so it
//! implements [`RelayStore`](ncdb_relay::store::RelayStore) but not
//! [`TokenUrls`](ncdb_relay::store::TokenUrls) (NOC-295, NOC-303).
//!
//! Taking an upload checks that its object holds exactly the declared size,
//! then deletes the item on the condition that it is of the member and
//! space that take it, so a frame is taken once.
//!
//! Each begin sweeps: it scans `uploads` for what expired and deletes it.
//! Each item also carries a TTL attribute, `ttl`, so DynamoDB deletes what
//! no sweep reached; the bucket's lifecycle rule does the same for
//! `uploads/`.

use ncdb_core::ids::{MemberId, SpaceId};
use ncdb_relay::store::{StoreError, UPLOAD_KEPT_AFTER_EXPIRY, Upload, UploadId};

use crate::item::{Item, b, failed, get_id, get_n, n, ttl};
use crate::{Aws, upload_key};

/// Whether a conditional DynamoDB call failed on its condition.
macro_rules! condition_failed {
    ($error:expr) => {
        $error
            .as_service_error()
            .is_some_and(|e| e.is_conditional_check_failed_exception())
    };
}

/// Deletes the uploads kept long enough after their URL expired, with
/// their frames, as of `now`.
async fn sweep(aws: &Aws, now: u64) -> Result<(), StoreError> {
    let gone = now.saturating_sub(UPLOAD_KEPT_AFTER_EXPIRY);
    for item in expired(aws, &aws.names.uploads, "attribute_exists(#e)", gone).await? {
        let id = UploadId(get_id(&item, "uploads", "id")?);
        delete(aws, &aws.names.uploads, "id", &id.0).await?;
        aws.s3
            .delete_object()
            .bucket(&aws.names.bucket)
            .key(upload_key(id))
            .send()
            .await
            .map_err(failed("delete an expired upload's frame"))?;
    }
    Ok(())
}

/// The items of `table` that match `filter` and expired at or before
/// `before`.
async fn expired(
    aws: &Aws,
    table: &str,
    filter: &str,
    before: u64,
) -> Result<Vec<Item>, StoreError> {
    let scan = aws
        .db
        .scan()
        .table_name(table)
        .filter_expression(format!("{filter} AND #e <= :before"))
        .expression_attribute_names("#e", "expires_at")
        .expression_attribute_values(":before", n(before))
        .consistent_read(true);
    let mut pages = scan.into_paginator().send();
    let mut items = Vec::new();
    while let Some(page) = pages.next().await {
        let page = page.map_err(failed("sweep"))?;
        items.extend(page.items().iter().cloned());
    }
    Ok(items)
}

/// Deletes the item with one key attribute.
async fn delete(aws: &Aws, table: &str, key: &str, value: &[u8]) -> Result<(), StoreError> {
    aws.db
        .delete_item()
        .table_name(table)
        .key(key, b(value))
        .send()
        .await
        .map_err(failed("delete an expired item"))?;
    Ok(())
}

/// An item of a table with one key attribute, read consistently.
async fn get(aws: &Aws, table: &str, key: &str, value: &[u8]) -> Result<Option<Item>, StoreError> {
    let out = aws
        .db
        .get_item()
        .table_name(table)
        .key(key, b(value))
        .consistent_read(true)
        .send()
        .await
        .map_err(failed("read an upload"))?;
    Ok(out.item)
}

pub async fn begin_upload(aws: &Aws, upload: Upload, now: u64) -> Result<(), StoreError> {
    sweep(aws, now).await?;
    let kept_until = ttl(upload.expires_at.saturating_add(UPLOAD_KEPT_AFTER_EXPIRY));
    aws.db
        .put_item()
        .table_name(&aws.names.uploads)
        .item("id", b(&upload.id.0))
        .item("space", b(&upload.space.0))
        .item("member", b(&upload.member.0))
        .item("size", n(upload.size))
        .item("expires_at", n(upload.expires_at))
        .item("ttl", kept_until)
        .condition_expression("attribute_not_exists(#id)")
        .expression_attribute_names("#id", "id")
        .send()
        .await
        .map_err(failed("begin an upload"))?;
    Ok(())
}

pub async fn take_upload(
    aws: &Aws,
    space: SpaceId,
    member: MemberId,
    id: UploadId,
) -> Result<Option<Vec<u8>>, StoreError> {
    let Some(item) = get(aws, &aws.names.uploads, "id", &id.0).await? else {
        return Ok(None);
    };
    let ours = get_id::<16>(&item, "uploads", "space")? == space.0
        && get_id::<32>(&item, "uploads", "member")? == member.0;
    if !ours {
        return Ok(None);
    }
    // An upload is filled straight into S3 through a presigned PUT URL that
    // fixes its size (RFC 0007, "How large changes go to S3"; NOC-272): its
    // object then holds exactly the declared size.
    let size = get_n(&item, "uploads", "size")?;
    let head = aws
        .s3
        .head_object()
        .bucket(&aws.names.bucket)
        .key(upload_key(id))
        .send()
        .await;
    match head {
        Ok(out) if out.content_length().and_then(|n| u64::try_from(n).ok()) == Some(size) => {}
        Ok(_) => return Ok(None),
        Err(error) if error.as_service_error().is_some_and(|e| e.is_not_found()) => {
            return Ok(None);
        }
        Err(error) => return Err(failed("read an upload's object")(error)),
    }
    let got = aws
        .s3
        .get_object()
        .bucket(&aws.names.bucket)
        .key(upload_key(id))
        .send()
        .await;
    let frame = match got {
        Ok(out) => out
            .body
            .collect()
            .await
            .map_err(|_| StoreError("read an upload's frame: the body broke off".into()))?
            .into_bytes()
            .to_vec(),
        // Deleted between the check above and the read.
        Err(error) if error.as_service_error().is_some_and(|e| e.is_no_such_key()) => {
            return Ok(None);
        }
        Err(error) => return Err(failed("read an upload's frame")(error)),
    };
    if frame.len() as u64 != size {
        return Ok(None);
    }
    let taken = aws
        .db
        .delete_item()
        .table_name(&aws.names.uploads)
        .key("id", b(&id.0))
        .condition_expression("#space = :space AND #member = :member")
        .expression_attribute_names("#space", "space")
        .expression_attribute_names("#member", "member")
        .expression_attribute_values(":space", b(&space.0))
        .expression_attribute_values(":member", b(&member.0))
        .send()
        .await;
    match taken {
        Ok(_) => {}
        Err(error) if condition_failed!(error) => return Ok(None),
        Err(error) => return Err(failed("take an upload")(error)),
    }
    aws.s3
        .delete_object()
        .bucket(&aws.names.bucket)
        .key(upload_key(id))
        .send()
        .await
        .map_err(failed("delete a taken upload's frame"))?;
    Ok(Some(frame))
}
