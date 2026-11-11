//! One `entries` message of a pull or a subscription. The log is read in
//! small chunks, each at most `ncdb_relay::PAGE_BYTES` of frames, so a page
//! of large frames holds only a few of them in memory at once; an entry whose frame is larger than [`MAX_INLINE`]
//! comes as a download URL (RFC 0007, "Large changes").

use std::sync::Arc;

use ncdb_core::ids::{MemberId, SpaceId};
use ncdb_relay::store::{Epoch, Position, RelayStore};
use ncdb_relay::{ErrorCode, Relay};

use crate::connection::blocking;
use crate::http::{FRAMES, hex};
use crate::wire::{self, Carried, Item};
use crate::{MAX_INLINE, MAX_MESSAGE};

/// The most entries read from the log at once.
const CHUNK: u64 = 16;

/// An `entries` message, the position of its last entry, if any, and the
/// space's head.
pub(crate) struct Page {
    pub bytes: Vec<u8>,
    pub last: Option<Position>,
    pub head: Position,
}

/// The entries of `space` after `after` that `member` reads, at most
/// `limit` and as many as fit into one message, with `req` as its request
/// id.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn read<S: RelayStore + 'static>(
    relay: Arc<Relay<S>>,
    base_url: Arc<str>,
    member: MemberId,
    req: Option<u64>,
    space: SpaceId,
    epoch: Epoch,
    after: Position,
    limit: u64,
) -> Result<Page, ErrorCode> {
    blocking(async move {
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
                    let token = relay.begin_download(space, entry.position).await?;
                    Carried::Download {
                        url: format!("{base_url}{FRAMES}{}", hex(&token)),
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
    })
    .await
}
