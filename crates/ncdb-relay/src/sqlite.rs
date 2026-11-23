//! The SQLite backend of RFC 0007 ("The backend interface"), as its
//! backend table says for SQLite: a conditional append is one write
//! transaction on the head, a row in `spaces`; "one change id, one
//! position" is a unique index on (space, change id); the one space of an
//! account the relay does not list is a row of `guest_spaces`, keyed by
//! the account's identity (RFC 0015, "The account's identity"), in the
//! same transaction; an identity's admission is a row of `admissions`
//! (RFC 0015, "How the relay takes it"); frames are kept inline; a person-key put is one write transaction that reads each
//! stored generation before it writes. An upload's frame (RFC 0007, "Large
//! changes") waits in `uploads` until a push takes it, and is then kept
//! inline in the log like any other; a download token names a log row.
//!
//! Several handles, also in several processes, may open one database: each
//! write transaction takes SQLite's write lock first (`BEGIN IMMEDIATE`) and
//! waits for it, so two appends never read the same head. A clone of a
//! handle shares its connection, and with it the database in memory of
//! [`SqliteStore::in_memory`], for a relay in a test.
//!
//! **What this does not do.** SQLite blocks the thread that polls the
//! future. A server that runs these futures on an asynchronous runtime
//! moves them to a thread that may block.

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use ncdb_core::change::Kind;
use ncdb_core::ids::{ChangeId, MemberId, SpaceId};
use rusqlite::{
    Connection, ErrorCode, OptionalExtension, Transaction, TransactionBehavior, params,
};

use crate::store::{
    AdmissionRecord, Admitted, AppendOutcome, Effect, Entry, Epoch, MemberRecord, NewEntry,
    PersonKeyId, PersonKeyPut, PersonKeyRecord, Position, PutOutcome, RelayStore, StoreError,
    UPLOAD_KEPT_AFTER_EXPIRY, Upload, UploadId,
};

mod admission;
mod schema;
mod tokens;
use schema::SCHEMA;

/// How long a write waits for another handle's write lock.
const BUSY_TIMEOUT: Duration = Duration::from_secs(30);

/// A handle on a relay's SQLite database. A clone is another handle on the
/// same connection. Its `Debug` output shows nothing of the database.
#[derive(Clone)]
pub struct SqliteStore {
    connection: Arc<Mutex<Connection>>,
}

impl std::fmt::Debug for SqliteStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqliteStore").finish_non_exhaustive()
    }
}

impl SqliteStore {
    /// Opens the database at `path`, creating it with its tables and a
    /// first relay epoch if needed.
    pub fn open(path: &Path) -> Result<SqliteStore, StoreError> {
        let connection = Connection::open(path).map_err(failed("open"))?;
        connection
            .busy_timeout(BUSY_TIMEOUT)
            .map_err(failed("busy timeout"))?;
        wal(&connection)?;
        SqliteStore::with_schema(connection)
    }

    /// A new database in memory, with its tables and a first relay epoch,
    /// for a relay in a test. Only this handle and its clones reach it, and
    /// it is gone with the last of them.
    pub fn in_memory() -> Result<SqliteStore, StoreError> {
        SqliteStore::with_schema(Connection::open_in_memory().map_err(failed("open"))?)
    }

    /// A handle on `connection`, its tables and first epoch created if
    /// needed.
    fn with_schema(mut connection: Connection) -> Result<SqliteStore, StoreError> {
        let tx = immediate(&mut connection)?;
        tx.execute_batch(SCHEMA).map_err(failed("schema"))?;
        tx.execute(
            "INSERT OR IGNORE INTO relay (id, epoch) VALUES (1, ?1)",
            [random_epoch()?.0],
        )
        .map_err(failed("first epoch"))?;
        tx.commit().map_err(failed("schema"))?;
        Ok(SqliteStore {
            connection: Arc::new(Mutex::new(connection)),
        })
    }

