//! Rich text, the field type `ncdb/richText` at rule 1 (RFC 0013,
//! `docs/rfc/0013-rich-text.md`): one Yjs document per object and field,
//! changed by operation 5, the rich-text update, and folded with Yrs.
//!
//! It is built only with the compiler cfg `ncdb_rich_text`, which only the
//! project's own CI sets: the fold stays off in every published build until
//! its guard is complete (NOC-468). Without it, a rich-text field's
//! operations park as "unknown type" and are kept.
//!
//! The fold (RFC 0013, "Folding"): decoding checks the update's format and
//! its client ids ([`edit`]); Yrs applies it to the field's document, in
//! any order of arrival, keeping content whose dependencies have not
//! arrived as pending; the state records what each operation added and
//! deleted, from which its outcome follows ([`outcome`]). Of two operations
//! that give one item id different content, the earlier in stamp order
//! keeps it ([`RichText::fit`]).

mod client_id;
mod document;
mod edit;
mod outcome;
pub mod read;
mod state;
pub mod update;

use std::collections::BTreeSet;
use std::fmt;

pub use client_id::client_id;
pub use edit::{Edit, UPDATE};
pub use state::{Corrupt, State};

use crate::change::Operation;
use crate::field::{FieldType, Outcome};
use crate::field_op::{Encoding, Origin};
use crate::ids::OperationId;
use crate::log::OperationParkReason;
use crate::redact::Size;
use state::Record;
use update::Run;

/// The field type `ncdb/richText`, rule 1. It takes no configuration.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RichText;

/// What the app reads from a rich-text field (RFC 0013, "Reading"): the
/// document as Markdown, as plain text, as a tree and as the references
/// its mentions hold ([`read`]), the document as Yrs writes it as XML, and
/// the operations that lost.
#[derive(Clone, PartialEq)]
pub struct Read {
    /// The document as Markdown, as [`read::markdown`].
    pub markdown: String,
    /// The document as plain text, as [`read::text`].
    pub text: String,
    /// The document as ProseMirror JSON, as [`read::tree`].
    pub tree: ncdb_plugin::Data,
    /// The references its mentions hold, as [`read::mentions`].
    pub mentions: Vec<read::Mention>,
    /// The document's root fragment as XML.
    pub xml: String,
    /// The operations whose outcome is lost: `$conflicts` names the field
    /// while there is one.
    pub lost: BTreeSet<OperationId>,
}

impl fmt::Debug for Read {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Read")
            .field("markdown", &Size(self.markdown.len(), "bytes <redacted>"))
            .field("text", &Size(self.text.len(), "bytes <redacted>"))
            .field("tree", &self.tree)
            .field("mentions", &Size(self.mentions.len(), "<redacted>"))
            .field("xml", &Size(self.xml.len(), "bytes <redacted>"))
            .field("lost", &self.lost)
            .finish()
    }
}

/// Whether an operation fits a state by the rule for the same id twice
/// (RFC 0013, "The same id twice").
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fit {
    /// It gives no id the state holds different content.
    Fits,
    /// An earlier operation the state holds gives one of its ids other
    /// content: it parks as "malformed arguments".
    Later,
    /// It is earlier than these operations the state holds, which give one
    /// of its ids other content: they park, and the field folds again
    /// without them.
    Displaces(Vec<OperationId>),
}

impl RichText {
    /// Whether `edit` fits `state` by the rule for the same id twice.
    pub fn fit(&self, state: &State, edit: &Edit) -> Fit {
        let mut displaced = Vec::new();
        for (id, record) in &state.edits {
            if *id == edit.id || !conflict(&record.runs, edit.runs()) {
                continue;
            }
            if record.order(*id) < edit.order() {
                return Fit::Later;
            }
            displaced.push(*id);
        }
        match displaced.is_empty() {
            true => Fit::Fits,
            false => Fit::Displaces(displaced),
        }
    }

    /// Applies `edit` to `state`, or says why it parks: an edit that is
    /// later than one giving its ids other content, or one Yrs refuses.
    /// An edit the state holds changes nothing. One that displaces others
    /// is applied; the caller folds the field again without them.
    pub fn try_apply(&self, state: &mut State, edit: &Edit) -> Result<(), OperationParkReason> {
        if state.edits.contains_key(&edit.id) {
            return Ok(());
        }
        if self.fit(state, edit) == Fit::Later {
            return Err(OperationParkReason::MalformedArguments);
        }
        state.document = document::apply(&state.document, &edit.update)
            .map_err(|_| OperationParkReason::MalformedArguments)?;
        state.edits.insert(edit.id, Record::of(edit));
        state
            .clients
            .extend(edit.clients.iter().map(|(c, n)| (*c, *n)));
        Ok(())
    }
}

/// Whether two operations give one id different content: their runs
/// overlap, other than in equal runs of equal content.
fn conflict(held: &[Run], new: &[Run]) -> bool {
    held.iter().any(|a| {
        new.iter().any(|b| {
            a.range.client == b.range.client
                && a.range.clock < b.range.end()
                && b.range.clock < a.range.end()
                && a != b
        })
    })
}

impl FieldType for RichText {
    type State = State;
    type Operation = Edit;
    type Read = Read;

    fn apply(&self, state: &mut State, edit: &Edit) {
        // An edit that parks never enters the state.
        let _ = self.try_apply(state, edit);
    }

    fn state_bytes(&self, state: &State) -> Vec<u8> {
        state.bytes()
    }

    fn read(&self, state: &State) -> Read {
        let read::Reads {
            markdown,
            text,
            tree,
            mentions,
        } = read::all(state);
        Read {
            markdown,
            text,
            tree,
            mentions,
            xml: document::xml(&state.document),
            lost: outcome::lost(state),
        }
    }

    fn outcome(&self, state: &State, id: OperationId) -> Option<Outcome> {
        outcome::of(state, id)
    }
}

impl Encoding for RichText {
    fn arguments(&self, edit: &Edit) -> (u64, crate::cbor::Value) {
        (UPDATE, edit::arguments(edit))
    }

    fn decode(&self, operation: &Operation, origin: Origin) -> Result<Edit, OperationParkReason> {
        edit::decode(operation, origin)
    }
}
