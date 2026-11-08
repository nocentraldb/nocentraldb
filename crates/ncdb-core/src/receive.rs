//! What a member does with a change it receives (RFC 0002 Amendment 1;
//! RFC 0006, "Folding"): decode, understand the header, verify the
//! signature, check membership, decrypt, understand the payload, fold. A
//! change stops at the first step it does not pass, and its [`State`] in
//! the log says where.
//!
//! The checks of steps 3 to 5 come in through [`Checks`] and the fold of
//! operations through [`Fold`], so the order of the steps and the states
//! live here, and signatures, decryption (RFC 0003, NOC-15) and the field
//! types (RFC 0004) plug in.

use std::fmt;

use crate::change::{Change, ChangeError, Kind, Operation, Payload};
use crate::ids::OperationId;
use crate::log::{AppendError, Appended, Log, ParkReason, State};
use crate::redact::Size;

/// The result of verifying a change's signature (step 3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verified {
    /// The signature verifies under the author's signing key.
    Valid,
    /// No membership entry the member holds names the author.
    UnknownAuthor,
    /// The signature does not verify.
    BadSignature,
}

/// The result of decrypting a sealed change's payload (step 5). Its `Debug`
/// output shows the plaintext only by its length.
#[derive(Clone, PartialEq, Eq)]
pub enum Opened {
    /// The plaintext payload map.
    Plaintext(Vec<u8>),
    /// The member holds no key for the change's key generation.
    NoKey,
    /// The AEAD fails.
    Undecryptable,
}

impl fmt::Debug for Opened {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Opened::Plaintext(bytes) => f
                .debug_tuple("Plaintext")
                .field(&Size(bytes.len(), "bytes"))
                .finish(),
            Opened::NoKey => f.write_str("NoKey"),
            Opened::Undecryptable => f.write_str("Undecryptable"),
        }
    }
}

/// The checks of steps 3 to 5, against the control state of the space:
/// the membership entries, key chain entries and key envelopes the member
/// holds. Their result must depend only on the change and the set of
/// control changes added, never on their order or on the time.
pub trait Checks {
    /// Whether this member knows the suite (step 2).
    fn knows_suite(&self, suite: u64) -> bool;
    /// Verifies the author's signature (step 3).
    fn verify(&self, change: &Change) -> Verified;
    /// Whether the author is a member at the change's key generation (step
    /// 4).
    fn is_member(&self, change: &Change) -> bool;
    /// Decrypts a sealed change's payload (step 5).
    fn open(&self, change: &Change) -> Opened;
    /// Adds a control change that passed steps 1 to 4 to the control state.
    fn add_control(&mut self, change: &Change);
}

/// The fold of operations (step 7): the member's model and its field types.
pub trait Fold {
    /// Whether the member's model knows the collection.
    fn knows_collection(&self, collection: &str) -> bool;
    /// Whether the member's model knows the field of the collection.
    fn knows_field(&self, collection: &str, field: &str) -> bool;
    /// Folds one operation the model knows into the view.
    fn fold(&mut self, id: OperationId, operation: &Operation);
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
    pub fn receive(
        &mut self,
        bytes: &[u8],
        checks: &mut impl Checks,
        fold: &mut impl Fold,
    ) -> Result<Appended, ReceiveError> {
        let change = Change::decode(bytes).map_err(ReceiveError::Decode)?;
        let appended = self.append(change.clone()).map_err(|error| match error {
            AppendError::WrongSpace => ReceiveError::WrongSpace,
        })?;
        if let Appended::New(_) = appended {
            let state = check(&change, checks, fold);
            self.set_state(change.id(), state);
        }
        Ok(appended)
    }
}

/// Steps 2 to 7 for one change.
fn check(change: &Change, checks: &mut impl Checks, fold: &mut impl Fold) -> State {
    let header = change.header();
    if !checks.knows_suite(header.suite) {
        return State::Parked(ParkReason::UnknownSuite);
    }
    if !header.kind.is_known() {
        return State::Parked(ParkReason::UnknownKind);
    }
    if header.format != crate::change::Header::FORMAT {
        return State::Parked(ParkReason::UnknownHeaderFormat);
    }
    if header.extensions().must_understand() {
        return State::Parked(ParkReason::UnknownHeaderKey);
    }
    if header.kind == Kind::CONTROL {
        checks.add_control(change);
        return State::Folded { parked: Vec::new() };
    }
    let Opened::Plaintext(plaintext) = checks.open(change) else {
        todo!("steps 3 to 5")
    };
    let payload = Payload::decode(&plaintext).expect("a payload");
    for (position, operation) in (0..).zip(&payload.operations) {
        fold.fold(change.operation_id(position), operation);
    }
    State::Folded { parked: Vec::new() }
}