    fn connection(&self) -> MutexGuard<'_, Connection> {
        // A panic while the lock was held leaves no transaction open: a
        // dropped transaction rolls back.
        self.connection
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// Switches the database to WAL. When several handles open a new database
/// at once, SQLite can answer SQLITE_BUSY here without calling the busy
/// handler (NOC-137), so this retries until `BUSY_TIMEOUT` itself.
fn wal(connection: &Connection) -> Result<(), StoreError> {
    let deadline = Instant::now() + BUSY_TIMEOUT;
    loop {
        match connection.pragma_update(None, "journal_mode", "WAL") {
            Err(rusqlite::Error::SqliteFailure(error, _))
                if error.code == ErrorCode::DatabaseBusy && Instant::now() < deadline =>
            {
                thread::sleep(Duration::from_millis(10));
            }
            result => return result.map_err(failed("journal mode")),
        }
    }
}

/// A write transaction that holds SQLite's write lock from its start.
fn immediate(connection: &mut Connection) -> Result<Transaction<'_>, StoreError> {
    connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(failed("begin"))
}

/// Maps a SQLite error to a store error that names the step. SQLite's
/// messages name tables and columns, never the values.
fn failed(step: &'static str) -> impl Fn(rusqlite::Error) -> StoreError {
    move |error| StoreError(format!("{step}: {error}"))
}

fn random_epoch() -> Result<Epoch, StoreError> {
    let mut epoch = [0; 16];
    getrandom::fill(&mut epoch).map_err(|_| StoreError("no secure generator".into()))?;
    Ok(Epoch(epoch))
}

/// SQLite's integers are signed 64-bit: positions, times, kinds and
/// generations are stored with the same 64 bits.
fn int(n: u64) -> i64 {
    n as i64
}

fn unsigned(n: i64) -> u64 {
    n as u64
}

fn array<const N: usize>(bytes: Vec<u8>) -> Result<[u8; N], StoreError> {
    bytes
        .try_into()
        .map_err(|_| StoreError("a stored id has the wrong length".into()))
}

/// The columns of a `person_keys` row: id, generation, writer, wrapped,
/// signature.
type RecordColumns = (Vec<u8>, i64, Vec<u8>, Vec<u8>, Vec<u8>);

fn record_columns(row: &rusqlite::Row<'_>) -> rusqlite::Result<RecordColumns> {
    Ok((
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
    ))
}

fn stored_record(
    (id, generation, writer, wrapped, signature): RecordColumns,
) -> Result<PersonKeyRecord, StoreError> {
    Ok(PersonKeyRecord {
        id: PersonKeyId(array(id)?),
        generation: unsigned(generation),
        writer: MemberId(array(writer)?),
        wrapped,
        signature,
    })
}

fn head_of(tx: &Connection, space: SpaceId) -> Result<Option<Position>, StoreError> {
    tx.query_row("SELECT head FROM spaces WHERE id = ?1", [space.0], |row| {
        row.get::<_, i64>(0)
    })
    .optional()
    .map(|head| head.map(unsigned))
    .map_err(failed("head"))
}

fn position_of(
    tx: &Connection,
    space: SpaceId,
    change: ChangeId,
) -> Result<Option<Position>, StoreError> {
    tx.query_row(
        "SELECT position FROM log WHERE space = ?1 AND change = ?2",
        params![space.0, change.0],
        |row| row.get::<_, i64>(0),
    )
    .optional()
    .map(|position| position.map(unsigned))
    .map_err(failed("find"))
}

/// Deletes the uploads kept long enough after their URL expired, and the
/// download tokens that expired, as of `now`.
fn sweep(tx: &Connection, now: u64) -> Result<(), StoreError> {
    tx.execute(
        "DELETE FROM uploads WHERE expires_at <= ?1",
        [int(now.saturating_sub(UPLOAD_KEPT_AFTER_EXPIRY))],
    )
    .map_err(failed("sweep uploads"))?;
    tx.execute("DELETE FROM downloads WHERE expires_at <= ?1", [int(now)])
        .map_err(failed("sweep downloads"))?;
    Ok(())
}

/// The log entries `sql` selects, with the columns position, change,
/// accepted at, author, kind and frame, in that order.
fn entries(
    connection: &Connection,
    sql: &str,
    params: impl rusqlite::Params,
) -> Result<Vec<Entry>, StoreError> {
    let mut statement = connection.prepare(sql).map_err(failed("read"))?;
    let rows = statement
        .query_map(params, |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Vec<u8>>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, Vec<u8>>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, Vec<u8>>(5)?,
            ))
        })
        .map_err(failed("read"))?;
    let mut entries = Vec::new();
    for row in rows {
        let (position, change, accepted_at, author, kind, frame) = row.map_err(failed("read"))?;
        entries.push(Entry {
            position: unsigned(position),
            change: ChangeId(array(change)?),
            accepted_at: unsigned(accepted_at),
            author: MemberId(array(author)?),
            kind: Kind(unsigned(kind)),
            frame,
        });
    }
    Ok(entries)
}

