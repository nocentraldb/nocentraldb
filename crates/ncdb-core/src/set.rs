//! Sets: an unordered collection of distinct basic values, merged as RFC
//! 0004 ("Sets") defines it. An addition wins over a removal that did not
//! see it; a removal that lost that way stays readable as lost.

use std::collections::{BTreeMap, BTreeSet};

use crate::basic::{Kind, OutsideSchema, Values};
use crate::cbor::{self, Value};
use crate::clock::Hlc;
use crate::field::{FieldType, Outcome};
use crate::ids::{MemberId, OperationId};

/// The set field type: a set of elements of one [`Kind`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Set {
    /// What the schema lets a member add.
    pub kind: Kind,
}

/// What an operation does to its element.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// RFC 0004's add(e, replaces): the operation's id is the addition's
    /// tag; `replaces` names the removals of the element its author's copy
    /// held.
    Add {
        /// The removals of the element the author had seen.
        replaces: BTreeSet<OperationId>,
    },
    /// RFC 0004's remove(e, observed): `observed` names the tags of the
    /// live additions of the element in its author's copy.
    Remove {
        /// The additions the removal removes.
        observed: BTreeSet<OperationId>,
    },
}

/// One operation on a set. Its `Debug` output hides the element, as
/// [`Value`]'s does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edit {
    /// The operation's id; for an addition, its tag.
    pub id: OperationId,
    /// The member that signed the change.
    pub author: MemberId,
    /// The change's clock.
    pub stamp: Hlc,
    /// The element added or removed.
    pub element: Value,
    /// Whether it adds or removes, and what it names.
    pub action: Action,
}

/// The state of one set field: every operation it received.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct State {
    edits: BTreeMap<OperationId, Edit>,
}

/// A removal that lost to an addition its author had not seen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Removal {
    /// The removal's operation id.
    pub id: OperationId,
    /// Who removed.
    pub author: MemberId,
    /// When, by its change's clock.
    pub stamp: Hlc,
}

/// One element of the set, as the app reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Element {
    /// The element as written, also when the reader's schema does not
    /// allow it.
    pub value: Value,
    /// Whether the reader's schema allows the element.
    pub known: bool,
    /// The removals of this element that lost, in stamp order.
    pub lost_removals: Vec<Removal>,
}

/// What the app reads of a set field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Read {
    /// The elements with at least one live addition, in ascending order of
    /// their canonical bytes.
    pub elements: Vec<Element>,
}

impl Set {
    /// A new addition of `element`, which replaces the removals of it that
    /// `state` holds and no addition has replaced yet. Refuses an element
    /// the kind does not allow.
    pub fn add(
        &self,
        state: &State,
        id: OperationId,
        author: MemberId,
        stamp: Hlc,
        element: Value,
    ) -> Result<Edit, OutsideSchema> {
        add(Values::Of(&self.kind), state, id, author, stamp, element)
    }

    /// A new removal of `element`, which removes the live additions of it
    /// that `state` holds. Refuses an element the kind does not allow.
    pub fn remove(
        &self,
        state: &State,
        id: OperationId,
        author: MemberId,
        stamp: Hlc,
        element: Value,
    ) -> Result<Edit, OutsideSchema> {
        remove(Values::Of(&self.kind), state, id, author, stamp, element)
    }
}

/// [`Set::add`] for a set of `values`; links add object ids with it.
pub(crate) fn add(
    values: Values<'_>,
    state: &State,
    id: OperationId,
    author: MemberId,
    stamp: Hlc,
    element: Value,
) -> Result<Edit, OutsideSchema> {
    if !values.allows(&element) {
        return Err(OutsideSchema);
    }
    let key = cbor::encode(&element);
    let readded = state.readded();
    let replaces = state
        .removals()
        .filter(|r| r.key() == key && !readded.contains(&r.id))
        .map(|r| r.id)
        .collect();
    Ok(Edit {
        id,
        author,
        stamp,
        element,
        action: Action::Add { replaces },
    })
}

/// [`Set::remove`] for a set of `values`.
pub(crate) fn remove(
    values: Values<'_>,
    state: &State,
    id: OperationId,
    author: MemberId,
    stamp: Hlc,
    element: Value,
) -> Result<Edit, OutsideSchema> {
    if !values.allows(&element) {
        return Err(OutsideSchema);
    }
    let key = cbor::encode(&element);
    let observed = state
        .live()
        .into_iter()
        .filter(|a| a.key() == key)
        .map(|a| a.id)
        .collect();
    Ok(Edit {
        id,
        author,
        stamp,
        element,
        action: Action::Remove { observed },
    })
}

impl Edit {
    /// The element's canonical bytes, by which elements are compared (O5
    /// of RFC 0004).
    fn key(&self) -> Vec<u8> {
        cbor::encode(&self.element)
    }

    /// Stamp order, as for basic values: the stamp, then the author member
    /// id, then the operation id.
    fn order(&self) -> (Hlc, MemberId, OperationId) {
        (self.stamp, self.author, self.id)
    }

