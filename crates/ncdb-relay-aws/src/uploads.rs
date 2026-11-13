//! Uploads and download tokens (RFC 0007, "Large changes"). An upload is an
//! item in `uploads` under its id, its token an item in `tokens` that names
//! it; its frame waits in S3 under [`upload_key`]. A download token is an
//! item in `tokens` that names an entry of the log.
//!
//! Filling an upload marks its item filled on the condition that it is not,
//! and only then writes the frame, so a frame is filled once. Taking it
//! deletes the item on the condition that it is filled and of the member
//! and space that take it, so a frame is taken once.
//!
//! Each begin sweeps: it scans both tables for what expired and deletes it.
//! Each item also carries a TTL attribute, `ttl`, so DynamoDB deletes what
//! no sweep reached; the bucket's lifecycle rule does the same for
//! `uploads/`.

use aws_sdk_dynamodb::types::{AttributeValue, Put, TransactWriteItem};
use aws_sdk_s3::primitives::ByteStream;
use ncdb_core::ids::{MemberId, SpaceId};
use ncdb_relay::store::{Position, StoreError, Token, UPLOAD_KEPT_AFTER_EXPIRY, Upload, UploadId};

use crate::item::{Item, b, failed, get_bool, get_id, get_n, n, ttl};
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
/// their tokens and frames, and the download tokens that expired, as of
/// `now`.
async fn sweep(aws: &Aws, now: u64) -> Result<(), StoreError> {
    let gone = now.saturating_sub(UPLOAD_KEPT_AFTER_EXPIRY);
    for item in expired(aws, &aws.names.uploads, "attribute_exists(#e)", gone).await? {
        let id = UploadId(get_id(&item, "uploads", "id")?);
        let token: [u8; 32] = get_id(&item, "uploads", "token")?;
        delete(aws, &aws.names.uploads, "id", &id.0).await?;
        delete(aws, &aws.names.tokens, "token", &token).await?;
        aws.s3
            .delete_object()
            .bucket(&aws.names.bucket)
            .key(upload_key(id))
            .send()
            .await
            .map_err(failed("delete an expired upload's frame"))?;
    }
    let downloads = "attribute_exists(#e) AND attribute_exists(#p)";
    for item in expired(aws, &aws.names.tokens, downloads, now).await? {
        let token: [u8; 32] = get_id(&item, "tokens", "token")?;
        delete(aws, &aws.names.tokens, "token", &token).await?;
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
    let mut scan = aws
        .db
        .scan()
        .table_name(table)
        .filter_expression(format!("{filter} AND #e <= :before"))
        .expression_attribute_names("#e", "expires_at")
        .expression_attribute_values(":before", n(before))
        .consistent_read(true);
    if filter.contains("#p") {
        scan = scan.expression_attribute_names("#p", "position");
    }
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
        .map_err(failed("read an upload or token"))?;
    Ok(out.item)
}

pub async fn begin_upload(aws: &Aws, upload: Upload, now: u64) -> Result<(), StoreError> {
    sweep(aws, now).await?;
    let kept_until = ttl(upload.expires_at.saturating_add(UPLOAD_KEPT_AFTER_EXPIRY));
    let built =
        |e: aws_sdk_dynamodb::error::BuildError| StoreError(format!("begin an upload: {e}"));
    let item = Put::builder()
        .table_name(&aws.names.uploads)
        .item("id", b(&upload.id.0))
        .item("token", b(&upload.token.0))
        .item("space", b(&upload.space.0))
        .item("member", b(&upload.member.0))
        .item("size", n(upload.size))
        .item("expires_at", n(upload.expires_at))
        .item("ttl", kept_until.clone())
        .condition_expression("attribute_not_exists(#id)")
        .expression_attribute_names("#id", "id")
        .build()
        .map_err(built)?;
    let token = Put::builder()
        .table_name(&aws.names.tokens)
        .item("token", b(&upload.token.0))
        .item("upload", b(&upload.id.0))
        .item("ttl", kept_until)
        .condition_expression("attribute_not_exists(#token)")
        .expression_attribute_names("#token", "token")
        .build()
        .map_err(built)?;
    aws.db
        .transact_write_items()
        .transact_items(TransactWriteItem::builder().put(item).build())
        .transact_items(TransactWriteItem::builder().put(token).build())
        .send()
        .await
        .map_err(failed("begin an upload"))?;
    Ok(())
}

/// The upload a token names, as its item holds it.
async fn upload_of(aws: &Aws, token: Token) -> Result<Option<Item>, StoreError> {
    let Some(item) = get(aws, &aws.names.tokens, "token", &token.0).await? else {
        return Ok(None);
    };
    let Some(AttributeValue::B(id)) = item.get("upload") else {
        return Ok(None);
    };
    get(aws, &aws.names.uploads, "id", id.as_ref()).await
}

pub async fn upload(aws: &Aws, token: Token) -> Result<Option<(Upload, bool)>, StoreError> {
    let Some(item) = upload_of(aws, token).await? else {
        return Ok(None);
    };
    let upload = Upload {
        id: UploadId(get_id(&item, "uploads", "id")?),
        token: Token(get_id(&item, "uploads", "token")?),
        space: SpaceId(get_id(&item, "uploads", "space")?),
        member: MemberId(get_id(&item, "uploads", "member")?),
        size: get_n(&item, "uploads", "size")?,
        expires_at: get_n(&item, "uploads", "expires_at")?,
    };
    Ok(Some((upload, get_bool(&item, "filled"))))
}

pub async fn fill_upload(aws: &Aws, token: Token, frame: Vec<u8>) -> Result<bool, StoreError> {
    let Some(item) = upload_of(aws, token).await? else {
        return Ok(false);
    };
    let id = UploadId(get_id(&item, "uploads", "id")?);
    let marked = aws
        .db
        .update_item()
        .table_name(&aws.names.uploads)
        .key("id", b(&id.0))
        .update_expression("SET #filled = :yes")
        .condition_expression("attribute_exists(#id) AND attribute_not_exists(#filled)")
        .expression_attribute_names("#id", "id")
        .expression_attribute_names("#filled", "filled")
        .expression_attribute_values(":yes", AttributeValue::Bool(true))
        .send()
        .await;
    match marked {
        Ok(_) => {}
        Err(error) if condition_failed!(error) => return Ok(false),
        Err(error) => return Err(failed("fill an upload")(error)),
    }
    let written = aws
        .s3
        .put_object()
        .bucket(&aws.names.bucket)
        .key(upload_key(id))
        .body(ByteStream::from(frame))
        .send()
        .await;
    if let Err(error) = written {
        // The upload is not filled after all: another fill may try again.
        let _ = aws
            .db
            .update_item()
            .table_name(&aws.names.uploads)
            .key("id", b(&id.0))
            .update_expression("REMOVE #filled")
            .expression_attribute_names("#filled", "filled")
            .send()
            .await;
        return Err(failed("write an upload's frame to S3")(error));
    }
    Ok(true)
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
    // An upload is filled through the relay's own URL (`fill_upload`), or,
    // behind API Gateway, straight into S3 through a presigned PUT URL that
    // fixes its size (RFC 0007, "How large changes go to S3"; NOC-272): its
    // object then holds exactly the declared size.
    let size = get_n(&item, "uploads", "size")?;
    if !get_bool(&item, "filled") {
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
        // Marked filled, but its frame is not written yet.
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
    let token: [u8; 32] = get_id(&item, "uploads", "token")?;
    delete(aws, &aws.names.tokens, "token", &token).await?;
    aws.s3
        .delete_object()
        .bucket(&aws.names.bucket)
        .key(upload_key(id))
        .send()
        .await
        .map_err(failed("delete a taken upload's frame"))?;
    Ok(Some(frame))
}

pub async fn begin_download(
    aws: &Aws,
    token: Token,
    space: SpaceId,
    position: Position,
    expires_at: u64,
    now: u64,
) -> Result<(), StoreError> {
    sweep(aws, now).await?;
    aws.db
        .put_item()
        .table_name(&aws.names.tokens)
        .item("token", b(&token.0))
        .item("space", b(&space.0))
        .item("position", n(position))
        .item("expires_at", n(expires_at))
        .item("ttl", ttl(expires_at))
        .send()
        .await
        .map_err(failed("begin a download"))?;
    Ok(())
}

pub async fn download(
    aws: &Aws,
    token: Token,
) -> Result<Option<(SpaceId, Position, u64)>, StoreError> {
    let Some(item) = get(aws, &aws.names.tokens, "token", &token.0).await? else {
        return Ok(None);
    };
    if !item.contains_key("position") {
        return Ok(None);
    }
    Ok(Some((
        SpaceId(get_id(&item, "tokens", "space")?),
        get_n(&item, "tokens", "position")?,
        get_n(&item, "tokens", "expires_at")?,
    )))
}
