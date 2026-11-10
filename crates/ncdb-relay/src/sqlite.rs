//! The SQLite backend of RFC 0007 ("The backend interface"), as its
//! backend table says for SQLite: a conditional append is one write
//! transaction on the head, a row in `spaces`; "one change id, one
//! position" is a unique index on (space, change id); frames are kept
//! inline.
//!
//! Several handles, also in several processes, may open one database: each
//! write transaction takes SQLite's write lock first (`BEGIN IMMEDIATE`) and
//! waits for it, so two appends never read the same head.
//!
//! **What this does not do.** SQLite blocks the thread that polls the
//! future. A server that runs these futures on an asynchronous runtime
//! moves them to a thread that may block.

use std::path::Path;
use std::sync::{Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

use ncdb_core::change::Kind;
use ncdb_core::ids::{ChangeId, MemberId, SpaceId};
use rusqlite::{
    Connection, ErrorCode, OptionalExtension, Transaction, TransactionBehavior, params,
};

use crate::store::{
    AppendOutcome, Effect, Entry, Epoch, MemberRecord, NewEntry, PersonKeyId, PersonKeyPut,
    PersonKeyRecord, Position, PutOutcome, RelayStore, StoreError,
};

/// How long a write waits for another handle's write lock.
const BUSY_TIMEOUT: Duration = Duration::from_secs(30);

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS relay (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    epoch BLOB NOT NULL
);
CREATE TABLE IF NOT EXISTS spaces (
    id BLOB PRIMARY KEY,
    head INTEGER NOT NULL
) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS log (
    space BLOB NOT NULL,
    position INTEGER NOT NULL,
    change BLOB NOT NULL,
    accepted_at INTEGER NOT NULL,
    author BLOB NOT NULL,
    kind INTEGER NOT NULL,
    frame BLOB NOT NULL,
    PRIMARY KEY (space, position)
) WITHOUT ROWID;
CREATE UNIQUE INDEX IF NOT EXISTS log_change ON log (space, change);
CREATE TABLE IF NOT EXISTS members (
    space BLOB NOT NULL,
    member BLOB NOT NULL,
    current INTEGER NOT NULL,
    state BLOB NOT NULL,
    PRIMARY KEY (space, member)
) WITHOUT ROWID;
CREATE INDEX IF NOT EXISTS members_current ON members (member, space) WHERE current = 1;
CREATE TABLE IF NOT EXISTS person_keys (
    space BLOB NOT NULL,
    id BLOB NOT NULL,
    generation INTEGER NOT NULL,
    writer BLOB NOT NULL,
    wrapped BLOB NOT NULL,
    signature BLOB NOT NULL,
    PRIMARY KEY (space, id)
) WITHOUT ROWID;
";

/// A handle on a relay's SQLite database. Its `Debug` output shows nothing
/// of the database.
pub struct SqliteStore {
    connection: Mutex<Connection>,
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
        let mut connection = Connection::open(path).map_err(failed("open"))?;
        connection
            .busy_timeout(BUSY_TIMEOUT)
            .map_err(failed("busy timeout"))?;
        wal(&connection)?;
        let tx = immediate(&mut connection)?;
        tx.execute_batch(SCHEMA).map_err(failed("schema"))?;
        tx.execute(
            "INSERT OR IGNORE INTO relay (id, epoch) VALUES (1, ?1)",
            [random_epoch()?.0],
        )
        .map_err(failed("first epoch"))?;
        tx.commit().map_err(failed("schema"))?;
        Ok(SqliteStore {
            connection: Mutex::new(connection),
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
        let connection = self.connection();
        let mut statement = connection
            .prepare(
                "SELECT position, change, accepted_at, author, kind, frame FROM log
                 WHERE space = ?1 AND position > ?2 ORDER BY position LIMIT ?3",
            )
            .map_err(failed("read"))?;
        let rows = statement
            .query_map(params![space.0, int(after), i64::from(limit)], |row| {
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
            let (position, change, accepted_at, author, kind, frame) =
                row.map_err(failed("read"))?;
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
            .query_map([space.0], |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                    row.get::<_, Vec<u8>>(3)?,
                    row.get::<_, Vec<u8>>(4)?,
                ))
            })
            .map_err(failed("person keys"))?;
        let mut records = Vec::new();
        for row in rows {
            let (id, generation, writer, wrapped, signature) =
                row.map_err(failed("person keys"))?;
            records.push(PersonKeyRecord {
                id: PersonKeyId(array(id)?),
                generation: unsigned(generation),
                writer: MemberId(array(writer)?),
                wrapped,
                signature,
            });
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
        for PersonKeyPut { record, .. } in puts {
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
}
