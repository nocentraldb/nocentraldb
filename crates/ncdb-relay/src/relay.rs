//! The relay core (RFC 0007, "What the relay checks"): every check a
//! received change goes through before a backend appends it.

use std::collections::BTreeSet;
use std::time::{SystemTime, UNIX_EPOCH};

use ncdb_core::change::{Change, Kind};
use ncdb_core::ids::{MemberId, SpaceId};
use ncdb_core::membership::{self, BodyError, ControlBody, Entry, Rejection, Verdict};
use ncdb_core::seal;

use crate::error::ErrorCode;
use crate::record;
use crate::store::{
    AppendOutcome, Effect, Entry as LogEntry, Epoch, NewEntry, Position, RelayStore, StoreError,
};

/// How often the core re-checks and retries an append that another append
/// overtook before it answers `unavailable`.
const RETRIES: usize = 64;

/// How many entries the core reads at once when it collects a space's
/// control changes.
const PAGE: u32 = 1024;

/// The largest frame the relay accepts: 16 MiB (RFC 0007, decision R5).
pub const MAX_FRAME: usize = 16 * 1024 * 1024;

/// What a relay deployment configures.
#[derive(Clone, Debug)]
pub struct Config {
    /// The account keys whose members may create spaces on this relay
    /// (RFC 0007, decision R6).
    pub allowed_accounts: BTreeSet<[u8; 32]>,
    /// The largest frame the relay accepts.
    pub max_frame: usize,
    /// The relay's clock, in milliseconds since the Unix epoch.
    pub clock: fn() -> u64,
}

impl Config {
    /// A configuration that lets members of these accounts create spaces.
    pub fn new(allowed_accounts: BTreeSet<[u8; 32]>) -> Config {
        Config {
            allowed_accounts,
            max_frame: MAX_FRAME,
            clock: now,
        }
    }
}

/// The system clock in milliseconds since the Unix epoch.
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

impl From<StoreError> for ErrorCode {
    /// A backend that failed appended nothing.
    fn from(_: StoreError) -> ErrorCode {
        ErrorCode::Unavailable
    }
}

/// The result of one frame of a push.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pushed {
    /// Appended at this position.
    Accepted(Position),
    /// The relay already holds this change id, at this position.
    Duplicate(Position),
    /// Refused, with this code.
    Rejected(ErrorCode),
}

/// A page of a space's log.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pulled {
    /// The entries after the cursor, in position order.
    pub entries: Vec<LogEntry>,
    /// The space's head.
    pub head: Position,
}

/// The relay core over a backend.
#[derive(Debug)]
pub struct Relay<S> {
    store: S,
    config: Config,
}

impl<S: RelayStore> Relay<S> {
    /// The relay core over `store`.
    pub fn new(store: S, config: Config) -> Relay<S> {
        Relay { store, config }
    }

    /// The backend.
    pub fn store(&self) -> &S {
        &self.store
    }

    /// The entries of `space` after the cursor `after`, at most `limit`,
    /// for `reader`.
    pub async fn pull(
        &self,
        reader: MemberId,
        space: SpaceId,
        epoch: Epoch,
        after: Position,
        limit: u32,
    ) -> Result<Pulled, ErrorCode> {
        if self.store.epoch().await? != epoch {
            return Err(ErrorCode::EpochChanged);
        }
        let Some(head) = self.store.head(space).await? else {
            return Err(ErrorCode::UnknownSpace);
        };
        self.check_uploader(reader, space).await?;
        if after > head {
            return Err(ErrorCode::BadCursor);
        }
        let entries = self.store.read(space, after, limit).await?;
        Ok(Pulled { entries, head })
    }

    /// Checks each frame `uploader` pushes to `space`, one by one, and
    /// appends the accepted ones in order. One result per frame: a refused
    /// frame does not stop the frames after it.
    pub async fn push(
        &self,
        uploader: MemberId,
        space: SpaceId,
        frames: Vec<Vec<u8>>,
    ) -> Vec<Pushed> {
        let mut results = Vec::with_capacity(frames.len());
        for frame in frames {
            let result = self.push_frame(uploader, space, frame).await;
            results.push(result.unwrap_or_else(Pushed::Rejected));
        }
        results
    }

    /// One frame: the checks that need only the frame, then, against the
    /// space as it stands at its head, the checks of the uploader, the
    /// author and the membership rules, and an append conditional on that
    /// head. When another append overtook it, the space has changed, so it
    /// checks again.
    async fn push_frame(
        &self,
        uploader: MemberId,
        space: SpaceId,
        frame: Vec<u8>,
    ) -> Result<Pushed, ErrorCode> {
        if frame.len() > self.config.max_frame {
            return Err(ErrorCode::TooLarge);
        }
        let change = Change::decode(&frame).map_err(|_| ErrorCode::Malformed)?;
        let header = change.header();
        if header.space != space {
            return Err(ErrorCode::Malformed);
        }
        // A suite, kind, header format or "must understand" key the relay
        // does not know: it cannot check the change.
        if header.suite != seal::SUITE || !header.foldable() {
            return Err(ErrorCode::UnsupportedSuite);
        }
        for _ in 0..RETRIES {
            let (expected_head, effects) = match self.store.head(space).await? {
                None => (0, self.check_genesis(uploader, &change)?),
                Some(head) => {
                    self.check_uploader(uploader, space).await?;
                    if let Some(position) = self.store.find(space, change.id()).await? {
                        return Ok(Pushed::Duplicate(position));
                    }
                    (head, self.check_change(space, &change).await?)
                }
            };
            let entry = NewEntry {
                change: change.id(),
                accepted_at: (self.config.clock)(),
                author: header.author,
                kind: header.kind,
                frame: frame.clone(),
            };
            match self
                .store
                .append(space, expected_head, vec![entry], effects)
                .await?
            {
                AppendOutcome::Appended { head } => return Ok(Pushed::Accepted(head)),
                AppendOutcome::Duplicate { position, .. } => {
                    return Ok(Pushed::Duplicate(position));
                }
                AppendOutcome::HeadMoved(_) => continue,
            }
        }
        Err(ErrorCode::Unavailable)
    }

