//! The relay's AWS backend (RFC 0007, "The AWS backend"): DynamoDB and S3
//! behind the relay core's backend interface, [`RelayStore`]. It passes the
//! conformance suite against the AWS Emulator (`tests/conformance.rs`). It
//! does not implement [`TokenUrls`](ncdb_relay::store::TokenUrls): behind
//! API Gateway the URLs of large changes are presigned S3 URLs, so the
//! backend keeps no tokens (NOC-295, NOC-303).
//!
//! The tables and keys are RFC 0007's ("Tables and keys"), each a table of
//! its own whose name [`Names`] gives:
//!
//! - `log`: PK `space`, SK `position`; `change`, `accepted_at`, `author`,
//!   `kind`, `size`, and the frame inline as `frame` or in S3 under
//!   `frame_key`.
//! - `changes`: PK `space`, SK `change`; `position`.
//! - `spaces`: PK `space`; `head`. The item with key [`EPOCH_ITEM`] holds
//!   the relay epoch.
//! - `members`: PK `space`, SK `member`; `current`, `state`, and a global
//!   secondary index `by_member` on `member`, sorted by `space`.
//! - `person_keys`: PK `space`, SK `id`; `generation`, `writer`, `wrapped`,
//!   `signature`.
//! - `uploads`: PK `id`; `space`, `member`, `size`, `expires_at`. Its
//!   frame goes straight into S3 under [`upload_key`].
//!
//! The `connections` table belongs
//! to delivery, not to the backend interface, and comes with it.
//!
//! An append is one `TransactWriteItems`; a frame above [`INLINE_LIMIT`]
//! goes to S3 first, under [`frame_key`], and [`AwsStore::sweep_orphans`]
//! deletes what a failed append left there.
//!
//! The SDK runs on Tokio. Each call of the store runs on the runtime the
//! store was given, so its futures run on any executor, also one without a
//! runtime, as the conformance suite's.
//!
//! **What this does not do.** [`RelayStore::members_of`] reads a global
//! secondary index, which DynamoDB keeps eventually consistent: a membership
//! an append just wrote may be missing from it for a moment.

use std::future::Future;
use std::sync::Arc;

use ncdb_core::ids::{ChangeId, MemberId, SpaceId};
use ncdb_relay::store::{
    AppendOutcome, Effect, Entry, Epoch, MemberRecord, NewEntry, PersonKeyId, PersonKeyPut,
    PersonKeyRecord, Position, PutOutcome, RelayStore, StoreError, Upload, UploadId,
};
use tokio::runtime::Handle;

pub use sweep::{PASS_EVERY, SweepBudget, SweepOutcome, SweepRun};

mod append;
mod item;
mod keys;
mod read;
pub mod schema;
mod sweep;
mod uploads;

/// The largest frame kept inline in its `log` item, in bytes: 8 KiB (RFC
/// 0007, "How large changes go to S3"). A larger frame goes to S3.
pub const INLINE_LIMIT: usize = 8 * 1024;

/// The key of the reserved item in `spaces` that holds the relay epoch. A
/// space id has 16 bytes, so no space has this key.
pub const EPOCH_ITEM: &[u8] = b"epoch";

/// The key of the reserved item in `spaces` that holds where the orphan
/// sweep stands (NOC-293; its place is the owner's NOC-306): the 5 bytes
/// `sweep`, which no 16-byte space id, no 32-byte account key and not
/// [`EPOCH_ITEM`] can be.
pub const SWEEP_ITEM: &[u8] = b"sweep";

/// The S3 key of a frame above [`INLINE_LIMIT`]:
/// `frames/<space id>/<change id>`, each id in lowercase hex.
pub fn frame_key(space: SpaceId, change: ChangeId) -> String {
    format!("frames/{}/{}", item::hex(&space.0), item::hex(&change.0))
}

/// The S3 key of an upload's frame: `uploads/<upload id>`, in lowercase hex.
pub fn upload_key(id: UploadId) -> String {
    format!("uploads/{}", item::hex(&id.0))
}

