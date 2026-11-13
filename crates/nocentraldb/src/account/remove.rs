//! Removing a lost member of the account (RFC 0010, "Losing a device";
//! NOC-268), the account space's side.

use crate::db::Db;
use crate::error::{Error, ErrorCode};
use crate::id::SpaceId;
use crate::members::{MemberId, Notice};

/// What removing a lost member did.
#[derive(Debug)]
pub(crate) struct Removal {
    /// The spaces this device removed the member from, each with
    /// `MemberRemoved`'s notice.
    pub(crate) removed: Vec<(SpaceId, Notice)>,
    /// The spaces where the member is a member and no device of the
    /// account is an admin, whose admins the app asks.
    pub(crate) ask_admins: Vec<SpaceId>,
}

impl Db {
    /// Removes the lost member `member` of the account.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) async fn remove_lost(&self, _member: MemberId) -> Result<Removal, Error> {
        Err(Error::new(ErrorCode::InvalidValue, "not built yet"))
    }
}
