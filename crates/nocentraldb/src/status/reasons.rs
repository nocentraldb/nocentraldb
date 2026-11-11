//! Why a change or an operation is parked, and why a change is rejected,
//! as `space.status` counts them (RFC 0009, "Sync and status"; NOC-193).
//! The names are the reasons the RFCs give: RFC 0002's table "What a
//! member does with a change it receives", RFC 0004's "Operations a
//! member does not understand" and RFC 0005's "Data from a newer model".
//! The core tells some of them apart more finely; each of its reasons maps
//! onto the RFC's name.

use ncdb_core::log::{
    OperationParkReason as CoreOperationReason, ParkReason as CoreParkReason,
    RejectReason as CoreRejectReason,
};

/// Why a change, or one operation of a folded change, is parked: kept, and
/// not folded, until an upgrade of the app teaches the member what it
/// means.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum ParkReason {
    /// The change's suite is unknown (RFC 0002, step 2).
    UnknownSuite,
    /// The change's kind is unknown (RFC 0002, step 2).
    UnknownKind,
    /// The change's header format, or a "must understand" key of its
    /// header or payload, is unknown (RFC 0002, steps 2 and 6).
    UnknownFormat,
    /// The payload decrypts but is not a payload this core can read, or
    /// its format is unknown (RFC 0002, steps 5a and 6).
    UnknownPayloadFormat,
    /// An operation names a collection the app's lock does not know (RFC
    /// 0005, "Data from a newer model").
    UnknownCollection,
    /// An operation names a field the app's lock does not know (RFC 0005,
    /// "Data from a newer model").
    UnknownField,
    /// An operation carries an unknown "must understand" key (RFC 0002,
    /// step 7). The name waits for the owner's answer on NOC-199.
    UnknownKey,
    /// An operation's number is not one its field's type defines (RFC
    /// 0004, "Operations a member does not understand").
    UnknownOperation,
    /// An operation's arguments carry a key from 4 to 63 that it does not
    /// define (RFC 0004, "Operations a member does not understand").
    UnknownArgument,
    /// An operation's arguments are malformed (RFC 0004, "Operations a
    /// member does not understand").
    MalformedArguments,
}

/// Why a change is rejected: kept, and not folded, until a control change
/// of its space gives the member what it lacked, or forever (RFC 0002,
/// "Rejected changes").
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum RejectReason {
    /// No membership entry the member holds names the author (RFC 0002,
    /// step 3).
    UnknownAuthor,
    /// The signature does not verify (RFC 0002, step 3).
    BadSignature,
    /// The author was not a member at the change's key generation (RFC
    /// 0002, step 4).
    NotMember,
    /// The member holds no key for the change's key generation (RFC 0002,
    /// step 5).
    NoKey,
    /// The payload does not decrypt (RFC 0002, step 5).
    Undecryptable,
}

/// The public reason of a parked change. A reason the core adds later
/// maps to `None` until this SDK names it; the core and the SDK are
/// released together.
pub(crate) fn park(reason: CoreParkReason) -> Option<ParkReason> {
    Some(match reason {
        CoreParkReason::UnknownSuite => ParkReason::UnknownSuite,
        CoreParkReason::UnknownKind => ParkReason::UnknownKind,
        CoreParkReason::UnknownHeaderFormat
        | CoreParkReason::UnknownHeaderKey
        | CoreParkReason::UnknownPayloadKey => ParkReason::UnknownFormat,
        CoreParkReason::UnknownPayloadFormat => ParkReason::UnknownPayloadFormat,
        _ => return None,
    })
}

/// The public reason of a parked operation; see [`park`].
pub(crate) fn park_operation(reason: CoreOperationReason) -> Option<ParkReason> {
    Some(match reason {
        CoreOperationReason::UnknownCollection => ParkReason::UnknownCollection,
        CoreOperationReason::UnknownField => ParkReason::UnknownField,
        CoreOperationReason::UnknownKey => ParkReason::UnknownKey,
        CoreOperationReason::UnknownOperation => ParkReason::UnknownOperation,
        CoreOperationReason::UnknownArgument => ParkReason::UnknownArgument,
        CoreOperationReason::MalformedArguments => ParkReason::MalformedArguments,
        _ => return None,
    })
}

/// The public reason of a rejected change; see [`park`].
pub(crate) fn reject(reason: CoreRejectReason) -> Option<RejectReason> {
    Some(match reason {
        CoreRejectReason::UnknownAuthor => RejectReason::UnknownAuthor,
        CoreRejectReason::BadSignature => RejectReason::BadSignature,
        CoreRejectReason::NotMember => RejectReason::NotMember,
        CoreRejectReason::NoKey => RejectReason::NoKey,
        CoreRejectReason::Undecryptable => RejectReason::Undecryptable,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every reason the core has today maps to a public one.
    #[test]
    fn every_core_reason_has_a_public_name() {
        use CoreOperationReason as O;
        use CoreParkReason as P;
        use CoreRejectReason as R;
        let parked = [
            P::UnknownSuite,
            P::UnknownKind,
            P::UnknownHeaderFormat,
            P::UnknownHeaderKey,
            P::UnknownPayloadFormat,
            P::UnknownPayloadKey,
        ];
        let operations = [
            O::UnknownCollection,
            O::UnknownField,
            O::UnknownKey,
            O::UnknownOperation,
            O::UnknownArgument,
            O::MalformedArguments,
        ];
        let rejected = [
            R::UnknownAuthor,
            R::BadSignature,
            R::NotMember,
            R::NoKey,
            R::Undecryptable,
        ];
        assert!(parked.into_iter().all(|r| park(r).is_some()));
        assert!(operations.into_iter().all(|r| park_operation(r).is_some()));
        assert!(rejected.into_iter().all(|r| reject(r).is_some()));
    }
}