/// The names of the backend's tables and bucket.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Names {
    /// The log: one item per change.
    pub log: String,
    /// One item per change id, for "one change id, one position".
    pub changes: String,
    /// One item per space, with its head, and the epoch item.
    pub spaces: String,
    /// The membership table.
    pub members: String,
    /// The person-key store.
    pub person_keys: String,
    /// Uploads that wait to be pushed.
    pub uploads: String,
    /// The bucket of frames above 8 KiB and of uploads.
    pub bucket: String,
}

impl Names {
    /// The tables `<prefix>log`, `<prefix>changes`, `<prefix>spaces`,
    /// `<prefix>members`, `<prefix>person-keys` and `<prefix>uploads`, and
    /// the bucket `<prefix>frames`. There is no tokens table: the backend
    /// serves no URLs of its own (NOC-295, NOC-303).
    pub fn with_prefix(prefix: &str) -> Names {
        let name = |table: &str| format!("{prefix}{table}");
        Names {
            log: name("log"),
            changes: name("changes"),
            spaces: name("spaces"),
            members: name("members"),
            person_keys: name("person-keys"),
            uploads: name("uploads"),
            bucket: name("frames"),
        }
    }
}

/// What every call of a handle shares: its clients and the names.
pub(crate) struct Aws {
    pub db: aws_sdk_dynamodb::Client,
    pub s3: aws_sdk_s3::Client,
    pub names: Names,
}

/// A handle on the AWS backend. Its `Debug` output shows only the names.
#[derive(Clone)]
pub struct AwsStore {
    aws: Arc<Aws>,
    runtime: Handle,
}

impl std::fmt::Debug for AwsStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AwsStore")
            .field("names", &self.aws.names)
            .finish_non_exhaustive()
    }
}

impl AwsStore {
    /// A handle on the tables and bucket `names` gives, through the clients
    /// given, whose calls run on `runtime`.
    pub fn new(
        db: aws_sdk_dynamodb::Client,
        s3: aws_sdk_s3::Client,
        names: Names,
        runtime: Handle,
    ) -> AwsStore {
        AwsStore {
            aws: Arc::new(Aws { db, s3, names }),
            runtime,
        }
    }

    /// Runs `call` on the store's runtime once the returned future is
    /// polled, on whatever executor polls it.
    fn run<T, F, C>(
        &self,
        call: C,
    ) -> impl Future<Output = Result<T, StoreError>> + Send + use<T, F, C>
    where
        T: Send + 'static,
        F: Future<Output = Result<T, StoreError>> + Send + 'static,
        C: FnOnce(Arc<Aws>) -> F + Send + 'static,
    {
        let (aws, runtime) = (self.aws.clone(), self.runtime.clone());
        async move {
            match runtime.spawn(call(aws)).await {
                Ok(result) => result,
                Err(_) => Err(StoreError("a call of the AWS backend stopped".into())),
            }
        }
    }

    /// Deletes every object under `frames/` that no change holds and that
    /// was last modified before `cutoff`, in milliseconds since the Unix
    /// epoch: the orphans of appends whose transaction failed after their
    /// frame was written (RFC 0007, "How large changes go to S3"). A daily
    /// sweep passes a cutoff a day ago, so it never deletes the frame of an
    /// append still in flight. Returns how many objects it deleted.
    pub fn sweep_orphans(
        &self,
        cutoff: u64,
    ) -> impl Future<Output = Result<u32, StoreError>> + Send + use<> {
        self.run(move |aws| async move { append::sweep_orphans(&aws, cutoff).await })
    }

    /// One bounded run of the sweep at `now`, in milliseconds since the Unix
    /// epoch: it continues where the last run stopped, deletes the orphans
    /// last modified before `cutoff` until `budget` is spent, and stores
    /// where it got to (NOC-293). A pass that completed less than
    /// [`PASS_EVERY`] ago is not started again: the run is
    /// [`SweepOutcome::Skipped`].
    pub fn sweep_orphans_run(
        &self,
        cutoff: u64,
        now: u64,
        budget: SweepBudget,
    ) -> impl Future<Output = Result<SweepRun, StoreError>> + Send + use<> {
        self.run(move |aws| async move { sweep::run(&aws, cutoff, now, budget).await })
    }
}

