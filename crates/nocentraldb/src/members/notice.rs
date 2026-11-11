//! The notice a call that changes who can read or write returns (RFC 0009,
//! "Invitations and members", "Notices"): what the change does, and what it
//! does not do, in plain English for the developer. The SDK ships no text
//! for end users (RFC 0006, "What developers and their users read").

/// What a membership call did, from [`Members::add`](crate::Members::add),
/// [`Members::remove`](crate::Members::remove),
/// [`Members::set_role`](crate::Members::set_role) or
/// [`Space::leave`](crate::Space::leave).
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
    /// A member was removed: it keeps its copy of what it received.
    MemberRemoved,
    /// A member's role was set: one whose role was lowered keeps its copy
    /// of what it received (Carsten, 2026-11-11, NOC-203).
    RoleSet,
    /// This member left the space: it keeps its copy, and until an admin
    /// rotates, the others write under a key it holds (Carsten, 2026-11-11,
    /// NOC-219).
    SpaceLeft,
}

impl Notice {
    /// The notice of [`Members::add`](crate::Members::add).
    pub(crate) const MEMBER_ADDED: Notice = Notice {
        code: NoticeCode::MemberAdded,
        text: "The new member reads the whole history of this space.",
    };

    /// The notice of [`Members::remove`](crate::Members::remove), RFC
    /// 0009's text.
    pub(crate) const MEMBER_REMOVED: Notice = Notice {
        code: NoticeCode::MemberRemoved,
        text: "The removed member keeps its copy of everything it received before now.",
    };

    /// The notice of [`Members::set_role`](crate::Members::set_role), the
    /// text the owner decided on NOC-203.
    pub(crate) const ROLE_SET: Notice = Notice {
        code: NoticeCode::RoleSet,
        text: "A member whose role was lowered keeps its copy of everything it received before now.",
    };

    /// The notice of [`Space::leave`](crate::Space::leave), the text the
    /// owner decided on NOC-219.
    pub(crate) const SPACE_LEFT: Notice = Notice {
        code: NoticeCode::SpaceLeft,
        text: "This member keeps its copy of everything it received before now. \
               Until an admin rotates the space key, the others write under a key it still holds.",
    };
}
