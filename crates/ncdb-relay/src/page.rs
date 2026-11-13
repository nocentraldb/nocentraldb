//! One `entries` message of a pull or a subscription. The log is read in
//! small chunks, each at most [`crate::PAGE_BYTES`] of frames, so a page
//! of large frames holds only a few of them in memory at once; an entry whose frame is larger than [`MAX_INLINE`]
//! comes as a download URL (RFC 0007, "Large changes"), which the
//! deployment makes ([`Downloads`]): from a download token on its own
//! listener, or as a presigned S3 URL on AWS.

use ncdb_core::ids::{MemberId, SpaceId};

use crate::store::{Entry, Epoch, Position, RelayStore};
use crate::wire::{self, Carried, Item, MAX_INLINE, MAX_MESSAGE};
use crate::{ErrorCode, Relay};

/// The most entries read from the log at once.
const CHUNK: u64 = 16;

/// An `entries` message, the position of its last entry, if any, and the
/// space's head.
pub struct Page {
    /// The `entries` message.
    pub bytes: Vec<u8>,
    /// The position of its last entry, none when it holds none.
    pub last: Option<Position>,
    /// The space's head.
    pub head: Position,
}

/// How a deployment hands out the URL of an entry too large to carry
/// inline (RFC 0007, "Large changes"). Its futures are `Send`.
pub trait Downloads: Sync {
    /// The URL `entry` of `space` downloads from, valid for
    /// [`crate::DOWNLOAD_URL_VALID_FOR`].
    fn url(
        &self,
        space: SpaceId,
        entry: &Entry,
    ) -> impl Future<Output = Result<String, ErrorCode>> + Send;
}

/// The download URLs of a deployment that serves download tokens on its
/// own listener: `prefix` and the token in lowercase hex, such as
/// `http://127.0.0.1:7700/frames/` and 64 hex digits.
pub struct TokenDownloads<'a, S> {
    /// The relay core, which hands out the tokens.
    pub relay: &'a Relay<S>,
    /// What every URL starts with, before the token.
    pub prefix: &'a str,
}

impl<S: RelayStore> Downloads for TokenDownloads<'_, S> {
    async fn url(&self, space: SpaceId, entry: &Entry) -> Result<String, ErrorCode> {
        let token = self.relay.begin_download(space, entry.position).await?;
        let hex: String = token.0.iter().map(|b| format!("{b:02x}")).collect();
        Ok(format!("{}{hex}", self.prefix))
    }
}

/// The entries of `space` after `after` that `member` reads, at most
/// `limit` and as many as fit into one message, with `req` as its request
/// id; `downloads` makes the URL of an entry too large to carry inline.
#[allow(clippy::too_many_arguments)]
pub async fn read<S: RelayStore, D: Downloads>(
    relay: &Relay<S>,
    downloads: &D,
    member: MemberId,
    req: Option<u64>,
    space: SpaceId,
    epoch: Epoch,
    after: Position,
    limit: u64,
) -> Result<Page, ErrorCode> {
    let mut items: Vec<Item> = Vec::new();
    let mut cursor = after;
    let mut head;
    let mut size = 0;
    loop {
        let want = CHUNK.min(limit - items.len() as u64).max(1);
        let pulled = relay
            .pull(member, space, epoch, cursor, want as u32)
            .await?;
        head = pulled.head;
        if size == 0 {
            size = wire::entries_overhead(req, space, head);
        }
        let read = pulled.entries.len() as u64;
        let mut full = false;
        for entry in pulled.entries {
            let carried = if entry.frame.len() > MAX_INLINE {
                Carried::Download {
                    url: downloads.url(space, &entry).await?,
                    size: entry.frame.len() as u64,
                }
            } else {
                Carried::Frame(entry.frame)
            };
            let item = Item {
                position: entry.position,
                accepted_at: entry.accepted_at,
                carried,
            };
            let grown = size + wire::item_size(&item);
            if !items.is_empty() && grown > MAX_MESSAGE {
                full = true;
                break;
            }
            size = grown;
            cursor = item.position;
            items.push(item);
        }
        // A pull may return fewer entries than asked, when their frames
        // reach the core's byte bound: only an empty one ends the log.
        if full || read == 0 || items.len() as u64 >= limit || cursor >= head {
            break;
        }
    }
    let (bytes, fit) = wire::entries(req, space, &items, head);
    debug_assert_eq!(fit, items.len());
    Ok(Page {
        bytes,
        last: items.last().map(|item| item.position),
        head,
    })
}
