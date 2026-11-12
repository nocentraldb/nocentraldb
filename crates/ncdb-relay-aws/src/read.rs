//! Reads of a space's log: one `Query` on the space from `position >
//! after`, in order, read consistently, page after page. A frame in S3 is
//! loaded only for an entry the read returns.

use ncdb_core::change::Kind;
use ncdb_core::ids::{ChangeId, MemberId, SpaceId};
use ncdb_relay::store::{Entry, Position, StoreError};

use crate::Aws;
use crate::item::{Item, b, failed, get_b, get_id, get_n, get_s, n};

/// Which entries a read returns.
#[derive(Clone, Copy)]
pub enum Filter {
    /// Every entry.
    All,
    /// Control entries only: a sealed entry's frame is never loaded.
    Controls,
    /// Every entry, until the frames would add up to more than this many
    /// bytes; the first entry comes whatever its size.
    Bytes(u64),
}

/// Where an entry's frame is.
enum Frame {
    Inline(Vec<u8>),
    S3(String),
}

/// An entry as its `log` item holds it: every field, and where its frame
/// is.
fn row(item: &Item) -> Result<(Entry, u64, Frame), StoreError> {
    let entry = Entry {
        position: get_n(item, "log", "position")?,
        change: ChangeId(get_id(item, "log", "change")?),
        accepted_at: get_n(item, "log", "accepted_at")?,
        author: MemberId(get_id(item, "log", "author")?),
        kind: Kind(get_n(item, "log", "kind")?),
        frame: Vec::new(),
    };
    let size = get_n(item, "log", "size")?;
    let frame = match get_s(item, "frame_key") {
        Some(key) => Frame::S3(key.to_owned()),
        None => Frame::Inline(get_b(item, "log", "frame")?),
    };
    Ok((entry, size, frame))
}

pub async fn read(
    aws: &Aws,
    space: SpaceId,
    after: Position,
    limit: u32,
    filter: Filter,
) -> Result<Vec<Entry>, StoreError> {
    let mut rows: Vec<(Entry, Frame)> = Vec::new();
    let mut bytes: u64 = 0;
    let mut start: Option<Item> = None;
    let limit = limit as usize;
    'pages: while rows.len() < limit {
        let mut query = aws
            .db
            .query()
            .table_name(&aws.names.log)
            .key_condition_expression("#space = :space AND #position > :after")
            .expression_attribute_names("#space", "space")
            .expression_attribute_names("#position", "position")
            .expression_attribute_values(":space", b(&space.0))
            .expression_attribute_values(":after", n(after))
            .consistent_read(true)
            .set_exclusive_start_key(start.take());
        query = match filter {
            Filter::Controls => query
                .filter_expression("#kind = :control")
                .expression_attribute_names("#kind", "kind")
                .expression_attribute_values(":control", n(Kind::CONTROL.0)),
            Filter::All | Filter::Bytes(_) => query.limit((limit - rows.len()).min(1000) as i32),
        };
        let out = query.send().await.map_err(failed("read the log"))?;
        for item in out.items() {
            let (entry, size, frame) = row(item)?;
            if let Filter::Bytes(max) = filter {
                if !rows.is_empty() && bytes.saturating_add(size) > max {
                    break 'pages;
                }
                bytes = bytes.saturating_add(size);
            }
            rows.push((entry, frame));
            if rows.len() == limit {
                break 'pages;
            }
        }
        match out.last_evaluated_key {
            Some(key) if !key.is_empty() => start = Some(key),
            _ => break,
        }
    }
    let mut entries = Vec::with_capacity(rows.len());
    for (mut entry, frame) in rows {
        entry.frame = match frame {
            Frame::Inline(frame) => frame,
            Frame::S3(key) => load(aws, &key).await?,
        };
        entries.push(entry);
    }
    Ok(entries)
}

/// The bytes of an object in the bucket.
pub async fn load(aws: &Aws, key: &str) -> Result<Vec<u8>, StoreError> {
    let out = aws
        .s3
        .get_object()
        .bucket(&aws.names.bucket)
        .key(key)
        .send()
        .await
        .map_err(failed("read a frame from S3"))?;
    let body = out
        .body
        .collect()
        .await
        .map_err(|_| StoreError("read a frame from S3: the body broke off".into()))?;
    Ok(body.into_bytes().to_vec())
}
