//! RFC 0008 ("Effects that wait until a change is stable") deletes a
//! shredded person key's record once the shred is stable. The relay checks
//! when a shred becomes stable, but keeps the record all the same: a stable
//! shred can still be rejected later by an admin key that was not yet
//! revoked (NOC-224), and the deletion is the one effect the relay cannot
//! undo, so it waits until NOC-224 is settled (NOC-225). While a shredded
//! key's record stays stored, no new record of its id is accepted.
//!
//! Stability is the core's ([`ncdb_core::membership::Resolved::stability`]),
//! with the relay's log as the holder: an admin's run reaches as far as the
//! relay holds that admin's changes, sealed ones included, from sequence 1
//! without a gap.

use std::collections::{BTreeMap, BTreeSet};

use ncdb_core::change::Change;
use ncdb_core::ids::{MemberId, SpaceId};
use ncdb_core::membership;

use crate::Relay;
use crate::error::ErrorCode;
use crate::relay::PAGE;
use crate::store::{PersonKeyId, RelayStore};

impl<S: RelayStore> Relay<S> {
    /// The person keys whose shred is stable in the relay's log of `space`
    /// and whose record is still stored. It reads the log only while a
    /// shredded key still has a stored record. It deletes nothing: the
    /// deletion waits for NOC-224 (NOC-225).
    pub(crate) async fn stable_shreds(
        &self,
        space: SpaceId,
    ) -> Result<Vec<PersonKeyId>, ErrorCode> {
        let stored: BTreeSet<[u8; 16]> = self
            .store()
            .person_keys(space)
            .await?
            .into_iter()
            .map(|record| record.id.0)
            .collect();
        if stored.is_empty() {
            return Ok(Vec::new());
        }
        let controls = self.controls(space).await?;
        let resolved = membership::resolve(space, &controls);
        let shredded: Vec<([u8; 16], _)> = resolved
            .person_keys()
            .filter(|(id, _)| stored.contains(id))
            .filter_map(|(id, key)| key.shred.map(|shred| (id, shred)))
            .collect();
        if shredded.is_empty() {
            return Ok(Vec::new());
        }
        let runs = self.runs(space).await?;
        let stability =
            resolved.stability(&controls, |member| runs.get(&member).copied().unwrap_or(0));
        Ok(shredded
            .into_iter()
            .filter(|(_, shred)| stability.is_stable(*shred))
            .map(|(id, _)| PersonKeyId(id))
            .collect())
    }

    /// For each author in the log of `space`, the highest n such that the
    /// log holds that author's changes 1 to n.
    async fn runs(&self, space: SpaceId) -> Result<BTreeMap<MemberId, u64>, ErrorCode> {
        let mut sequences: BTreeMap<MemberId, BTreeSet<u64>> = BTreeMap::new();
        let mut after = 0;
        loop {
            let page = self.store().read(space, after, PAGE).await?;
            let Some(last) = page.last() else {
                break;
            };
            after = last.position;
            for entry in &page {
                // The relay appended only frames that decode.
                let change = Change::decode(&entry.frame).map_err(|_| ErrorCode::Unavailable)?;
                sequences
                    .entry(change.header().author)
                    .or_default()
                    .insert(change.header().sequence);
            }
        }
        Ok(sequences
            .into_iter()
            .map(|(author, held)| {
                let run = (1..).take_while(|n| held.contains(n)).last().unwrap_or(0);
                (author, run)
            })
            .collect())
    }
}
