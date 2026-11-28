//! One anchored edit made on the agent's copy of the document: the quote
//! looked up in the copy's Markdown, the top-level blocks it touches read
//! back from that Markdown and compared with what the document holds, and
//! the difference to the replacement's blocks written in one transaction.
//!
//! Only the blocks a quote touches take part, so a part elsewhere in the
//! document that the core does not read back from Markdown, such as a node
//! outside the vocabulary, does not stop the edit. In the touched blocks,
//! a tag the replacement keeps as it is stays untouched ([`kept`]). When
//! the touched blocks do not read back as what the document holds, or the
//! replacement does not keep their tags, the edit is refused: the core
//! would otherwise write more than the agent changed.

use yrs::types::xml::XmlFragment;
use yrs::{Doc, ReadTxn, StateVector, Transact, XmlFragmentRef};

use super::super::read;
use super::diff::{self, write_blocks};
use super::kept;
use super::lookup::{self, Match, Parts};
use super::parse::{Read, read};
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
            let mut touched: Vec<Old> = old[found.first..=found.last]
                .iter()
                .filter(|o| !o.block.invisible())
                .cloned()
                .collect();
            let units = kept::keep(&mut touched);
            let held: Vec<Block> = touched.iter().map(|o| o.block.clone()).collect();
            let old_tags = kept::tags(markdown).ok_or(unsupported)?;
            let new_tags = kept::tags(&replaced).ok_or(unsupported)?;
            if !kept::same(markdown, &old_tags, &replaced, &new_tags) {
                return Err(unsupported);
            }
            let read_back = read(&kept::mask(markdown, &old_tags), &units).ok_or(unsupported)?;
            if !blocks_eq(&held, &read_back.blocks) {
                return Err(unsupported);
            }
            let new = read(&kept::mask(&replaced, &new_tags), &units).ok_or(unsupported)?;
            let new = links(new, &read_back.ncdb, k)?;
            diff::blocks(&mut txn, root, &touched, &new, old[found.first].index)
                .map_err(|_| unsupported)?;
        }
        Anchored::InsertAfter { quote, markdown } => {
            let found = found(&parts, quote, k)?;
            let new = read(markdown, &[]).ok_or(unsupported)?;
            let new = links(new, &[], k)?;
            write_blocks(&mut txn, root, found.last as u32 + 1, &new).map_err(|_| unsupported)?;
        }
        Anchored::Append { markdown } => {
            let new = read(markdown, &[]).ok_or(unsupported)?;
            let new = links(new, &[], k)?;
            let end = root.len(&txn);
            write_blocks(&mut txn, root, end, &new).map_err(|_| unsupported)?;
        }
    }
    Ok(txn.encode_update_v1())
}

/// The blocks of `new`, unless it writes an `ncdb:` link that is no
/// mention's form and that `held`, the links of the Markdown it replaces,
/// does not hold: a link the document holds already does not stop an
/// edit, but the agent never writes a malformed mention as a link.
fn links(new: Read, held: &[String], edit: usize) -> Result<Vec<Block>, Error> {
    let mut held = held.to_vec();
    for href in &new.ncdb {
        match held.iter().position(|h| h == href) {
            Some(k) => {
                held.swap_remove(k);
            }
            None => return Err(Error::InvalidValue { edit }),
        }
    }
    Ok(new.blocks)
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
