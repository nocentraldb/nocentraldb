//! The core's reads of a rich-text field (RFC 0013, "Reading", step 3b):
//! the document as Markdown, as plain text, as a tree read from Yrs, and
//! the references its mentions hold, each a function of the state alone.
#![cfg(ncdb_rich_text)]

#[path = "../rich_text_support/mod.rs"]
mod rich_text_support;

mod doc;
mod tree;
