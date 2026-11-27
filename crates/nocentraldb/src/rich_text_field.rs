//! A rich-text field as the Rust SDK reads it (RFC 0013, "The Rust SDK"):
//! `object.get::<RichText>("body")` gives the document as Markdown, as
//! plain text, as the tree and as the references its mentions hold, each
//! as the core renders it from the folded state, so every member reads the
//! same state the same way (RFC 0013, "Reading").
//!
//! Built only with the compiler cfg `ncdb_rich_text` (NOC-468).

use std::fmt;

use ncdb_core::field::FieldType;
use ncdb_core::rich_text::State;
use ncdb_plugin::Data;

use crate::error::Error;
use crate::id::{ObjectId, SpaceId};
use crate::value::{Reference, Repr, Value};

/// A rich-text field's document, as a read gives it. Its `Debug` output
/// shows sizes only: a document is content.
#[derive(Clone, PartialEq)]
pub struct RichText {
    markdown: String,
    plain_text: String,
    tree: Data,
    mentions: Vec<Reference>,
}

impl RichText {
    /// The document read from `state`, or `None` while nothing was written
    /// to the field: it reads unset, as any other field.
    pub(crate) fn read(state: &State) -> Option<RichText> {
        if state.document().is_empty() {
            return None;
        }
        let read = ncdb_core::rich_text::RichText.read(state);
        Some(RichText {
            markdown: read.markdown,
            plain_text: read.text,
            tree: read.tree,
            mentions: read
                .mentions
                .iter()
                .map(|mention| Reference::new(SpaceId(mention.space), ObjectId(mention.id)))
                .collect(),
        })
    }

    /// The document as Markdown: CommonMark with GitHub's tables,
    /// strikethrough and task lists, in TipTap's standard vocabulary, with
    /// an HTML tag for any other node or mark. What an agent reads.
    pub fn markdown(&self) -> &str {
        &self.markdown
    }

    /// The document as plain text, the text of each block on a line of its
    /// own, for previews; a mention reads as `@`.
    pub fn plain_text(&self) -> &str {
        &self.plain_text
    }

    /// The document as ProseMirror JSON, `{ type, attrs, content, marks,
    /// text }` under a root of type `doc`, for an app that renders the
    /// document itself. It keeps every node, attribute and mark, those
    /// outside the vocabulary too.
    pub fn tree(&self) -> &Data {
        &self.tree
    }

    /// The references the document's mentions hold, in document order. A
    /// mention holds no name: an app reads the target from its copy to
    /// show one.
    pub fn mentions(&self) -> &[Reference] {
        &self.mentions
    }

    /// The read value of a rich-text field.
    pub(crate) fn value(self) -> Value {
        Value(Repr::RichTextRead(Box::new(self)))
    }
}

impl fmt::Debug for RichText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RichText")
            .field(
                "markdown",
                &format_args!("{} bytes [redacted]", self.markdown.len()),
            )
            .field("mentions", &self.mentions.len())
            .finish()
    }
}

impl<'a> TryFrom<&'a Value> for RichText {
    type Error = Error;

    fn try_from(value: &'a Value) -> Result<RichText, Error> {
        match &value.0 {
            Repr::RichTextRead(read) => Ok((**read).clone()),
            _ => Err(crate::value::other_kind(crate::texts::part::KIND_RICH_TEXT)),
        }
    }
}