impl RelayStore for SqliteStore {
    async fn append(
        &self,
        space: SpaceId,
        expected_head: Position,
        entries: Vec<NewEntry>,
        effects: Vec<Effect>,
    ) -> Result<AppendOutcome, StoreError> {
        let mut connection = self.connection();
        let tx = immediate(&mut connection)?;
        let head = head_of(&tx, space)?.unwrap_or(0);
        if head != expected_head {
            return Ok(AppendOutcome::HeadMoved(head));
        }
        for entry in &entries {
            if let Some(position) = position_of(&tx, space, entry.change)? {
                let change = entry.change;
                return Ok(AppendOutcome::Duplicate { change, position });
            }
        }
        for effect in &effects {
            if let Effect::GuestSpace(account) = effect {
                let held: Option<Vec<u8>> = tx
                    .query_row(
                        "SELECT space FROM guest_spaces WHERE account = ?1",
                        [account],
                        |row| row.get(0),
                    )
                    .optional()
                    .map_err(failed("guest space"))?;
                if held.is_some() {
                    return Ok(AppendOutcome::GuestSpaceTaken);
                }
            }
            if let Effect::AdmittedSpace(identity) = effect
                && !admission::holds(&tx, *identity)?
            {
                return Ok(AppendOutcome::AdmissionSpent);
            }
        }
        let mut position = head;
        for entry in &entries {
            position += 1;
            tx.execute(
                "INSERT INTO log (space, position, change, accepted_at, author, kind, frame)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    space.0,
                    int(position),
                    entry.change.0,
                    int(entry.accepted_at),
                    entry.author.0,
                    int(entry.kind.0),
                    entry.frame,
                ],
            )
            .map_err(failed("append"))?;
        }
        tx.execute(
            "INSERT INTO spaces (id, head) VALUES (?1, ?2)
             ON CONFLICT (id) DO UPDATE SET head = excluded.head",
            params![space.0, int(position)],
        )
        .map_err(failed("head"))?;
        for effect in effects {
            match effect {
                Effect::Member(record) => {
                    tx.execute(
                        "INSERT INTO members (space, member, current, state)
                         VALUES (?1, ?2, ?3, ?4)
                         ON CONFLICT (space, member)
                         DO UPDATE SET current = excluded.current, state = excluded.state",
                        params![space.0, record.member.0, record.current, record.state],
                    )
                    .map_err(failed("member"))?;
                }
                Effect::GuestSpace(account) => {
                    tx.execute(
                        "INSERT INTO guest_spaces (account, space) VALUES (?1, ?2)",
                        params![account, space.0],
                    )
                    .map_err(failed("guest space"))?;
                }
                // MAX compares the signed integers SQLite stores, which
                // orders every number below 2^63; a statement's number is
                // a count or Unix seconds.
                Effect::Statement { identity, number } => {
                    tx.execute(
                        "INSERT INTO statements (identity, number) VALUES (?1, ?2)
                         ON CONFLICT (identity)
                         DO UPDATE SET number = MAX(number, excluded.number)",
                        params![identity, int(number)],
                    )
                    .map_err(failed("statement"))?;
                }
                Effect::AdmittedSpace(identity) => admission::count(&tx, identity)?,
            }
        }
        tx.commit().map_err(failed("commit"))?;
        Ok(AppendOutcome::Appended { head: position })
    }

    async fn read(
        &self,
        space: SpaceId,
        after: Position,
        limit: u32,
    ) -> Result<Vec<Entry>, StoreError> {
        entries(
            &self.connection(),
            "SELECT position, change, accepted_at, author, kind, frame FROM log
             WHERE space = ?1 AND position > ?2 ORDER BY position LIMIT ?3",
            params![space.0, int(after), i64::from(limit)],
        )
    }

    async fn read_controls(
        &self,
        space: SpaceId,
        after: Position,
        limit: u32,
    ) -> Result<Vec<Entry>, StoreError> {
        // The kind comes before the frame in a row, so SQLite skips a
        // sealed row without reading its frame's overflow pages.
        entries(
            &self.connection(),
            "SELECT position, change, accepted_at, author, kind, frame FROM log
             WHERE space = ?1 AND position > ?2 AND kind = ?3 ORDER BY position LIMIT ?4",
            params![space.0, int(after), int(Kind::CONTROL.0), i64::from(limit)],
        )
    }

    async fn read_bounded(
        &self,
        space: SpaceId,
        after: Position,
        limit: u32,
        max_bytes: u64,
    ) -> Result<Vec<Entry>, StoreError> {
        let connection = self.connection();
        // `length` of a blob reads the row's header, not the blob.
        let mut statement = connection
            .prepare(
                "SELECT position, length(frame) FROM log
                 WHERE space = ?1 AND position > ?2 ORDER BY position LIMIT ?3",
            )
            .map_err(failed("read"))?;
        let rows = statement
            .query_map(params![space.0, int(after), i64::from(limit)], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
            })
            .map_err(failed("read"))?;
        let (mut last, mut bytes) = (None, 0u64);
        for row in rows {
            let (position, length) = row.map_err(failed("read"))?;
            bytes = bytes.saturating_add(unsigned(length));
            if last.is_some() && bytes > max_bytes {
                break;
            }
            last = Some(position);
        }
        drop(statement);
        let Some(last) = last else {
            return Ok(Vec::new());
        };
        entries(
            &connection,
            "SELECT position, change, accepted_at, author, kind, frame FROM log
             WHERE space = ?1 AND position > ?2 AND position <= ?3 ORDER BY position",
            params![space.0, int(after), last],
        )
    }

    async fn head(&self, space: SpaceId) -> Result<Option<Position>, StoreError> {
        head_of(&self.connection(), space)
    }

    async fn find(&self, space: SpaceId, change: ChangeId) -> Result<Option<Position>, StoreError> {
        position_of(&self.connection(), space, change)
    }

    async fn member(
        &self,
        space: SpaceId,
        member: MemberId,
    ) -> Result<Option<MemberRecord>, StoreError> {
        self.connection()
            .query_row(
                "SELECT current, state FROM members WHERE space = ?1 AND member = ?2",
                params![space.0, member.0],
                |row| {
                    Ok(MemberRecord {
                        member,
                        current: row.get(0)?,
                        state: row.get(1)?,
                    })
                },
            )
            .optional()
            .map_err(failed("member"))
    }

    async fn statement(&self, identity: [u8; 32]) -> Result<Option<u64>, StoreError> {
        let number: Option<i64> = self
            .connection()
            .query_row(
                "SELECT number FROM statements WHERE identity = ?1",
                [identity],
                |row| row.get(0),
            )
            .optional()
            .map_err(failed("statement"))?;
        Ok(number.map(unsigned))
    }

    async fn admit(&self, identity: [u8; 32], admitted: Admitted) -> Result<(), StoreError> {
        admission::admit(&self.connection(), identity, admitted)
    }

    async fn admission(&self, identity: [u8; 32]) -> Result<Option<AdmissionRecord>, StoreError> {
        admission::admission(&self.connection(), identity)
    }

    async fn members_of(&self, member: MemberId) -> Result<Vec<SpaceId>, StoreError> {
        let connection = self.connection();
        let mut statement = connection
            .prepare("SELECT space FROM members WHERE member = ?1 AND current = 1 ORDER BY space")
            .map_err(failed("members of"))?;
        let rows = statement
            .query_map([member.0], |row| row.get::<_, Vec<u8>>(0))
            .map_err(failed("members of"))?;
        let mut spaces = Vec::new();
        for row in rows {
            spaces.push(SpaceId(array(row.map_err(failed("members of"))?)?));
        }
        Ok(spaces)
    }

    async fn person_keys(&self, space: SpaceId) -> Result<Vec<PersonKeyRecord>, StoreError> {
        let connection = self.connection();
        let mut statement = connection
            .prepare(
                "SELECT id, generation, writer, wrapped, signature FROM person_keys
                 WHERE space = ?1 ORDER BY id",
            )
            .map_err(failed("person keys"))?;
        let rows = statement
            .query_map([space.0], record_columns)
            .map_err(failed("person keys"))?;
        let mut records = Vec::new();
        for row in rows {
            records.push(stored_record(row.map_err(failed("person keys"))?)?);
        }
        Ok(records)
    }

    async fn person_keys_of(
        &self,
        space: SpaceId,
        ids: Vec<PersonKeyId>,
    ) -> Result<Vec<PersonKeyRecord>, StoreError> {
        let ids: BTreeSet<PersonKeyId> = ids.into_iter().collect();
        let connection = self.connection();
        let mut statement = connection
            .prepare(
                "SELECT id, generation, writer, wrapped, signature FROM person_keys
                 WHERE space = ?1 AND id = ?2",
            )
            .map_err(failed("person keys"))?;
        let mut records = Vec::new();
        for id in ids {
            let row = statement
                .query_row(params![space.0, id.0], record_columns)
                .optional()
                .map_err(failed("person keys"))?;
            if let Some(row) = row {
                records.push(stored_record(row)?);
            }
        }
        Ok(records)
    }

    async fn put_person_keys(
        &self,
        space: SpaceId,
        puts: Vec<PersonKeyPut>,
    ) -> Result<PutOutcome, StoreError> {
        let mut connection = self.connection();
        let tx = immediate(&mut connection)?;
        for PersonKeyPut { record, replaces } in puts {
            let stored = tx
                .query_row(
                    "SELECT generation FROM person_keys WHERE space = ?1 AND id = ?2",
                    params![space.0, record.id.0],
                    |row| row.get::<_, i64>(0),
                )
                .optional()
                .map_err(failed("put person keys"))?
                .map(unsigned);
            if stored != replaces {
                // Dropping the transaction rolls back the puts before it.
                return Ok(PutOutcome::Moved {
                    id: record.id,
                    stored,
                });
            }
            tx.execute(
                "INSERT INTO person_keys (space, id, generation, writer, wrapped, signature)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT (space, id) DO UPDATE SET generation = excluded.generation,
                    writer = excluded.writer, wrapped = excluded.wrapped,
                    signature = excluded.signature",
                params![
                    space.0,
                    record.id.0,
                    int(record.generation),
                    record.writer.0,
                    record.wrapped,
                    record.signature,
                ],
            )
            .map_err(failed("put person keys"))?;
        }
        tx.commit().map_err(failed("put person keys"))?;
        Ok(PutOutcome::Stored)
    }

    async fn delete_person_keys(
        &self,
        space: SpaceId,
        ids: Vec<PersonKeyId>,
    ) -> Result<(), StoreError> {
        let mut connection = self.connection();
        let tx = immediate(&mut connection)?;
        for id in ids {
            tx.execute(
                "DELETE FROM person_keys WHERE space = ?1 AND id = ?2",
                params![space.0, id.0],
            )
            .map_err(failed("delete person keys"))?;
        }
        tx.commit().map_err(failed("delete person keys"))
    }

    async fn epoch(&self) -> Result<Epoch, StoreError> {
        let epoch = self
            .connection()
            .query_row("SELECT epoch FROM relay WHERE id = 1", [], |row| {
                row.get::<_, Vec<u8>>(0)
            })
            .map_err(failed("epoch"))?;
        Ok(Epoch(array(epoch)?))
    }

    async fn new_epoch(&self) -> Result<Epoch, StoreError> {
        let epoch = random_epoch()?;
        self.connection()
            .execute("UPDATE relay SET epoch = ?1 WHERE id = 1", [epoch.0])
            .map_err(failed("new epoch"))?;
        Ok(epoch)
    }

    async fn begin_upload(&self, upload: Upload, now: u64) -> Result<(), StoreError> {
        let mut connection = self.connection();
        let tx = immediate(&mut connection)?;
        sweep(&tx, now)?;
        tx.execute(
            "INSERT INTO uploads (token, id, space, member, size, expires_at, frame)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)",
            params![
                upload.token.0,
                upload.id.0,
                upload.space.0,
                upload.member.0,
                int(upload.size),
                int(upload.expires_at)
            ],
        )
        .map_err(failed("begin upload"))?;
        tx.commit().map_err(failed("begin upload"))
    }

    async fn take_upload(
        &self,
        space: SpaceId,
        member: MemberId,
        id: UploadId,
    ) -> Result<Option<Vec<u8>>, StoreError> {
        self.connection()
            .query_row(
                "DELETE FROM uploads
                 WHERE id = ?1 AND space = ?2 AND member = ?3 AND frame IS NOT NULL
                 RETURNING frame",
                params![id.0, space.0, member.0],
                |row| row.get::<_, Vec<u8>>(0),
            )
            .optional()
            .map_err(failed("take upload"))
    }
}
