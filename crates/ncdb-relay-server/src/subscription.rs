//! One subscription (RFC 0007, "Subscribe"): catch-up pages with the
//! request id, as for a pull, until the head; then, each time the space's
//! head moves, the new entries without a request id. Every read goes
//! through the relay core's pull, so a member that is no longer a current
//! member reads nothing: its subscription ends with `removed_member`.

use std::sync::Arc;

use ncdb_relay::store::RelayStore;
use ncdb_relay::{ErrorCode, Relay};
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;

use crate::connection::{Out, PAGE, blocking};
use crate::wire::{self, Cursor};

/// Starts a subscription of `member` from `cursor`; `head` must be watched
/// before the first read, so no append between the two is missed.
pub(crate) fn spawn<S: RelayStore + 'static>(
    relay: Arc<Relay<S>>,
    member: ncdb_core::ids::MemberId,
    cursor: Cursor,
    head: watch::Receiver<u64>,
    out: mpsc::UnboundedSender<Out>,
) -> JoinHandle<()> {
    tokio::spawn(run(relay, member, cursor, head, out))
}

async fn run<S: RelayStore + 'static>(
    relay: Arc<Relay<S>>,
    member: ncdb_core::ids::MemberId,
    cursor: Cursor,
    mut head: watch::Receiver<u64>,
    out: mpsc::UnboundedSender<Out>,
) {
    let Cursor {
        req, space, epoch, ..
    } = cursor;
    let mut after = cursor.after;
    let mut catching_up = true;
    loop {
        loop {
            let relay = relay.clone();
            let limit = PAGE as u32;
            let pulled =
                blocking(async move { relay.pull(member, space, epoch, after, limit).await }).await;
            let page = match pulled {
                Ok(page) => page,
                Err(code) => {
                    let code = match code {
                        ErrorCode::NotMember if !catching_up => ErrorCode::RemovedMember,
                        code => code,
                    };
                    let _ = out.send(Out::Message(wire::error(Some(req), code)));
                    return;
                }
            };
            let tag = catching_up.then_some(req);
            if page.entries.is_empty() {
                if catching_up {
                    let (bytes, _) = wire::entries(tag, space, &[], page.head);
                    let _ = out.send(Out::Message(bytes));
                }
                break;
            }
            let (bytes, sent) = wire::entries(tag, space, &page.entries, page.head);
            let _ = out.send(Out::Message(bytes));
            after = page.entries[sent - 1].position;
            if after >= page.head {
                break;
            }
        }
        catching_up = false;
        if head.changed().await.is_err() {
            return;
        }
    }
}
