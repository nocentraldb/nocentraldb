//! The core's reads of a rich-text field (RFC 0013, "Reading"): the
//! document as a tree, read from Yrs and never through y-prosemirror's
//! conversion. The core renders it on read, from the state, so every
//! member renders the same state the same way (C7).

mod markdown;
mod mention;
mod node;
mod plain;

use ncdb_plugin::Data;

use super::State;
pub use mention::Mention;
pub(crate) use node::mark_name;

/// The document as ProseMirror JSON, `{ type, attrs, content, marks, text
/// }`, under a root of type `doc`, for apps that render the document
/// themselves. Every node, attribute and mark is kept, those outside the
/// vocabulary too; attributes read in the order of their names.
pub fn tree(state: &State) -> Data {
    node::tree(&node::nodes(&state.document))
}

/// The document as Markdown: CommonMark with GitHub's tables,
/// strikethrough and task lists, in TipTap's standard vocabulary, with an
/// HTML tag for any other part. What agents read, and what `space.get`
/// returns.
pub fn markdown(state: &State) -> String {
    markdown::render(&node::nodes(&state.document))
}

/// The Markdown of each top-level block of `document`, the whole state as
/// a Yjs update, which [`markdown`] joins with a blank line.
pub(crate) fn parts(document: &[u8]) -> Vec<String> {
    markdown::parts(&node::nodes(document))
}

/// The document as plain text, the text of each block on a line of its
/// own, for previews.
pub fn text(state: &State) -> String {
    plain::render(&node::nodes(&state.document))
}

/// The references the document's mentions hold, in document order, for an
/// app that resolves names.
pub fn mentions(state: &State) -> Vec<Mention> {
    let mut out = Vec::new();
    mention::all(&node::nodes(&state.document), &mut out);
    out
}

/// The four reads of one state, from one reading of its document.
pub(crate) struct Reads {
    pub(crate) markdown: String,
    pub(crate) text: String,
    pub(crate) tree: Data,
    pub(crate) mentions: Vec<Mention>,
}

pub(crate) fn all(state: &State) -> Reads {
    let nodes = node::nodes(&state.document);
    let mut mentions = Vec::new();
    mention::all(&nodes, &mut mentions);
    Reads {
        markdown: markdown::render(&nodes),
        text: plain::render(&nodes),
        tree: node::tree(&nodes),
        mentions,
    }
}
