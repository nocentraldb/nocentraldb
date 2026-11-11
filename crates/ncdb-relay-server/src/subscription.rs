//! One subscription (RFC 0007, "Subscribe"): catch-up pages with the
//! request id, as for a pull, until the head; then, each time the space's
//! head moves, the new entries without a request id. Every read goes
//! through the relay core's pull, so a member that is no longer a current
//! member reads nothing: its subscription ends with `removed_member`. Each
//! page waits for room in the connection's bounded queue before the next
//! is read, so a subscription reads only as fast as its member takes.

use std::sync::Arc;

use ncdb_relay::store::RelayStore;
use ncdb_relay::{ErrorCode, Relay};
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;

use crate::connection::{Out, PAGE};
use crate::page;
use crate::wire::{self, Cursor};

/// Starts a subscription of `member` from `cursor`; `head` must be watched
/// before the first read, so no append between the two is missed.
pub(crate) fn spawn<S: RelayStore + 'static>(
    relay: Arc<Relay<S>>,
    base_url: Arc<str>,
    member: ncdb_core::ids::MemberId,
    cursor: Cursor,
    head: watch::Receiver<u64>,
    out: mpsc::Sender<Out>,
) -> JoinHandle<()> {
    tokio::spawn(run(relay, base_url, member, cursor, head, out))
}

async fn run<S: RelayStore + 'static>(
    relay: Arc<Relay<S>>,
    base_url: Arc<str>,
    member: ncdb_core::ids::MemberId,
    cursor: Cursor,
    mut head: watch::Receiver<u64>,
    out: mpsc::Sender<Out>,
) {
    let Cursor {
        req, space, epoch, ..
    } = cursor;
    let mut after = cursor.after;
    let mut catching_up = true;
    loop {
        loop {
            let tag = catching_up.then_some(req);
            let (relay, base_url) = (relay.clone(), base_url.clone());
            let pulled = page::read(relay, base_url, member, tag, space, epoch, after, PAGE).await;
            let page = match pulled {
                Ok(page) => page,
                Err(code) => {
                    let code = match code {
                        ErrorCode::NotMember if !catching_up => ErrorCode::RemovedMember,
                        code => code,
                    };
                    let _ = out.send(Out::Message(wire::error(Some(req), code))).await;
                    return;
                }
            };
            let Some(last) = page.last else {
                if catching_up && out.send(Out::Message(page.bytes)).await.is_err() {
                    return;
                }
                break;
            };
            if out.send(Out::Message(page.bytes)).await.is_err() {
                return;
            }
            after = last;
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
