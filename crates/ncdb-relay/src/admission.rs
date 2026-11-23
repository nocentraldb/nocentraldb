//! Admission tokens (RFC 0015, "How the relay takes it"): `admit` checks an
//! admission against the relay's admission keys, its own relay id and its
//! clock, and records the identity; a genesis then creates a space under
//! that admission while it holds.
//!
//! The order a genesis goes through is the allowlist, then an admission
//! that holds, then the guest's one space (pm, 2026-11-23, NOC-469). A
//! later admission replaces the earlier one and keeps its count of spaces
//! created ([`RelayStore::admit`]); the owner decides that rule on NOC-493.

use ncdb_core::admission::Admission;

use crate::error::ErrorCode;
use crate::relay::Relay;
use crate::store::{AdmissionRecord, Admitted, Effect, RelayStore};

impl<S: RelayStore> Relay<S> {
    /// `admit`: records the admission in `bytes` for its identity. Bytes
    /// that are not an admission's canonical map are `malformed`. An
    /// admission for another relay than `relay_id`, signed by no key the
    /// relay configures, or that lapsed by the relay's clock is
    /// `not_allowed` (pm, 2026-11-23, NOC-469).
    pub async fn admit(&self, relay_id: &[u8], bytes: &[u8]) -> Result<(), ErrorCode> {
        let admission = Admission::decode(bytes).map_err(|_| ErrorCode::Malformed)?;
        if admission.encode() != bytes {
            return Err(ErrorCode::Malformed);
        }
        if admission.relay_id != relay_id || lapsed(admission.until, self.now()) {
            return Err(ErrorCode::NotAllowed);
        }
        let key = self
            .config()
            .admission_keys
            .iter()
            .find(|key| admission.verify(key))
            .ok_or(ErrorCode::NotAllowed)?;
        let admitted = Admitted {
            until: admission.until,
            spaces: admission.spaces,
            key: *key,
        };
        self.store().admit(admission.identity, admitted).await?;
        Ok(())
    }

    /// The effect that creates a space under the identity's admission, if
    /// it holds: it has not lapsed, and its count is below its spaces. The
    /// append checks the count again, so two geneses at once cannot exceed
    /// it.
    pub(crate) async fn admitted_space(
        &self,
        identity: [u8; 32],
    ) -> Result<Option<Effect>, ErrorCode> {
        let record = self.store().admission(identity).await?;
        Ok(record
            .filter(|record| holds(record, self.now()))
            .map(|_| Effect::AdmittedSpace(identity)))
    }

    /// The relay's clock, in milliseconds since the Unix epoch.
    fn now(&self) -> u64 {
        (self.config().clock)()
    }
}

/// Whether an admission with this `until`, in Unix seconds, lapsed at
/// `now`, in milliseconds: it holds while the clock is below until × 1000.
fn lapsed(until: u64, now: u64) -> bool {
    now >= until.saturating_mul(1000)
}

fn holds(record: &AdmissionRecord, now: u64) -> bool {
    !lapsed(record.until, now) && record.created < record.spaces
}
