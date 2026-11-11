//! The relay core (RFC 0007, "What the relay checks"): every check a
//! received change goes through before a backend appends it.

use std::collections::BTreeSet;
use std::time::{SystemTime, UNIX_EPOCH};

use ncdb_core::change::{Change, Kind};
use ncdb_core::ids::{MemberId, SpaceId};
use ncdb_core::membership::{self, BodyError, ControlBody, Entry, Rejection, Role, Verdict};
use ncdb_core::seal;

use crate::error::ErrorCode;
use crate::large::PushFrame;
use crate::record;
use crate::store::{
    AppendOutcome, Effect, Entry as LogEntry, Epoch, NewEntry, Position, RelayStore, StoreError,
};

/// How often the core re-checks and retries an append, or a put of
/// person-key records, that another one overtook before it answers
/// `unavailable`.
pub(crate) const RETRIES: usize = 64;

/// How many entries the core reads at once when it collects a space's
/// control changes.
pub(crate) const PAGE: u32 = 1024;

/// The most frame bytes one read of the whole log loads, unless one frame
/// alone is larger: 4 MiB. A space of large changes costs the relay at most
/// this much memory per read, not a page of 16 MiB frames.
pub const PAGE_BYTES: u64 = 4 * 1024 * 1024;

/// The largest frame the relay accepts: 16 MiB (RFC 0007, "Large changes").
pub const MAX_FRAME: usize = 16 * 1024 * 1024;

/// What a relay deployment configures.
#[derive(Clone, Debug)]
pub struct Config {
    /// The account keys whose members may create spaces on this relay
    /// (RFC 0007, "What the relay checks", who may create a space).
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

/// The results of a push, and the members its appends made current.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Push {
    /// One result per frame, in the order of the frames.
    pub results: Vec<Pushed>,
    /// The members that an accepted control change made current in the
    /// space, each once, in the order they became current: the relay tells
    /// their open connections with `added` (RFC 0007, "Subscribe").
    pub added: Vec<MemberId>,
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

