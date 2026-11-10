//! Keys and the person-key store (RFC 0007, "Keys and the person-key
//! store"): a member reads its key envelopes and the key chain, and reads
//! and puts the space's person-key records, which the relay accepts only as
//! "What the relay checks" says.

use std::collections::BTreeMap;

use ncdb_core::change::{Change, Kind};
use ncdb_core::ids::{MemberId, SpaceId};
use ncdb_core::membership::{self, ControlBody, Entry, Resolved};

use crate::Relay;
use crate::error::ErrorCode;
use crate::relay::PAGE;
use crate::store::{Entry as LogEntry, PersonKeyRecord, RelayStore};

/// A member's keys in one space: the key envelopes addressed to it and the
/// key chain entries, in position order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Keys {
    /// The key envelopes whose recipient is the member.
    pub envelopes: Vec<LogEntry>,
    /// Every key chain entry of the space.
    pub chain: Vec<LogEntry>,
}

impl<S: RelayStore> Relay<S> {
    /// `keys`: the key envelopes addressed to `reader` and the key chain
    /// entries of `space`, every one the log holds, whatever their verdict:
    /// the member folds them itself.
    pub async fn keys(&self, reader: MemberId, space: SpaceId) -> Result<Keys, ErrorCode> {
        self.check_reader(reader, space).await?;
        let mut keys = Keys {
            envelopes: Vec::new(),
            chain: Vec::new(),
        };
        let mut after = 0;
        loop {
            let page = self.store().read(space, after, PAGE).await?;
            let Some(last) = page.last() else {
                return Ok(keys);
            };
            after = last.position;
            for entry in page.into_iter().filter(|e| e.kind == Kind::CONTROL) {
                // The relay appended only frames that decode.
                let change = Change::decode(&entry.frame).map_err(|_| ErrorCode::Unavailable)?;
                match ControlBody::decode(&change.header().payload).map(|body| body.entry) {
                    Ok(Entry::KeyEnvelope(envelope)) if envelope.recipient == reader => {
                        keys.envelopes.push(entry);
                    }
                    Ok(Entry::KeyChain(_)) => keys.chain.push(entry),
                    _ => {}
                }
            }
        }
    }

    /// `person-keys`: the person-key records of `space`.
    pub async fn person_keys(
        &self,
        reader: MemberId,
        space: SpaceId,
    ) -> Result<Vec<PersonKeyRecord>, ErrorCode> {
        self.check_reader(reader, space).await?;
        Ok(self.store().person_keys(space).await?)
    }

    /// `put-person-keys`: stores the records, each replacing the record of
    /// its person-key id, when `uploader` is a current member and every
    /// record is acceptable; otherwise `bad_record`, and nothing is stored.
    /// A record is acceptable when its writer's signature verifies, an
    /// accepted person-key entry of its id announces its generation and
    /// record hash, no accepted shred destroyed its id, and its generation
    /// is the generation of the record it replaces, or descends from it.
    /// The records of one request are checked in order, each against the
    /// one before it of the same id.
    pub async fn put_person_keys(
        &self,
        uploader: MemberId,
        space: SpaceId,
        records: Vec<PersonKeyRecord>,
    ) -> Result<(), ErrorCode> {
        self.check_reader(uploader, space).await?;
        let resolved = membership::resolve(space, &self.controls(space).await?);
        let mut current: BTreeMap<[u8; 16], u64> = self
            .store()
            .person_keys(space)
            .await?
            .into_iter()
            .map(|r| (r.id.0, r.generation))
            .collect();
        for record in &records {
            if !acceptable(&resolved, space, record, current.get(&record.id.0).copied()) {
                return Err(ErrorCode::BadRecord);
            }
            current.insert(record.id.0, record.generation);
        }
        Ok(self.store().put_person_keys(space, records).await?)
    }

    /// The space exists and `reader` is a current member of it.
    async fn check_reader(&self, reader: MemberId, space: SpaceId) -> Result<(), ErrorCode> {
        if self.store().head(space).await?.is_none() {
            return Err(ErrorCode::UnknownSpace);
        }
        self.check_uploader(reader, space).await
    }
}

/// RFC 0007's four conditions on a record, past the uploader's.
fn acceptable(
    resolved: &Resolved,
    space: SpaceId,
    record: &PersonKeyRecord,
    replaces: Option<u64>,
) -> bool {
    let id = record.id.0;
    let signed = resolved.member(record.writer).is_some_and(|writer| {
        membership::verify_record(
            &writer.key,
            space,
            id,
            record.generation,
            &record.wrapped,
            &record.signature,
        )
    });
    let hash = membership::record_hash(&record.wrapped);
    let announced = resolved
        .person_key(id)
        .is_some_and(|key| key.shred.is_none() && key.records.contains(&(record.generation, hash)));
    let descends = replaces.is_none_or(|stored| resolved.descends(record.generation, stored));
    signed && announced && descends
}
