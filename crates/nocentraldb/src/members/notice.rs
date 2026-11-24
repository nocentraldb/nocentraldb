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
    /// An erasure in a space with one account is provisional: the space
    /// status gives it on every device of the account, not a call
    /// (Carsten, 2026-11-11, NOC-230).
    ErasurePending,
    /// The account's new device was added to a space, from
    /// `Account::pair`, once per space (Carsten,
    /// 2026-11-12, NOC-273).
    DeviceAdded,
    /// This device now holds the account key, from
    /// `Account::complete_pairing`.
    DevicePaired,
    /// This agent acts for the account, from
    /// `Account::complete_pairing`.
    AgentPaired,
    /// The database's own account was linked into the one it paired with,
    /// from `Account::complete_pairing`.
    AccountLinked,
    /// The account was recovered from its code on this device.
    AccountRecovered,
    /// The account has a new account key, from removing a lost member.
    AccountKeyRenewed,
    /// An ad-hoc space was created for a sub-agent, from
    /// `Spaces::create_ad_hoc` (Carsten, 2026-11-15, NOC-320 Q10).
    AdHocSpaceCreated,
    /// A person was erased in the spaces of this database, from
    /// `Db::erase` (Carsten, 2026-11-15, on NOC-326, Q6).
    Erased,
    /// A provisional erasure was taken back, from
    /// `space.erasures().take_back` (Carsten, 2026-11-15, on NOC-326, Q7).
    TakenBack,
    /// Objects were moved to another space, from `Space::move_objects`
    /// (RFC 0016, "The SDK call and its notice").
    ObjectMoved,
}

impl Notice {
    /// The notices of accounts, with the texts the owner decided (Carsten,
    /// 2026-11-12, NOC-273; RFC 0009, "Accounts and devices").
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) const DEVICE_ADDED: Notice = Notice {
        code: NoticeCode::DeviceAdded,
        text: crate::texts::notice::DEVICE_ADDED,
    };
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) const DEVICE_PAIRED: Notice = Notice {
        code: NoticeCode::DevicePaired,
        text: crate::texts::notice::DEVICE_PAIRED,
    };
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) const AGENT_PAIRED: Notice = Notice {
        code: NoticeCode::AgentPaired,
        text: crate::texts::notice::AGENT_PAIRED,
    };
    pub(crate) const ACCOUNT_LINKED: Notice = Notice {
        code: NoticeCode::AccountLinked,
        text: crate::texts::notice::ACCOUNT_LINKED,
    };
    pub(crate) const ACCOUNT_RECOVERED: Notice = Notice {
        code: NoticeCode::AccountRecovered,
        text: crate::texts::notice::ACCOUNT_RECOVERED,
    };
    pub(crate) const ACCOUNT_KEY_RENEWED: Notice = Notice {
        code: NoticeCode::AccountKeyRenewed,
        text: crate::texts::notice::ACCOUNT_KEY_RENEWED,
    };

    /// The notice of [`Spaces::create_ad_hoc`](crate::Spaces::create_ad_hoc).
    pub(crate) const AD_HOC_SPACE_CREATED: Notice = Notice {
        code: NoticeCode::AdHocSpaceCreated,
        text: crate::texts::notice::AD_HOC_SPACE_CREATED,
    };

    /// The notice of [`Space::move_objects`](crate::Space::move_objects).
    pub(crate) const OBJECT_MOVED: Notice = Notice {
        code: NoticeCode::ObjectMoved,
        text: crate::texts::notice::OBJECT_MOVED,
    };

    /// The notice of [`Members::add`](crate::Members::add).
    pub(crate) const MEMBER_ADDED: Notice = Notice {
        code: NoticeCode::MemberAdded,
        text: crate::texts::notice::MEMBER_ADDED,
    };

    /// The notice of [`Members::remove`](crate::Members::remove), RFC
    /// 0009's text.
    pub(crate) const MEMBER_REMOVED: Notice = Notice {
        code: NoticeCode::MemberRemoved,
        text: crate::texts::notice::MEMBER_REMOVED,
    };

    /// The notice of [`Members::set_role`](crate::Members::set_role), the
    /// text the owner decided on NOC-203.
    pub(crate) const ROLE_SET: Notice = Notice {
        code: NoticeCode::RoleSet,
        text: crate::texts::notice::ROLE_SET,
    };

    /// The notice of [`Space::leave`](crate::Space::leave), the text the
    /// owner decided on NOC-219.
    pub(crate) const SPACE_LEFT: Notice = Notice {
        code: NoticeCode::SpaceLeft,
        text: crate::texts::notice::SPACE_LEFT,
    };

    /// The notice of a provisional erasure, which
    /// [`SpaceStatus::provisional_erasures`](crate::SpaceStatus) gives on
    /// every device of the account (the text: pm, 2026-11-12, NOC-234).
    pub(crate) const ERASURE_PENDING: Notice = Notice {
        code: NoticeCode::ErasurePending,
        text: crate::texts::notice::ERASURE_PENDING,
    };
}
