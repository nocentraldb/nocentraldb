//! Large changes on the member's side (RFC 0007, "Large changes"): an
//! entry that comes as a download link is fetched and then folded like any
//! other, and a frame too large for one push goes up through an upload URL.
//! The relay's checks of the change are not trusted: the downloaded frame
//! goes through the same checks as an inline one ([`super::apply`]).
//!
//! A download that fails, or whose body is not of the size the link
//! declares, ends at that entry: the entries before it are folded, the
//! cursor stays before it, and the next pull asks again, with a fresh link.

use super::http;
use super::wire::{Carried, Entry, Listed};

/// The entries of `listed` with their frames, in order, up to the first
/// one whose download failed. Whether every entry came.
pub(super) async fn fetch(listed: Vec<Listed>) -> (Vec<Entry>, bool) {
    let mut entries = Vec::with_capacity(listed.len());
    for Listed { position, carried } in listed {
        let frame = match carried {
            Carried::Frame(frame) => frame,
            Carried::Download { url, size } => match http::get(&url, size).await {
                Ok(frame) => frame,
                Err(_) => return (entries, false),
            },
        };
        entries.push(Entry { position, frame });
    }
    (entries, true)
}

/// The entries of `listed` up to the first one that comes as a download
/// link, which only an asynchronous pull can fetch. Whether every entry
/// came.
pub(super) fn inline(listed: Vec<Listed>) -> (Vec<Entry>, bool) {
    let mut entries = Vec::with_capacity(listed.len());
    for Listed { position, carried } in listed {
        match carried {
            Carried::Frame(frame) => entries.push(Entry { position, frame }),
            Carried::Download { .. } => return (entries, false),
        }
    }
    (entries, true)
}

/// Puts `frame` to an upload URL.
pub(super) async fn upload(url: &str, frame: &[u8]) -> Result<(), http::Failed> {
    http::put(url, frame).await
}
