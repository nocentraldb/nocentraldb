//! One `entries` message of a pull or a subscription, read by the relay
//! core's [`ncdb_relay::page`] on a thread that may block, with download
//! URLs on this server's listener.

use std::sync::Arc;

use ncdb_core::ids::{MemberId, SpaceId};
use ncdb_relay::page::{self, Page, TokenDownloads};
use ncdb_relay::store::{Epoch, Position, TokenUrls};
use ncdb_relay::{ErrorCode, Relay};

use crate::connection::blocking;
use crate::http::FRAMES;

/// The entries of `space` after `after` that `member` reads, at most
/// `limit` and as many as fit into one message, with `req` as its request
/// id.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn read<S: TokenUrls + 'static>(
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
        let prefix = format!("{base_url}{FRAMES}");
        let downloads = TokenDownloads {
            relay: &relay,
            prefix: &prefix,
        };
        page::read(&relay, &downloads, member, req, space, epoch, after, limit).await
    })
    .await
}

/// The error that refuses `member` a read of `space` with `code`: for a
/// removed member, with the control change that removed it (RFC 0007,
/// "The errors"), read by the relay core's [`page::refusal`] on a thread
/// that may block.
pub(crate) async fn refusal<S: TokenUrls + 'static>(
    relay: Arc<Relay<S>>,
    member: MemberId,
    req: Option<u64>,
    space: SpaceId,
    code: ErrorCode,
) -> Vec<u8> {
    blocking(async move { page::refusal(&relay, member, req, space, code).await }).await
}
