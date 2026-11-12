//! RFC 0008 ("A shred is final when accepted"; the owner's NOC-224) lets
//! the relay delete a shredded person key's record as soon as it accepts
//! the shred: a shred's verdict reads only its causal past, so no later
//! change rejects it. After every append of a control change, the relay
//! deletes the stored records of every key its state names as shredded, so
//! a deletion that failed once is made by the next control change. While a
//! shredded key's record is stored, no new record of its id is accepted.

use std::collections::BTreeSet;

use ncdb_core::ids::SpaceId;

use crate::Relay;
use crate::error::ErrorCode;
use crate::store::{PersonKeyId, RelayStore};

impl<S: RelayStore> Relay<S> {
    /// Deletes the stored records of `shredded`, the person keys the
    /// space's state names as shredded. It reads the stored records only,
    /// never the log.
    pub(crate) async fn delete_shredded(
        &self,
        space: SpaceId,
        shredded: &BTreeSet<[u8; 16]>,
    ) -> Result<(), ErrorCode> {
        let ids: Vec<PersonKeyId> = self
            .store()
            .person_keys(space)
            .await?
            .into_iter()
            .filter(|record| shredded.contains(&record.id.0))
            .map(|record| record.id)
            .collect();
        if ids.is_empty() {
            return Ok(());
        }
        self.store().delete_person_keys(space, ids).await?;
        Ok(())
    }
}