    /// The ids it names: an addition's replaced removals, a removal's
    /// observed tags.
    fn names(&self) -> &BTreeSet<OperationId> {
        match &self.action {
            Action::Add { replaces } => replaces,
            Action::Remove { observed } => observed,
        }
    }

    fn is_add(&self) -> bool {
        matches!(self.action, Action::Add { .. })
    }
}

impl State {
    fn additions(&self) -> impl Iterator<Item = &Edit> {
        self.edits.values().filter(|e| e.is_add())
    }

    fn removals(&self) -> impl Iterator<Item = &Edit> {
        self.edits.values().filter(|e| !e.is_add())
    }

    /// T: the tags every received removal observed.
    fn removed(&self) -> BTreeSet<OperationId> {
        self.removals()
            .flat_map(|r| r.names().iter().copied())
            .collect()
    }

    /// The removals every received addition replaces.
    fn readded(&self) -> BTreeSet<OperationId> {
        self.additions()
            .flat_map(|a| a.names().iter().copied())
            .collect()
    }

    /// A: the additions no received removal observed, in id order.
    fn live(&self) -> Vec<&Edit> {
        let removed = self.removed();
        self.additions()
            .filter(|a| !removed.contains(&a.id))
            .collect()
    }

    /// The canonical bytes of the elements with a live addition.
    fn present(&self) -> BTreeSet<Vec<u8>> {
        self.live().into_iter().map(Edit::key).collect()
    }
}

impl FieldType for Set {
    type State = State;
    type Operation = Edit;
    type Read = Read;

    fn apply(&self, state: &mut State, op: &Edit) {
        state.insert(op);
    }

    fn state_bytes(&self, state: &State) -> Vec<u8> {
        state.bytes()
    }

    fn read(&self, state: &State) -> Read {
        read(Values::Of(&self.kind), state)
    }

    fn outcome(&self, state: &State, id: OperationId) -> Option<Outcome> {
        state.outcome(id)
    }
}

/// What every set field does alike, whatever its elements: links merge by
/// these too.
impl State {
    /// Keeps every operation it receives; the live additions and the
    /// removed tags are computed from them, so the state is a function of
    /// the set of operations, whatever their order or repeats.
    pub(crate) fn insert(&mut self, op: &Edit) {
        self.edits.entry(op.id).or_insert_with(|| op.clone());
    }

    /// The canonical CBOR (RFC 0002's profile) of every operation received,
    /// in operation id order: `[[id, author, stamp, element, action,
    /// [named id…]]…]`, where the action is 0 for an addition (naming the
    /// removals it replaces) and 1 for a removal (naming the tags it
    /// observed), and an id is `[change id, position]`. The bytes of the
    /// local view, not a wire format.
    pub(crate) fn bytes(&self) -> Vec<u8> {
        let id = |id: &OperationId| {
            Value::Array(vec![
                Value::Bytes(id.change.0.to_vec()),
                Value::Unsigned(id.position),
            ])
        };
        let edits = self
            .edits
            .values()
            .map(|e| {
                Value::Array(vec![
                    id(&e.id),
                    Value::Bytes(e.author.0.to_vec()),
                    Value::Unsigned(e.stamp.0),
                    e.element.clone(),
                    Value::Unsigned(u64::from(!e.is_add())),
                    Value::Array(e.names().iter().map(id).collect()),
                ])
            })
            .collect();
        cbor::encode(&Value::Array(edits))
    }

    /// An addition is replaced by the received removal with the lowest
    /// operation id that observed it, and visible otherwise. A removal is
    /// replaced by the received addition with the lowest operation id that
    /// names it; otherwise lost while its element is in the set, and
    /// visible (applied) when it is not.
    pub(crate) fn outcome(&self, id: OperationId) -> Option<Outcome> {
        let edit = self.edits.get(&id)?;
        let by = self
            .edits
            .values()
            .find(|e| e.is_add() != edit.is_add() && e.names().contains(&id));
        if let Some(by) = by {
            return Some(Outcome::Replaced(by.id));
        }
        Some(if !edit.is_add() && self.present().contains(&edit.key()) {
            Outcome::Lost
        } else {
            Outcome::Visible
        })
    }
}

/// The read value of a set of `values`.
pub(crate) fn read(values: Values<'_>, state: &State) -> Read {
    let readded = state.readded();
    let mut elements: BTreeMap<Vec<u8>, Element> = BTreeMap::new();
    for addition in state.live() {
        elements.entry(addition.key()).or_insert_with(|| Element {
            value: addition.element.clone(),
            known: values.allows(&addition.element),
            lost_removals: Vec::new(),
        });
    }
    let mut lost: Vec<&Edit> = state
        .removals()
        .filter(|r| !readded.contains(&r.id))
        .collect();
    lost.sort_by_key(|r| r.order());
    for removal in lost {
        if let Some(element) = elements.get_mut(&removal.key()) {
            element.lost_removals.push(Removal {
                id: removal.id,
                author: removal.author,
                stamp: removal.stamp,
            });
        }
    }
    Read {
        elements: elements.into_values().collect(),
    }
}