    /// The deployment's configuration.
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// The entries of `space` after the cursor `after`, at most `limit` and
    /// at most [`PAGE_BYTES`] of frames (at least one entry, when there is
    /// one), for `reader`.
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
        let entries = self
            .store
            .read_bounded(space, after, limit, PAGE_BYTES)
            .await?;
        Ok(Pulled { entries, head })
    }

    /// Checks each frame `uploader` pushes to `space`, one by one, and
    /// appends the accepted ones in order. One result per frame: a refused
    /// frame does not stop the frames after it.
    pub async fn push(&self, uploader: MemberId, space: SpaceId, frames: Vec<Vec<u8>>) -> Push {
        let frames = frames.into_iter().map(PushFrame::Bytes).collect();
        self.push_frames(uploader, space, frames).await
    }

    /// As [`Relay::push`], with frames that may name an upload (RFC 0007,
    /// "Large changes"). An upload is taken when its frame's turn comes,
    /// whatever its result: a push names an upload once.
    pub async fn push_frames(
        &self,
        uploader: MemberId,
        space: SpaceId,
        frames: Vec<PushFrame>,
    ) -> Push {
        let mut push = Push {
            results: Vec::with_capacity(frames.len()),
            added: Vec::new(),
        };
        for frame in frames {
            let frame = match frame {
                PushFrame::Bytes(bytes) => Ok(bytes),
                PushFrame::Upload(id) => self.take_upload(uploader, space, id).await,
            };
            let pushed = match frame {
                Ok(frame) => self.push_frame(uploader, space, frame).await,
                Err(code) => Err(code),
            };
            let result = match pushed {
                Ok((result, added)) => {
                    for member in added {
                        if !push.added.contains(&member) {
                            push.added.push(member);
                        }
                    }
                    result
                }
                Err(code) => Pushed::Rejected(code),
            };
            push.results.push(result);
        }
        push
    }

    /// One frame: the checks that need only the frame, then, against the
    /// space as it stands at its head, the checks of the uploader, the
    /// author and the membership rules, and an append conditional on that
    /// head. When another append overtook it, the space has changed, so it
    /// checks again. With the result come the members the append made
    /// current.
    async fn push_frame(
        &self,
        uploader: MemberId,
        space: SpaceId,
        frame: Vec<u8>,
    ) -> Result<(Pushed, Vec<MemberId>), ErrorCode> {
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
            let (expected_head, checked) = match self.store.head(space).await? {
                None => (0, self.check_genesis(uploader, &change)?),
                Some(head) => {
                    self.check_uploader(uploader, space).await?;
                    if let Some(position) = self.store.find(space, change.id()).await? {
                        return Ok((Pushed::Duplicate(position), Vec::new()));
                    }
                    (head, self.check_change(space, &change).await?)
                }
            };
            let Checked {
                effects,
                added,
                recheck,
            } = checked;
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
                AppendOutcome::Appended { head } => {
                    // Only an append that can change a shred's stability
                    // makes the relay check again. The check deletes
                    // nothing until NOC-224 is settled (NOC-225), and the
                    // append stands whatever the check does.
                    if recheck {
                        let _ = self.stable_shreds(space).await;
                    }
                    return Ok((Pushed::Accepted(head), added));
                }
                AppendOutcome::Duplicate { position, .. } => {
                    return Ok((Pushed::Duplicate(position), Vec::new()));
                }
                AppendOutcome::HeadMoved(_) => continue,
            }
        }
        Err(ErrorCode::Unavailable)
    }

    /// A change to a space the relay does not hold: only its genesis, pushed
    /// by its creator, a member of an account the relay allows (RFC 0007,
    /// "What the relay checks"), and valid by RFC 0008's rules for a genesis.
    fn check_genesis(&self, uploader: MemberId, change: &Change) -> Result<Checked, ErrorCode> {
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
        Ok(Checked::new(&resolved, &BTreeSet::new()))
    }

    /// The uploader or reader is a current member of the space.
    pub(crate) async fn check_uploader(
        &self,
        uploader: MemberId,
        space: SpaceId,
    ) -> Result<(), ErrorCode> {
        match self.store.member(space, uploader).await? {
            Some(row) if row.current => Ok(()),
            _ => Err(ErrorCode::NotMember),
        }
    }

    /// A change to a space the relay holds. A sealed change: its signature
    /// verifies with its author's key from the membership table, and its
    /// author may write it (RFC 0008, "Sealed changes"). A control change:
    /// `resolve` over the space's control changes and this one accepts it,
    /// which for a shred entry includes its second account's sign-off; the
    /// new membership table is the append's effect. The members current
    /// after it and not before were added by it.
    async fn check_change(&self, space: SpaceId, change: &Change) -> Result<Checked, ErrorCode> {
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
            return Ok(Checked {
                recheck: holds_or_held_admin(&row.state),
                ..Checked::default()
            });
        }
        if let Entry::Genesis(_) = control_body(change)?.entry {
            return Err(ErrorCode::SpaceExists);
        }
        let mut controls = self.controls(space).await?;
        let before = current(&membership::resolve(space, &controls));
        controls.push(change.clone());
        let resolved = membership::resolve(space, &controls);
        verdict(resolved.verdict(change.id()))?;
        Ok(Checked {
            recheck: true,
            ..Checked::new(&resolved, &before)
        })
    }

    /// The space's control changes, in position order. It loads no sealed
    /// change's frame.
    pub(crate) async fn controls(&self, space: SpaceId) -> Result<Vec<Change>, ErrorCode> {
        let mut controls = Vec::new();
        let mut after = 0;
        loop {
            let page = self.store.read_controls(space, after, PAGE).await?;
            let Some(last) = page.last() else {
                return Ok(controls);
            };
            after = last.position;
            for entry in &page {
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

/// What a change that passed its checks writes besides the log, the
/// members it makes current, and whether it can change a waiting shred's
/// stability.
#[derive(Default)]
struct Checked {
    effects: Vec<Effect>,
    added: Vec<MemberId>,
    recheck: bool,
}

impl Checked {
    /// The membership table of a resolved space, as the append's effects,
    /// and its current members that were not current `before`.
    fn new(resolved: &membership::Resolved, before: &BTreeSet<MemberId>) -> Checked {
        let records = record::records(resolved);
        let added = records
            .iter()
            .filter(|row| row.current && !before.contains(&row.member))
            .map(|row| row.member)
            .collect();
        let effects = records.into_iter().map(Effect::Member).collect();
        Checked {
            effects,
            added,
            recheck: false,
        }
    }
}

/// Whether a member's changes count for stability (RFC 0008, "When a change
/// is stable"): it holds the admin role, or an accepted grant ever gave it.
/// A sealed change of any other member cannot change a shred's stability:
/// only control changes and the runs of these members can.
fn holds_or_held_admin(state: &membership::MemberState) -> bool {
    state.role == Some(Role::Admin) || state.grants.iter().any(|grant| grant.role == Role::Admin)
}

/// The current members of a resolved space.
fn current(resolved: &membership::Resolved) -> BTreeSet<MemberId> {
    record::records(resolved)
        .into_iter()
        .filter(|row| row.current)
        .map(|row| row.member)
        .collect()
}
