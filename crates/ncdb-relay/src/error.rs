//! The errors of RFC 0007 ("The errors"). An error never carries a frame,
//! a payload or any content: it names the code.

use std::fmt;

/// An error code of RFC 0007.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ErrorCode {
    /// The relay speaks none of the member's protocol versions.
    UnsupportedVersion,
    /// A request arrived before a valid `auth`, or the signature was wrong.
    Unauthenticated,
    /// The connection's member id is not a current member of the space.
    NotMember,
    /// The member was removed from the space.
    RemovedMember,
    /// The relay holds no log for this space id.
    UnknownSpace,
    /// A space-creating change names a space the relay already holds.
    SpaceExists,
    /// The member may not create spaces on this relay.
    NotAllowed,
    /// The frame is not well-formed or not canonical CBOR.
    Malformed,
    /// The frame exceeds the largest frame the relay accepts.
    TooLarge,
    /// The relay cannot check the change: it does not know its suite.
    UnsupportedSuite,
    /// The signature does not verify against the author's signing key.
    BadSignature,
    /// The author may not write the change.
    AuthorNotMember,
    /// A control change breaks the membership rules.
    MembershipRule,
    /// An erasure decision that finalizes a provisional shred arrived less
    /// than 7 days, by the relay's clock, after the relay accepted that
    /// shred (RFC 0008, "A provisional erasure in a space with one
    /// account"; Carsten, 2026-11-11, NOC-230). The member pushes it again
    /// later.
    TooEarly,
    /// The cursor belongs to an earlier relay epoch.
    EpochChanged,
    /// A person-key record the relay does not accept.
    BadRecord,
    /// The cursor points before a range moved into the archive.
    Compacted,
    /// The cursor is above the head.
    BadCursor,
    /// The member or the space exceeded its quota.
    RateLimited,
    /// The backend failed or timed out. Nothing was appended.
    Unavailable,
}

impl ErrorCode {
    /// The code as the protocol writes it.
    pub fn code(self) -> &'static str {
        match self {
            ErrorCode::UnsupportedVersion => "unsupported_version",
            ErrorCode::Unauthenticated => "unauthenticated",
            ErrorCode::NotMember => "not_member",
            ErrorCode::RemovedMember => "removed_member",
            ErrorCode::UnknownSpace => "unknown_space",
            ErrorCode::SpaceExists => "space_exists",
            ErrorCode::NotAllowed => "not_allowed",
            ErrorCode::Malformed => "malformed",
            ErrorCode::TooLarge => "too_large",
            ErrorCode::UnsupportedSuite => "unsupported_suite",
            ErrorCode::BadSignature => "bad_signature",
            ErrorCode::AuthorNotMember => "author_not_member",
            ErrorCode::MembershipRule => "membership_rule",
            ErrorCode::TooEarly => "too_early",
            ErrorCode::EpochChanged => "epoch_changed",
            ErrorCode::BadRecord => "bad_record",
            ErrorCode::Compacted => "compacted",
            ErrorCode::BadCursor => "bad_cursor",
            ErrorCode::RateLimited => "rate_limited",
            ErrorCode::Unavailable => "unavailable",
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

impl std::error::Error for ErrorCode {}
