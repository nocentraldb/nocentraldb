//! `Account::remove_member` (RFC 0009, "Accounts and devices"; RFC 0010,
//! "Losing a device"; NOC-268).

use crate::error::{Error, ErrorCode};
use crate::id::SpaceId;
use crate::members::{MemberId, Notice};
use crate::pairing::Account;

/// What [`Account::remove_member`] did.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Removed {
    /// The spaces the member was removed from, each with `MemberRemoved`'s
    /// notice.
    pub removed: Vec<(SpaceId, Notice)>,
    /// The spaces where the member is a member and no device of the
    /// account is an admin, whose admins the app asks to remove it.
    pub ask_admins: Vec<SpaceId>,
    /// `AccountKeyRenewed`.
    pub notice: Notice,
}

impl Account<'_> {
    /// Removes a lost device or an agent of the account.
    pub async fn remove_member(&self, _member: MemberId, _words: &str) -> Result<Removed, Error> {
        Err(Error::new(ErrorCode::InvalidValue, "not built yet"))
    }
}
