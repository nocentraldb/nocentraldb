//! What a member does with a change it receives (RFC 0002;
//! RFC 0006, "Folding"): decode, understand the header, verify the
//! signature, check membership, decrypt, understand the payload, fold. A
//! change stops at the first step it does not pass, and its [`State`] in
//! the log says where.
//!
//! The checks of steps 3 to 5 come in through [`Checks`] and the fold of
//! field operations through [`Fold`], so the order of the steps and the
//! states live here, and the field types (RFC 0004) plug in. Operations
//! without a field, creates and deletes, belong to the object for every
//! collection (RFC 0004, "Creating and deleting an object"), so step 7 decodes them here and
//! hands them to [`Fold::fold_object`]. A check that fails
//! says where the change stops, as a [`Stop`]. [`crate::checks`] implements
//! [`Checks`] with the signature and decryption of RFC 0003.

use std::fmt;

use crate::change::{Change, ChangeError, Header, Kind, Operation, Payload};
use crate::field_op::{self, Origin};
use crate::ids::ObjectId;
use crate::log::{
    AppendError, Appended, Log, OperationParkReason, ParkReason, ParkedOperation, RejectReason,
    State,
};
use crate::object::ObjectOperation;
use crate::tips::Opened;

/// Where a change stops when it fails a check: parked or rejected, with
/// the reason of the step that failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stop {
    /// Parked, at step 2 or 6.
    Parked(ParkReason),
    /// Rejected, at step 3, 4 or 5.
    Rejected(RejectReason),
}

impl Stop {
    /// The step of RFC 0002, "What a member does with a change it
    /// receives", whose reason this is.
    pub fn step(self) -> u8 {
        match self {
            Stop::Parked(
                ParkReason::UnknownSuite
                | ParkReason::UnknownKind
                | ParkReason::UnknownHeaderFormat
                | ParkReason::UnknownHeaderKey,
            ) => 2,
            Stop::Parked(ParkReason::UnknownPayloadFormat | ParkReason::UnknownPayloadKey) => 6,
            Stop::Rejected(RejectReason::UnknownAuthor | RejectReason::BadSignature) => 3,
            Stop::Rejected(RejectReason::NotMember) => 4,
            Stop::Rejected(RejectReason::NoKey | RejectReason::Undecryptable) => 5,
        }
    }
}

impl From<Stop> for State {
    fn from(stop: Stop) -> State {
        match stop {
            Stop::Parked(reason) => State::Parked(reason),
            Stop::Rejected(reason) => State::Rejected(reason),
        }
    }
}

/// The checks of steps 3 to 5, against the control state of the space:
/// the membership entries, key chain entries and key envelopes the member
/// holds. Their result must depend only on the change and the set of
/// control changes added, never on their order or on the time. A change
/// that passed [`Checks::verify`] keeps passing it as control changes are
/// added. [`Checks::is_member`] may turn from pass to fail, and so may
/// [`Checks::open`] when a control change replaces the key of a generation;
/// [`Checks::still_opens`] says so, and the log then un-folds the change.
pub trait Checks {
    /// Whether this member knows the suite (step 2).
    fn knows_suite(&self, suite: u64) -> bool;
    /// Verifies the author's signature (step 3).
    fn verify(&self, change: &Change) -> Result<(), Stop>;
    /// Whether the author is a member at the change's key generation (step
    /// 4).
    fn is_member(&self, change: &Change) -> bool;
    /// Decrypts a sealed change's payload (step 5) and decodes it: a
    /// payload that decrypts but is no payload map this core can read
    /// stops at step 6.
    fn open(&self, change: &Change) -> Result<Payload, Stop>;
    /// Whether a sealed change that passed step 5 still passes it, or
    /// where it stops now. Checks whose keys never change once held keep
    /// the default, which says it passes without opening the change.
    fn still_opens(&self, _change: &Change) -> Result<(), Stop> {
        Ok(())
    }
    /// Opens a change that was folded, with the key it was folded under
    /// even when a control change has replaced that key since, so that
    /// un-folding finds the operations it folded. The default is
    /// [`Checks::open`].
    fn open_folded(&self, change: &Change) -> Result<Payload, Stop> {
        self.open(change)
    }
    /// Adds a control change that passed steps 1 to 4 to the control state.
    /// Adding a control change the checks already hold changes nothing, so
    /// [`Log::receive`] can hand them every folded control change of the
    /// log first.
    fn add_control(&mut self, change: &Change);
}

