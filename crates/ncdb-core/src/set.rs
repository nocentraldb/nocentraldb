//! Sets: an unordered collection of distinct basic values, merged as RFC
//! 0004 ("Sets") defines it. An addition wins over a removal that did not
//! see it; a removal that lost that way stays readable as lost.

use std::collections::{BTreeMap, BTreeSet};

use crate::basic::{Kind, OutsideSchema};
use crate::cbor::Value;
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
    /// `state` holds. Refuses an element the kind does not allow.
    pub fn add(
        &self,
        _state: &State,
        _id: OperationId,
        _author: MemberId,
        _stamp: Hlc,
        _element: Value,
    ) -> Result<Edit, OutsideSchema> {
        todo!()
    }

    /// A new removal of `element`, which removes the live additions of it
    /// that `state` holds. Refuses an element the kind does not allow.
    pub fn remove(
        &self,
        _state: &State,
        _id: OperationId,
        _author: MemberId,
        _stamp: Hlc,
        _element: Value,
    ) -> Result<Edit, OutsideSchema> {
        todo!()
    }
}

impl FieldType for Set {
    type State = State;
    type Operation = Edit;
    type Read = Read;

    fn apply(&self, _state: &mut State, _op: &Edit) {
        todo!()
    }

    fn state_bytes(&self, _state: &State) -> Vec<u8> {
        todo!()
    }

    fn read(&self, _state: &State) -> Read {
        todo!()
    }

    fn outcome(&self, _state: &State, _id: OperationId) -> Option<Outcome> {
        todo!()
    }
}
