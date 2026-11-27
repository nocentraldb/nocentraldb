//! What the rich-text tests share: documents that write real Yjs updates
//! with Yrs, as a member's editor would, and a writer of the update
//! encoding v1 for updates no editor writes.

#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::Arc;

use yrs::types::text::Text;
use yrs::types::xml::{Xml, XmlElementPrelim, XmlFragment, XmlIn, XmlOut, XmlTextPrelim};
use yrs::{Any, Doc, OffsetKind, Options, ReadTxn, StateVector, Transact, XmlFragmentRef};

/// The root a rich-text document's editor binds (RFC 0013).
pub const ROOT: &str = "default";

/// A member's document, writing under `client`, with UTF-16 offsets as
/// Yjs counts them.
pub fn doc(client: u64) -> Doc {
    let doc = Doc::with_options(Options {
        client_id: yrs::ClientID::new(client),
        offset_kind: OffsetKind::Utf16,
        ..Options::default()
    });
    let _ = doc.get_or_insert_xml_fragment(ROOT);
    doc
}

fn body(doc: &Doc) -> XmlFragmentRef {
    doc.get_or_insert_xml_fragment(ROOT)
}

/// Appends a block `tag` holding `text`; returns the update.
pub fn add_block(doc: &Doc, tag: &str, text: &str) -> Vec<u8> {
    let body = body(doc);
    let mut txn = doc.transact_mut();
    let at = body.len(&txn);
    body.insert(
        &mut txn,
        at,
        XmlElementPrelim::new(tag, [XmlIn::from(XmlTextPrelim::new(text))]),
    );
    txn.encode_update_v1()
}

/// Types `text` at `at` in block `block`; returns the update.
pub fn type_text(doc: &Doc, block: u32, at: u32, text: &str) -> Vec<u8> {
    let body = body(doc);
    let mut txn = doc.transact_mut();
    let t = text_of(&body, &txn, block);
    t.insert(&mut txn, at, text);
    txn.encode_update_v1()
}

/// Deletes `len` characters at `at` in block `block`; returns the update.
pub fn delete_text(doc: &Doc, block: u32, at: u32, len: u32) -> Vec<u8> {
    let body = body(doc);
    let mut txn = doc.transact_mut();
    let t = text_of(&body, &txn, block);
    t.remove_range(&mut txn, at, len);
    txn.encode_update_v1()
}

/// Makes `len` characters at `at` in block `block` bold; returns the update.
pub fn bold(doc: &Doc, block: u32, at: u32, len: u32) -> Vec<u8> {
    let body = body(doc);
    let mut txn = doc.transact_mut();
    let t = text_of(&body, &txn, block);
    let attrs = HashMap::from([(Arc::<str>::from("bold"), Any::Bool(true))]);
    t.format(&mut txn, at, len, attrs);
    txn.encode_update_v1()
}

/// Marks `len` characters at `at` in block `block` with the mark `name`
/// holding `attrs`, as TipTap writes a mark; returns the update.
pub fn mark(doc: &Doc, block: u32, at: u32, len: u32, name: &str, attrs: Any) -> Vec<u8> {
    let body = body(doc);
    let mut txn = doc.transact_mut();
    let t = text_of(&body, &txn, block);
    t.format(
        &mut txn,
        at,
        len,
        HashMap::from([(Arc::<str>::from(name), attrs)]),
    );
    txn.encode_update_v1()
}

/// A TipTap link's attributes: `href`, `target`, `rel` and `class`.
pub fn link(href: &str) -> Any {
    Any::from(HashMap::from([
        ("href".to_owned(), Any::from(href)),
        ("target".to_owned(), Any::from("_blank")),
        ("rel".to_owned(), Any::from("noopener noreferrer nofollow")),
        ("class".to_owned(), Any::Null),
    ]))
}

