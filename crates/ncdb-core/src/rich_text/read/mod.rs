//! The core's reads of a rich-text field (RFC 0013, "Reading"): the
//! document as a tree, read from Yrs and never through y-prosemirror's
//! conversion. The core renders it on read, from the state, so every
//! member renders the same state the same way (C7).

mod markdown;
mod node;

use ncdb_plugin::Data;

use super::State;

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
