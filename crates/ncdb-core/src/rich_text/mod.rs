//! Rich text, the field type `ncdb/richText` at rule 1 (RFC 0013,
//! `docs/rfc/0013-rich-text.md`): one Yjs document per object and field,
//! changed by operation 5, the rich-text update, and folded with Yrs.
//!
//! It is built only with the feature `rich-text`, which no published build
//! turns on: the fold stays off in every published build until its guard
//! is complete (NOC-468). Without it, a rich-text field's operations park
//! as "unknown type" and are kept.

mod client_id;

pub use client_id::client_id;