/// Sets the attribute `name` of block `block` to any value, as TipTap
/// writes a node's attribute that is no text; returns the update.
pub fn set_attribute_value(doc: &Doc, block: u32, name: &str, value: Any) -> Vec<u8> {
    let body = body(doc);
    let mut txn = doc.transact_mut();
    let Some(XmlOut::Element(el)) = body.get(&txn, block) else {
        panic!("block {block} is an element");
    };
    el.insert_attribute(&mut txn, name, value);
    txn.encode_update_v1()
}

/// Sets the attribute `name` of block `block`; returns the update.
pub fn set_attribute(doc: &Doc, block: u32, name: &str, value: &str) -> Vec<u8> {
    let body = body(doc);
    let mut txn = doc.transact_mut();
    let Some(XmlOut::Element(el)) = body.get(&txn, block) else {
        panic!("block {block} is an element");
    };
    el.insert_attribute(&mut txn, name, value);
    txn.encode_update_v1()
}

/// Deletes block `block`; returns the update.
pub fn delete_block(doc: &Doc, block: u32) -> Vec<u8> {
    let body = body(doc);
    let mut txn = doc.transact_mut();
    body.remove_range(&mut txn, block, 1);
    txn.encode_update_v1()
}

/// Applies `update` to `doc`.
pub fn apply(doc: &Doc, update: &[u8]) {
    use yrs::updates::decoder::Decode;
    let update = yrs::Update::decode_v1(update).expect("an update Yrs wrote");
    doc.transact_mut()
        .apply_update(update)
        .expect("an update Yrs wrote applies");
}

/// The whole document as one update, encoding v1.
pub fn state(doc: &Doc) -> Vec<u8> {
    doc.transact()
        .encode_state_as_update_v1(&StateVector::default())
}

/// The document as XML.
pub fn xml(doc: &Doc) -> String {
    use yrs::GetString;
    body(doc).get_string(&doc.transact())
}

fn text_of(body: &XmlFragmentRef, txn: &impl ReadTxn, block: u32) -> yrs::XmlTextRef {
    match body.get(txn, block) {
        Some(XmlOut::Element(el)) => match el.get(txn, 0) {
            Some(XmlOut::Text(t)) => t,
            _ => panic!("block {block} holds text"),
        },
        _ => panic!("block {block} is an element"),
    }
}

/// A writer of lib0's encoding, for updates no editor writes.
#[derive(Default)]
pub struct Bytes(pub Vec<u8>);

impl Bytes {
    pub fn uint(mut self, mut n: u64) -> Self {
        loop {
            let byte = (n & 0x7f) as u8;
            n >>= 7;
            if n == 0 {
                self.0.push(byte);
                return self;
            }
            self.0.push(byte | 0x80);
        }
    }

    pub fn byte(mut self, b: u8) -> Self {
        self.0.push(b);
        self
    }

    pub fn string(self, s: &str) -> Self {
        let mut this = self.uint(s.len() as u64);
        this.0.extend_from_slice(s.as_bytes());
        this
    }

    pub fn raw(mut self, bytes: &[u8]) -> Self {
        self.0.extend_from_slice(bytes);
        self
    }
}

/// An update with one string item of `client` at `clock` in the root,
/// without origins, and an empty delete set.
pub fn string_item(client: u64, clock: u64, text: &str) -> Vec<u8> {
    Bytes::default()
        .uint(1)
        .uint(1)
        .uint(client)
        .uint(clock)
        .byte(4)
        .uint(1)
        .string(ROOT)
        .string(text)
        .uint(0)
        .0
}
pub mod harness;
pub mod unfolding;

/// The characters of a document's whole state, without its markup, as
/// Yrs reads them: what a reader sees, apart from formatting.
pub fn plain_text(document: &[u8]) -> String {
    use yrs::GetString;
    let doc = doc(0);
    if !document.is_empty() {
        apply(&doc, document);
    }
    let xml = body(&doc).get_string(&doc.transact());
    let mut out = String::new();
    let mut in_tag = false;
    for c in xml.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}
