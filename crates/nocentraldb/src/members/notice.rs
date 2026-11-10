//! The notice a call that changes who can read or write returns (RFC 0009,
//! "Invitations and members", "Notices"): what the change does, and what it
//! does not do, in plain English for the developer. The SDK ships no text
//! for end users (RFC 0006, decision 14).

/// What a membership call did, from [`Members::add`](crate::Members::add).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Notice {
    /// Which notice it is, for an app that shows its own text.
    pub code: NoticeCode,
    /// The notice in plain English, for the developer.
    pub text: &'static str,
}

/// The codes of [`Notice`], one per notice, named by the call that returns
/// it. Only the codes of the calls this SDK has are listed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum NoticeCode {
    /// A member was added: it reads the whole history of the space.
    MemberAdded,
}

impl Notice {
    /// The notice of [`Members::add`](crate::Members::add).
    pub(crate) const MEMBER_ADDED: Notice = Notice {
        code: NoticeCode::MemberAdded,
        text: "The new member reads the whole history of this space.",
    };
}
