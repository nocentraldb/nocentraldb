//! The harness adapter for un-folding rich text (RFC 0013, "Un-folding a
//! rejected change"): besides edits, a member writes rejections, each of an
//! earlier operation, as a late removal of its author rejects a change.
//! A copy receives both in any order. An edit rejected before or after it
//! arrives is placed as placeholders; the field folds again in stamp
//! order, the edits first and the placeholders after, whenever a
//! rejection, a placeholder or an edit displaces what the state holds.

use std::collections::{BTreeMap, BTreeSet};

use ncdb_core::field::{FieldType, Outcome as CoreOutcome};
use ncdb_core::rich_text::{Edit, Fit, Read, RichText, State};
use proptest::prelude::*;

use super::harness::{self, Body, NONCE, Raw, client, decode, edit, member_doc, operation_id};
use ncdb_harness::{Author, Generator, Mergeable, Op, OpId, Outcome};

/// One operation as the harness carries it: an edit, or the rejection of
/// an earlier operation.
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    Edit(Body),
    Reject(OpId),
}

/// How a copy un-folds: RFC 0013's rule, as the view folds it.
pub const PLACEHOLDERS: u8 = 0;
/// Broken: places a rejected edit only when it had folded it, and leaves
/// out one whose rejection arrived first.
pub const ONLY_WHAT_IT_FOLDED: u8 = 1;
/// Broken: leaves every rejected edit out, as other field types un-fold.
pub const LEAVES_OUT: u8 = 2;

/// A copy of the field with rejections, un-folding by `RULE`; the broken
/// rules must be caught (C6).
#[derive(Clone, Debug, Default)]
pub struct Unfolding<const RULE: u8 = PLACEHOLDERS> {
    received: BTreeMap<OpId, Option<Edit>>,
    rejections: BTreeSet<OpId>,
    rejected: BTreeSet<OpId>,
    parked: BTreeSet<OpId>,
    /// The rejected edits this copy folded before their rejection, which
    /// the broken rule alone keeps apart.
    folded_before: BTreeSet<OpId>,
    pub state: State,
}

impl<const RULE: u8> Unfolding<RULE> {
    fn placed(&self, id: OpId) -> bool {
        self.rejected.contains(&id)
            && match RULE {
                PLACEHOLDERS => true,
                ONLY_WHAT_IT_FOLDED => self.folded_before.contains(&id),
                _ => false,
            }
    }

    /// Folds the field again from every edit received, in stamp order, the
    /// rejected ones placed after the others.
    fn refold(&mut self) {
        self.state = State::default();
        self.parked.clear();
        let mut edits = Vec::new();
        let mut placeholders = Vec::new();
        for (id, edit) in &self.received {
            match edit {
                None => {
                    self.parked.insert(*id);
                }
                Some(edit) if self.rejected.contains(id) => {
                    if self.placed(*id) {
                        placeholders.push(edit);
                    }
                }
                Some(edit) => edits.push((*id, edit)),
            }
        }
        edits.sort_by_key(|(_, edit)| edit.order());
        placeholders.sort_by_key(|edit| edit.order());
        for (id, edit) in edits {
            if RichText.try_apply(&mut self.state, edit).is_err() {
                self.parked.insert(id);
            }
        }
        for edit in placeholders {
            RichText.place(&mut self.state, edit);
        }
    }

    /// The document's state, for a member that edits from this copy.
    pub fn document(&self) -> Vec<u8> {
        self.state.document().to_vec()
    }
}

impl<const RULE: u8> Mergeable for Unfolding<RULE> {
    type Change = Step;
    type Read = Read;

    fn apply(&mut self, op: &Op<Step>) {
        match &op.body {
            Step::Reject(target) => {
                if !self.rejections.insert(op.id) || !self.rejected.insert(*target) {
                    return;
                }
                if let Some(Some(_)) = self.received.get(target) {
                    self.folded_before.insert(*target);
                    self.refold();
                }
            }
            Step::Edit(body) => {
                if self.received.contains_key(&op.id) {
                    return;
                }
                let edit = decode(&Op {
                    id: op.id,
                    author: op.author,
                    body: body.clone(),
                });
                self.received.insert(op.id, edit.clone());
                let Some(edit) = edit else {
                    self.parked.insert(op.id);
                    return;
                };
                if self.rejected.contains(&op.id) {
                    if self.placed(op.id) {
                        match RichText.fit_placeholder(&self.state, &edit) {
                            Fit::Displaces(_) => self.refold(),
                            _ => RichText.place(&mut self.state, &edit),
                        }
                    }
                    return;
                }
                match RichText.fit(&self.state, &edit) {
                    Fit::Displaces(_) => self.refold(),
                    _ => {
                        if RichText.try_apply(&mut self.state, &edit).is_err() {
                            self.parked.insert(op.id);
                        }
                    }
                }
            }
        }
    }

    fn state_bytes(&self) -> Vec<u8> {
        RichText.state_bytes(&self.state)
    }

    fn read(&self) -> Read {
        RichText.read(&self.state)
    }

    fn outcome(&self, id: OpId) -> Outcome {
        // A rejection is no edit of the field; a rejected edit never
        // enters the state as one.
        if self.rejections.contains(&id) {
            return Outcome::Visible;
        }
        if self.parked.contains(&id) || self.rejected.contains(&id) {
            return Outcome::Parked;
        }
        match RichText.outcome(&self.state, operation_id(id)) {
            Some(CoreOutcome::Visible) => Outcome::Visible,
            Some(CoreOutcome::Replaced(by)) => {
                Outcome::Replaced(u32::from_be_bytes(by.change.0[..4].try_into().unwrap()))
            }
            Some(CoreOutcome::Lost) => Outcome::Lost,
            None => panic!("operation #{id} was received but has no outcome"),
        }
    }

    fn round_trips(&self) -> bool {
        State::from_bytes(&self.state.bytes()).is_ok_and(|read| read == self.state)
    }
}

/// What a member does: one time in four it rejects an earlier operation,
/// picked by the number; otherwise it edits.
pub type UnfoldRaw = (u8, u32, Raw);

/// Generates edits as [`harness::Edits`] does, and rejections of earlier
/// operations, whoever wrote them and whatever their author had seen.
pub struct EditsAndRejections;

impl<const RULE: u8> Generator<Unfolding<RULE>> for EditsAndRejections {
    type Raw = UnfoldRaw;

    fn raw(&self, _: ncdb_harness::Member) -> BoxedStrategy<UnfoldRaw> {
        any::<UnfoldRaw>().boxed()
    }

    fn make(&self, author: &Author<'_, Unfolding<RULE>>, (kind, pick, raw): &UnfoldRaw) -> Step {
        if kind % 4 == 0 && author.id > 0 {
            return Step::Reject(pick % author.id);
        }
        let client = client(author.member);
        let update = edit(&member_doc(client, &author.copy.document()), raw);
        let stamp = author
            .seen
            .iter()
            .filter_map(|op| match &op.body {
                Step::Edit(body) => Some(body.stamp + 1),
                Step::Reject(_) => None,
            })
            .max()
            .unwrap_or(0);
        let adds = ncdb_core::rich_text::update::read(&update)
            .map(|shape| !shape.runs.is_empty())
            .unwrap_or(true);
        Step::Edit(Body {
            stamp,
            update,
            clients: if adds { vec![(client, NONCE)] } else { vec![] },
        })
    }

    fn malformed(&self) -> Option<BoxedStrategy<Step>> {
        Some(harness::malformed().prop_map(Step::Edit).boxed())
    }
}