impl RelayStore for AwsStore {
    fn append(
        &self,
        space: SpaceId,
        expected_head: Position,
        entries: Vec<NewEntry>,
        effects: Vec<Effect>,
    ) -> impl Future<Output = Result<AppendOutcome, StoreError>> + Send {
        self.run(move |aws| async move {
            append::append(&aws, space, expected_head, entries, effects).await
        })
    }

    fn read(
        &self,
        space: SpaceId,
        after: Position,
        limit: u32,
    ) -> impl Future<Output = Result<Vec<Entry>, StoreError>> + Send {
        self.run(move |aws| async move {
            read::read(&aws, space, after, limit, read::Filter::All).await
        })
    }

    fn read_controls(
        &self,
        space: SpaceId,
        after: Position,
        limit: u32,
    ) -> impl Future<Output = Result<Vec<Entry>, StoreError>> + Send {
        self.run(move |aws| async move {
            read::read(&aws, space, after, limit, read::Filter::Controls).await
        })
    }

    fn read_bounded(
        &self,
        space: SpaceId,
        after: Position,
        limit: u32,
        max_bytes: u64,
    ) -> impl Future<Output = Result<Vec<Entry>, StoreError>> + Send {
        self.run(move |aws| async move {
            read::read(&aws, space, after, limit, read::Filter::Bytes(max_bytes)).await
        })
    }

    fn head(
        &self,
        space: SpaceId,
    ) -> impl Future<Output = Result<Option<Position>, StoreError>> + Send {
        self.run(move |aws| async move { append::head(&aws, space).await })
    }

    fn find(
        &self,
        space: SpaceId,
        change: ChangeId,
    ) -> impl Future<Output = Result<Option<Position>, StoreError>> + Send {
        self.run(move |aws| async move { append::find(&aws, space, change).await })
    }

    fn member(
        &self,
        space: SpaceId,
        member: MemberId,
    ) -> impl Future<Output = Result<Option<MemberRecord>, StoreError>> + Send {
        self.run(move |aws| async move { keys::member(&aws, space, member).await })
    }

    fn members_of(
        &self,
        member: MemberId,
    ) -> impl Future<Output = Result<Vec<SpaceId>, StoreError>> + Send {
        self.run(move |aws| async move { keys::members_of(&aws, member).await })
    }

    fn person_keys(
        &self,
        space: SpaceId,
    ) -> impl Future<Output = Result<Vec<PersonKeyRecord>, StoreError>> + Send {
        self.run(move |aws| async move { keys::person_keys(&aws, space).await })
    }

    fn put_person_keys(
        &self,
        space: SpaceId,
        puts: Vec<PersonKeyPut>,
    ) -> impl Future<Output = Result<PutOutcome, StoreError>> + Send {
        self.run(move |aws| async move { keys::put_person_keys(&aws, space, puts).await })
    }

    fn delete_person_keys(
        &self,
        space: SpaceId,
        ids: Vec<PersonKeyId>,
    ) -> impl Future<Output = Result<(), StoreError>> + Send {
        self.run(move |aws| async move { keys::delete_person_keys(&aws, space, ids).await })
    }

    fn epoch(&self) -> impl Future<Output = Result<Epoch, StoreError>> + Send {
        self.run(move |aws| async move { keys::epoch(&aws).await })
    }

    fn new_epoch(&self) -> impl Future<Output = Result<Epoch, StoreError>> + Send {
        self.run(move |aws| async move { keys::new_epoch(&aws).await })
    }

    fn begin_upload(
        &self,
        upload: Upload,
        now: u64,
    ) -> impl Future<Output = Result<(), StoreError>> + Send {
        self.run(move |aws| async move { uploads::begin_upload(&aws, upload, now).await })
    }

    fn take_upload(
        &self,
        space: SpaceId,
        member: MemberId,
        id: UploadId,
    ) -> impl Future<Output = Result<Option<Vec<u8>>, StoreError>> + Send {
        self.run(move |aws| async move { uploads::take_upload(&aws, space, member, id).await })
    }
}
