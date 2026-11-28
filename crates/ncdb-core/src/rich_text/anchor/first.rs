//! A first document from Markdown (RFC 0013, "The first content"): the
//! Markdown read back as blocks with the parser of the anchored edits, the
//! same reading an edit's replacement gets, and the blocks written into an
//! empty document under one client id, as one Yjs update.

use yrs::{ClientID, Doc, OffsetKind, Options, Transact};

use super::super::document::ROOT;
use super::diff::write_blocks;
use super::parse::parse;

/// The document `markdown` reads as, written into an empty document under
/// the client id `client`, as one Yjs update in encoding v1; `None` when
/// the Markdown holds something the core does not write from Markdown,
/// such as HTML or an image. The caller writes it as an operation 5 into
/// a field that holds no content.
pub fn from_markdown(markdown: &str, client: u64) -> Option<Vec<u8>> {
    let blocks = parse(markdown)?;
    let doc = Doc::with_options(Options {
        client_id: ClientID::new(client),
        offset_kind: OffsetKind::Utf16,
        ..Options::default()
    });
    let root = doc.get_or_insert_xml_fragment(ROOT);
    let mut txn = doc.transact_mut();
    write_blocks(&mut txn, &root, 0, &blocks).ok()?;
    Some(txn.encode_update_v1())
}