    /// A change to a space the relay does not hold: only its genesis, pushed
    /// by its creator, a member of an account the relay allows (decision
    /// R6), and valid by RFC 0008's rules for a genesis.
    fn check_genesis(&self, uploader: MemberId, change: &Change) -> Result<Vec<Effect>, ErrorCode> {
        let genesis = match control_body(change) {
            Ok(ControlBody {
                entry: Entry::Genesis(genesis),
                ..
            }) => genesis,
            _ => return Err(ErrorCode::UnknownSpace),
        };
        if genesis.creator.id() != uploader {
            return Err(ErrorCode::NotMember);
        }
        if !self
            .config
            .allowed_accounts
            .contains(&genesis.certificate.account_key)
        {
            return Err(ErrorCode::NotAllowed);
        }
        let resolved = membership::resolve(change.header().space, [change]);
        verdict(resolved.verdict(change.id()))?;
        Ok(effects(&resolved))
    }

    /// The uploader or reader is a current member of the space.
    async fn check_uploader(&self, uploader: MemberId, space: SpaceId) -> Result<(), ErrorCode> {
        match self.store.member(space, uploader).await? {
            Some(row) if row.current => Ok(()),
            _ => Err(ErrorCode::NotMember),
        }
    }

    /// A change to a space the relay holds. A sealed change: its signature
    /// verifies with its author's key from the membership table, and its
    /// author may write it (RFC 0008, "Sealed changes"). A control change:
    /// `resolve` over the space's control changes and this one accepts it;
    /// the new membership table is the append's effect.
    async fn check_change(
        &self,
        space: SpaceId,
        change: &Change,
    ) -> Result<Vec<Effect>, ErrorCode> {
        let header = change.header();
        if header.kind == Kind::SEALED {
            let Some(row) = self.store.member(space, header.author).await? else {
                return Err(ErrorCode::AuthorNotMember);
            };
            let row = record::decode(&row).map_err(|_| ErrorCode::Unavailable)?;
            seal::verify(change, &row.state.key).map_err(|_| ErrorCode::BadSignature)?;
            if !row.admits(header) {
                return Err(ErrorCode::AuthorNotMember);
            }
            return Ok(Vec::new());
        }
        let body = control_body(change)?;
        match body.entry {
            Entry::Genesis(_) => return Err(ErrorCode::SpaceExists),
            // The relay cannot yet check a shred entry's sign-off by a
            // second account, which RFC 0007 requires: it keeps the entry
            // out until it can.
            Entry::Shred(_) => {
                return Err(ErrorCode::UnsupportedSuite);
            }
            _ => {}
        }
        let mut controls = self.controls(space).await?;
        controls.push(change.clone());
        let resolved = membership::resolve(space, &controls);
        verdict(resolved.verdict(change.id()))?;
        Ok(effects(&resolved))
    }

    /// The space's control changes, in position order.
    async fn controls(&self, space: SpaceId) -> Result<Vec<Change>, ErrorCode> {
        let mut controls = Vec::new();
        let mut after = 0;
        loop {
            let page = self.store.read(space, after, PAGE).await?;
            let Some(last) = page.last() else {
                return Ok(controls);
            };
            after = last.position;
            for entry in page.iter().filter(|e| e.kind == Kind::CONTROL) {
                // The relay appended only frames that decode.
                let change = Change::decode(&entry.frame).map_err(|_| ErrorCode::Unavailable)?;
                controls.push(change);
            }
        }
    }
}

/// The control body of a change: malformed when it is not one, not
/// understood (an entry type or key this relay does not know) when it
/// cannot check it.
fn control_body(change: &Change) -> Result<ControlBody, ErrorCode> {
    if change.header().kind != Kind::CONTROL {
        return Err(ErrorCode::UnknownSpace);
    }
    ControlBody::decode(&change.header().payload).map_err(|error| match error {
        BodyError::Malformed(_) => ErrorCode::Malformed,
        BodyError::NotUnderstood => ErrorCode::UnsupportedSuite,
    })
}

/// The relay's answer to `resolve`'s verdict on a control change.
fn verdict(verdict: Option<&Verdict>) -> Result<(), ErrorCode> {
    match verdict {
        Some(Verdict::Accepted | Verdict::Lost) => Ok(()),
        Some(Verdict::Parked) => Err(ErrorCode::UnsupportedSuite),
        Some(Verdict::Rejected(Rejection::Malformed)) => Err(ErrorCode::Malformed),
        Some(Verdict::Rejected(Rejection::BadSignature)) => Err(ErrorCode::BadSignature),
        Some(Verdict::Rejected(Rejection::UnknownAuthor)) => Err(ErrorCode::AuthorNotMember),
        Some(Verdict::Rejected(_)) | None => Err(ErrorCode::MembershipRule),
    }
}

/// The membership table of a resolved space, as the append's effects.
fn effects(resolved: &membership::Resolved) -> Vec<Effect> {
    record::records(resolved)
        .into_iter()
        .map(Effect::Member)
        .collect()
}
