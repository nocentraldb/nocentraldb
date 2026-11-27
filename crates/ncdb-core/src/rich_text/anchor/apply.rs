//! One anchored edit made on the agent's copy of the document: the quote
//! looked up in the copy's Markdown, the top-level blocks it touches read
//! back from that Markdown and compared with what the document holds, and
//! the difference to the replacement's blocks written in one transaction.
//!
//! Only the blocks a quote touches take part, so a part elsewhere in the
//! document that the core does not read back from Markdown, such as a node
//! outside the vocabulary, does not stop the edit. When the touched blocks
//! do not read back as what the document holds, the edit is refused: the
//! core would otherwise write more than the agent changed.

use yrs::types::xml::XmlFragment;
use yrs::{Doc, ReadTxn, StateVector, Transact, XmlFragmentRef};

use super::super::read;
use super::diff::{self, write_blocks};
use super::lookup::{self, Match, Parts};
use super::parse::parse;
use super::view::{self, Block, Old, blocks_eq};
use super::{Anchored, Error};

/// Makes edit `k` on `doc`; the update it wrote.
pub(super) fn one(
    doc: &Doc,
    root: &XmlFragmentRef,
    k: usize,
    edit: &Anchored,
) -> Result<Vec<u8>, Error> {
    let unsupported = Error::Unsupported { edit: k };
    let state = doc
        .transact()
        .encode_state_as_update_v1(&StateVector::default());
    let parts = Parts::new(read::parts(&state));
    let mut txn = doc.transact_mut();
    let old = view::root(&txn, root).ok_or(unsupported)?;
    if old.len() != parts.spans.len() {
        return Err(unsupported);
    }
    match edit {
        Anchored::Replace { quote, replacement } => {
            let found = found(&parts, quote, k)?;
            let region = parts.region(found.first, found.last);
            let markdown = &parts.text[region.clone()];
            let replaced = format!(
                "{}{replacement}{}",
                &markdown[..found.range.start - region.start],
                &markdown[found.range.end - region.start..]
            );
            let touched: Vec<Old> = old[found.first..=found.last]
                .iter()
                .filter(|o| !o.block.invisible())
                .cloned()
                .collect();
            let held: Vec<Block> = touched.iter().map(|o| o.block.clone()).collect();
            let read_back = parse(markdown).ok_or(unsupported)?;
            if !blocks_eq(&held, &read_back) {
                return Err(unsupported);
            }
            let new = parse(&replaced).ok_or(unsupported)?;
            diff::blocks(&mut txn, root, &touched, &new, old[found.first].index)
                .map_err(|_| unsupported)?;
        }
        Anchored::InsertAfter { quote, markdown } => {
            let found = found(&parts, quote, k)?;
            let new = parse(markdown).ok_or(unsupported)?;
            write_blocks(&mut txn, root, found.last as u32 + 1, &new).map_err(|_| unsupported)?;
        }
        Anchored::Append { markdown } => {
            let new = parse(markdown).ok_or(unsupported)?;
            let end = root.len(&txn);
            write_blocks(&mut txn, root, end, &new).map_err(|_| unsupported)?;
        }
    }
    Ok(txn.encode_update_v1())
}

/// The one occurrence of `quote`, or why there is not one.
fn found(parts: &Parts, quote: &str, edit: usize) -> Result<Match, Error> {
    let mut matches = lookup::find(parts, quote);
    match matches.len() {
        0 => Err(Error::AnchorNotFound { edit }),
        1 => Ok(matches.remove(0)),
        n => Err(Error::AnchorAmbiguous { edit, matches: n }),
    }
}
