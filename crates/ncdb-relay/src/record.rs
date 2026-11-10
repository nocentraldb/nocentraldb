//! A member's row of the membership table (RFC 0007, "The backend
//! interface"): what [`ncdb_core::membership::resolve`] says about one
//! member of a space, encoded by the core, so that a backend stores opaque
//! bytes.

use std::collections::{BTreeMap, BTreeSet};

use ncdb_core::change::Header;
use ncdb_core::ids::ChangeId;
use ncdb_core::membership::{MemberState, Resolved};

use crate::store::MemberRecord;

/// A member's row, decoded: its state and, for each of its accepted
/// grants, the accepted revocations the grant saw.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemberRow {
    /// Its keys, certificate, role, grants and revocations.
    pub state: MemberState,
    /// For each of its accepted grants, the accepted revocations it saw.
    pub seen: BTreeMap<ChangeId, BTreeSet<ChangeId>>,
}

/// Why a stored row does not decode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecordError;

/// The rows of every member, current or former, of a resolved space.
pub fn records(_resolved: &Resolved) -> Vec<MemberRecord> {
    Vec::new()
}

/// Decodes a stored row.
pub fn decode(_record: &MemberRecord) -> Result<MemberRow, RecordError> {
    Err(RecordError)
}

impl MemberRow {
    /// RFC 0008's membership verdict for a sealed change of this member:
    /// the core's [`MemberState::admits`].
    pub fn admits(&self, header: &Header) -> bool {
        self.state.admits(header, &self.seen)
    }
}
