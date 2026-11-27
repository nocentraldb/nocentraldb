//! An agent's anchored edits in the core (RFC 0013, "An agent: anchored
//! edits", step 5): the read version and its cache, the lookup of a quote
//! in the version the agent read, the smallest change at the Yrs level,
//! and the errors `anchorNotFound` and `anchorAmbiguous`. Three rules:
//! a quote's ends fall on text, never inside markup; a block's type
//! changes only when the replacement changes it; an agent's edits for one
//! field go through one lease, in order.
#![cfg(ncdb_rich_text)]

#[path = "../rich_text_support/mod.rs"]
mod rich_text_support;

#[path = "../rich_text_read/doc.rs"]
#[allow(dead_code)]
mod doc;

mod support;

mod blocks;
mod edit;
mod lease;
mod lookup;
mod version;