/// The fold of operations (step 7): the member's model, its field types
/// and the objects of the copy, in memory or in the local view.
pub trait Fold {
    /// Whether the member's model knows the collection.
    fn knows_collection(&self, collection: &str) -> bool;
    /// Whether the member's model knows the field of the collection.
    fn knows_field(&self, collection: &str, field: &str) -> bool;
    /// Folds one operation on a field the model knows into the view, after
    /// decoding it with the field type's [`field_op::Encoding`], or says
    /// why it is parked.
    fn fold(&mut self, origin: Origin, operation: &Operation) -> Result<(), OperationParkReason>;
    /// Folds a create or a delete into the object of the copy, which
    /// exists from its first operation on.
    fn fold_object(&mut self, object: ObjectId, operation: &ObjectOperation);
    /// Forgets the state of a field of an object, as if no operation had
    /// touched it: un-folding then folds it again from the operations of
    /// the changes still folded (RFC 0006, "Un-folding").
    fn clear_field(&mut self, object: ObjectId, collection: &str, field: &str);
    /// Forgets the creates and deletes of an object, for the same reason.
    fn clear_object(&mut self, object: ObjectId);
}

/// Why a received frame was not appended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReceiveError {
    /// Step 1: the frame is not well-formed or not canonical, so it has no
    /// stable change id.
    Decode(ChangeError),
    /// The change belongs to another space.
    WrongSpace,
}

impl fmt::Display for ReceiveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReceiveError::Decode(error) => write!(f, "the frame does not decode: {error}"),
            ReceiveError::WrongSpace => AppendError::WrongSpace.fmt(f),
        }
    }
}

impl std::error::Error for ReceiveError {}

impl Log {
    /// Receives a frame: appends it, takes it through the steps and
    /// records its state, all in this call. A duplicate changes nothing.
    ///
    /// First the checks learn every control change the log holds folded,
    /// so checks that know nothing yet, such as those of a view reopened on
    /// an existing database, check against the control state of the log
    /// (RFC 0006, "Control changes and changes that wait for them").
    ///
    /// When the frame is a control change that passes, the rejected changes
    /// of the space are checked again in the same call (RFC 0002, "Rejected
    /// changes"), until no
    /// further control change passes. Then every folded sealed change whose
    /// author the control state no longer admits, such as one above the
    /// cutoff of a revocation that arrived late, is un-folded (RFC 0006,
    /// "Un-folding"). Since every check depends only on the change
    /// and the set of control changes added, the states the log ends in,
    /// and what the fold holds, depend only on the set of changes received,
    /// not on their order.
    pub fn receive(
        &mut self,
        bytes: &[u8],
        checks: &mut impl Checks,
        fold: &mut impl Fold,
    ) -> Result<Appended, ReceiveError> {
        let change = Change::decode(bytes).map_err(ReceiveError::Decode)?;
        for control in self.folded_controls() {
            checks.add_control(control);
        }
        let appended = self.append(change.clone()).map_err(|error| match error {
            AppendError::WrongSpace => ReceiveError::WrongSpace,
        })?;
        if let Appended::New(_) = appended {
            let (state, opened) = check(&change, checks, fold);
            let control_passed = is_folded_control(&change, &state);
            self.set_state_opened(change.id(), state, opened);
            if control_passed {
                self.check_rejected_again(checks, fold);
                crate::unfold::unfold(self, checks, fold);
            }
        }
        Ok(appended)
    }

