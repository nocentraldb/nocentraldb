//! The tables of the SQLite backend: the log and the spaces' heads, the
//! membership table, the person-key store, the one space of each account
//! the relay does not list, uploads and download tokens, and the epoch.

pub(super) const SCHEMA: &str = "
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
CREATE TABLE IF NOT EXISTS uploads (
    token BLOB PRIMARY KEY,
    id BLOB NOT NULL UNIQUE,
    space BLOB NOT NULL,
    member BLOB NOT NULL,
    size INTEGER NOT NULL,
    expires_at INTEGER NOT NULL,
    frame BLOB
);
CREATE INDEX IF NOT EXISTS uploads_expiry ON uploads (expires_at);
CREATE TABLE IF NOT EXISTS guest_spaces (
    account BLOB PRIMARY KEY,
    space BLOB NOT NULL
) WITHOUT ROWID;
CREATE TABLE IF NOT EXISTS downloads (
    token BLOB PRIMARY KEY,
    space BLOB NOT NULL,
    position INTEGER NOT NULL,
    expires_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS downloads_expiry ON downloads (expires_at);
";