    /// Checks every rejected change again, in log order, and again after any
    /// control change among them passes, since it may let others pass.
    fn check_rejected_again(&mut self, checks: &mut impl Checks, fold: &mut impl Fold) {
        loop {
            let mut control_passed = false;
            for change in self.rejected() {
                let (state, opened) = check(&change, checks, fold);
                control_passed |= is_folded_control(&change, &state);
                self.set_state_opened(change.id(), state, opened);
            }
            if !control_passed {
                return;
            }
        }
    }
}

/// Whether a change is a control change that passed its checks.
fn is_folded_control(change: &Change, state: &State) -> bool {
    change.header().kind == Kind::CONTROL && matches!(state, State::Folded { .. })
}

/// Steps 2 to 7 for one change: its state, and for a sealed change that
/// folds, what its payload gives the log's tips.
fn check(
    change: &Change,
    checks: &mut impl Checks,
    fold: &mut impl Fold,
) -> (State, Option<Opened>) {
    let header = change.header();
    if !checks.knows_suite(header.suite) {
        return (State::Parked(ParkReason::UnknownSuite), None);
    }
    if !header.kind.is_known() {
        return (State::Parked(ParkReason::UnknownKind), None);
    }
    if header.format != Header::FORMAT {
        return (State::Parked(ParkReason::UnknownHeaderFormat), None);
    }
    if header.extensions().must_understand() {
        return (State::Parked(ParkReason::UnknownHeaderKey), None);
    }
    if let Err(stop) = checks.verify(change) {
        return (stop.into(), None);
    }
    if !checks.is_member(change) {
        return (State::Rejected(RejectReason::NotMember), None);
    }
    if header.kind == Kind::CONTROL {
        // A control change stops after step 4: it has no payload to decrypt.
        checks.add_control(change);
        return (State::Folded { parked: Vec::new() }, None);
    }
    let payload = match checks.open(change) {
        Ok(payload) => payload,
        Err(stop) => return (stop.into(), None),
    };
    if payload.format != Payload::FORMAT {
        return (State::Parked(ParkReason::UnknownPayloadFormat), None);
    }
    if payload.extensions().must_understand() {
        return (State::Parked(ParkReason::UnknownPayloadKey), None);
    }
    let mut parked = Vec::new();
    for (position, operation) in (0..).zip(&payload.operations) {
        let origin = Origin {
            id: change.operation_id(position),
            author: header.author,
            stamp: payload.clock,
        };
        let folded = understand(operation, fold).and_then(|()| fold_one(operation, origin, fold));
        if let Err(reason) = folded {
            parked.push(ParkedOperation { position, reason });
        }
    }
    let opened = Opened {
        dependencies: payload.dependencies,
        clock: payload.clock,
    };
    (State::Folded { parked }, Some(opened))
}

/// Folds one operation the model knows, a create or a delete of its object
/// or a field operation, through the fold.
pub(crate) fn fold_one(
    operation: &Operation,
    origin: Origin,
    fold: &mut impl Fold,
) -> Result<(), OperationParkReason> {
    if operation.field.is_some() {
        return fold.fold(origin, operation);
    }
    let object = field_op::decode_object(operation, origin)?;
    fold.fold_object(operation.object, &object);
    Ok(())
}

/// Whether the member's model knows an operation; RFC 0002's "The atomic
/// unit" parks it on its own
/// when it does not.
pub(crate) fn understand(
    operation: &Operation,
    fold: &impl Fold,
) -> Result<(), OperationParkReason> {
    if !fold.knows_collection(&operation.collection) {
        return Err(OperationParkReason::UnknownCollection);
    }
    if let Some(field) = &operation.field
        && !fold.knows_field(&operation.collection, field)
    {
        return Err(OperationParkReason::UnknownField);
    }
    if operation.extensions().must_understand() {
        return Err(OperationParkReason::UnknownKey);
    }
    Ok(())
}
