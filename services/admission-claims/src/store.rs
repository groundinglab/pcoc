//! One SQLite file. The `name` table is created and left empty until stage 2.
//!
//! A claim takes `BEGIN IMMEDIATE` so two concurrent claims leave one badge row.
//! The process holds an exclusive non-blocking flock on the file for its life.

use crate::nostr::Event;
use nix::fcntl::{Flock, FlockArg};
use rusqlite::{params, Connection};
use std::fs::{self, File, OpenOptions};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS nonce (
    nonce TEXT PRIMARY KEY,
    pubkey TEXT NOT NULL,
    action TEXT NOT NULL,
    seen_unix_s INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS definition (
    d TEXT PRIMARY KEY,
    event_id TEXT NOT NULL UNIQUE,
    event_json TEXT NOT NULL,
    created_unix_s INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS badge (
    pubkey TEXT PRIMARY KEY,
    event_id TEXT NOT NULL UNIQUE,
    event_json TEXT NOT NULL,
    policy_id TEXT NOT NULL,
    log_day TEXT NOT NULL,
    log_seq INTEGER NOT NULL,
    admitted_unix_s INTEGER NOT NULL,
    object_sha256 TEXT NOT NULL,
    created_unix_s INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS names (
    name TEXT PRIMARY KEY,
    pubkey TEXT NOT NULL UNIQUE,
    event_id TEXT NOT NULL UNIQUE,
    event_json TEXT NOT NULL,
    created_unix_s INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS name (
    name TEXT PRIMARY KEY,
    pubkey TEXT NOT NULL UNIQUE,
    object_sha256 TEXT NOT NULL UNIQUE,
    event_id TEXT NOT NULL UNIQUE,
    event_json TEXT NOT NULL,
    policy_id TEXT NOT NULL,
    log_day TEXT NOT NULL,
    log_seq INTEGER NOT NULL,
    created_unix_s INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS outbox (
    event_id TEXT NOT NULL,
    target TEXT NOT NULL,
    body TEXT NOT NULL,
    attempts INTEGER NOT NULL,
    next_unix_s INTEGER NOT NULL,
    acked INTEGER NOT NULL,
    PRIMARY KEY (event_id, target)
);
";

#[derive(Clone, Debug)]
pub struct BadgeRow {
    pub pubkey: String,
    pub event_id: String,
    pub event_json: String,
    pub policy_id: String,
    pub log_day: String,
    pub log_seq: i64,
    pub admitted_unix_s: i64,
    pub object_sha256: String,
    pub created_unix_s: i64,
}

#[derive(Clone, Debug)]
pub struct DefinitionRow {
    pub d: String,
    pub event_id: String,
    pub event_json: String,
    pub created_unix_s: i64,
}

#[derive(Clone, Debug)]
pub struct OutboxRow {
    pub event_id: String,
    pub target: String,
    pub body: String,
    pub attempts: i64,
}

pub struct Store {
    conn: Mutex<Connection>,
    /// Held open so the flock stays with this process. `File` is not `Sync`.
    lock_file: Option<Mutex<Flock<File>>>,
    path: PathBuf,
}

impl Store {
    /// Create the file, take the flock, and create tables. A second writer
    /// gets an error instead of waiting.
    pub fn open_writer(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)
                    .map_err(|err| format!("could not create {}: {err}", parent.display()))?;
                let mut perms = fs::metadata(parent)
                    .map_err(|err| format!("could not stat {}: {err}", parent.display()))?
                    .permissions();
                perms.set_mode(0o700);
                fs::set_permissions(parent, perms)
                    .map_err(|err| format!("could not set directory mode: {err}"))?;
            }
        }
        let lock_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)
            .map_err(|err| format!("could not open {}: {err}", path.display()))?;
        let lock_file = Flock::lock(lock_file, FlockArg::LockExclusiveNonblock)
            .map_err(|_| "another admission-claims process holds the database lock".to_owned())?;
        let mut perms = lock_file
            .metadata()
            .map_err(|err| format!("could not stat database: {err}"))?
            .permissions();
        perms.set_mode(0o600);
        fs::set_permissions(path, perms)
            .map_err(|err| format!("could not set database mode: {err}"))?;
        let conn =
            Connection::open(path).map_err(|err| format!("could not open database: {err}"))?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")
            .map_err(|err| format!("could not set database pragmas: {err}"))?;
        conn.execute_batch(SCHEMA)
            .map_err(|err| format!("could not create tables: {err}"))?;
        Ok(Self {
            conn: Mutex::new(conn),
            lock_file: Some(Mutex::new(lock_file)),
            path: path.to_path_buf(),
        })
    }

    pub fn open_read(path: &Path) -> Result<Self, String> {
        let conn = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|err| format!("could not open database: {err}"))?;
        Ok(Self {
            conn: Mutex::new(conn),
            lock_file: None,
            path: path.to_path_buf(),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn db_ok(&self) -> bool {
        let conn = self.lock();
        conn.query_row("SELECT 1", [], |row| row.get::<_, i64>(0))
            .is_ok()
    }

    /// Drop nonces older than the signature skew, then insert. A duplicate
    /// inside the window is [`NonceInsert::Reused`].
    pub fn insert_nonce(
        &self,
        nonce: &str,
        pubkey: &str,
        action: &str,
        now: i64,
        skew: i64,
    ) -> Result<(), NonceInsert> {
        let conn = self.lock();
        let cutoff = now.saturating_sub(skew);
        conn.execute("DELETE FROM nonce WHERE seen_unix_s < ?1", params![cutoff])
            .map_err(|_| NonceInsert::Db)?;
        let changed = conn
            .execute(
                "INSERT INTO nonce (nonce, pubkey, action, seen_unix_s) VALUES (?1, ?2, ?3, ?4)",
                params![nonce, pubkey, action, now],
            )
            .map_err(|err| {
                if let rusqlite::Error::SqliteFailure(code, _) = &err {
                    if code.code == rusqlite::ErrorCode::ConstraintViolation {
                        return NonceInsert::Reused;
                    }
                }
                NonceInsert::Db
            })?;
        if changed != 1 {
            return Err(NonceInsert::Db);
        }
        Ok(())
    }

    pub fn definition(&self, d: &str) -> Result<Option<DefinitionRow>, String> {
        let conn = self.lock();
        let mut stmt = conn
            .prepare("SELECT d, event_id, event_json, created_unix_s FROM definition WHERE d = ?1")
            .map_err(|err| err.to_string())?;
        let mut rows = stmt.query(params![d]).map_err(|err| err.to_string())?;
        if let Some(row) = rows.next().map_err(|err| err.to_string())? {
            return Ok(Some(DefinitionRow {
                d: row.get(0).map_err(|err| err.to_string())?,
                event_id: row.get(1).map_err(|err| err.to_string())?,
                event_json: row.get(2).map_err(|err| err.to_string())?,
                created_unix_s: row.get(3).map_err(|err| err.to_string())?,
            }));
        }
        Ok(None)
    }

    pub fn insert_definition(&self, d: &str, event: &Event) -> Result<(), String> {
        let conn = self.lock();
        conn.execute(
            "INSERT INTO definition (d, event_id, event_json, created_unix_s) VALUES (?1, ?2, ?3, ?4)",
            params![d, event.id, event.to_json(), event.created_at],
        )
        .map_err(|err| format!("could not store definition: {err}"))?;
        Ok(())
    }

    /// Same `d`, new signed event (a replaceable 30009). Old outbox rows stay as sent.
    pub fn replace_definition(&self, d: &str, event: &Event) -> Result<(), String> {
        let conn = self.lock();
        conn.execute(
            "UPDATE definition SET event_id = ?2, event_json = ?3, created_unix_s = ?4 WHERE d = ?1",
            params![d, event.id, event.to_json(), event.created_at],
        )
        .map_err(|err| format!("could not replace definition: {err}"))?;
        Ok(())
    }

/// The stored name row for a name or for a pubkey.
    pub fn name_by(&self, column: NameKey, value: &str) -> Result<Option<NameRow>, String> {
        let conn = self.lock();
        lookup_name(&conn, column, value)
    }

    /// One name per npub, one npub per name. Same npub and name again returns the row.
    pub fn insert_name(&self, row: &NameRow) -> Result<NameInsert, String> {
        let conn = self.lock();
        conn.execute_batch("BEGIN IMMEDIATE")
            .map_err(|err| format!("could not begin: {err}"))?;
        let result = (|| {
            if let Some(existing) = lookup_name(&conn, NameKey::Pubkey, &row.pubkey)? {
                if existing.name == row.name {
                    return Ok(NameInsert::Existing(existing));
                }
                return Ok(NameInsert::HasName(existing));
            }
            if lookup_name(&conn, NameKey::Name, &row.name)?.is_some() {
                return Ok(NameInsert::Taken);
            }
            conn.execute(
                "INSERT INTO names (name, pubkey, event_id, event_json, created_unix_s) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![row.name, row.pubkey, row.event_id, row.event_json, row.created_unix_s],
            )
            .map_err(|err| format!("could not store name: {err}"))?;
            Ok(NameInsert::Fresh(row.clone()))
        })();
        match &result {
            Ok(_) => conn
                .execute_batch("COMMIT")
                .map_err(|err| format!("could not commit: {err}"))?,
            Err(_) => {
                let _ = conn.execute_batch("ROLLBACK");
            }
        }
        result
    }

    pub fn names(&self) -> Result<Vec<NameRow>, String> {
        let conn = self.lock();
        let mut stmt = conn
            .prepare("SELECT name, pubkey, event_id, event_json, created_unix_s FROM names ORDER BY created_unix_s, name")
            .map_err(|err| err.to_string())?;
        let rows = stmt
            .query_map([], name_row)
            .map_err(|err| err.to_string())?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|err| err.to_string())
    }

    /// Insert the badge or return the row already stored for this npub.
    /// `fresh` is true when this call inserted the row.
    pub fn insert_badge(&self, row: &BadgeRow) -> Result<(BadgeRow, bool), String> {
        let conn = self.lock();
        conn.execute_batch("BEGIN IMMEDIATE")
            .map_err(|err| format!("could not begin: {err}"))?;
        let existing = match lookup_badge(&conn, &row.pubkey) {
            Ok(found) => found,
            Err(err) => {
                let _ = conn.execute_batch("ROLLBACK");
                return Err(err);
            }
        };
        if let Some(existing) = existing {
            conn.execute_batch("COMMIT")
                .map_err(|err| format!("could not commit: {err}"))?;
            return Ok((existing, false));
        }
        let inserted = conn.execute(
            "INSERT INTO badge (pubkey, event_id, event_json, policy_id, log_day, log_seq, admitted_unix_s, object_sha256, created_unix_s)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                row.pubkey,
                row.event_id,
                row.event_json,
                row.policy_id,
                row.log_day,
                row.log_seq,
                row.admitted_unix_s,
                row.object_sha256,
                row.created_unix_s
            ],
        );
        if let Err(err) = inserted {
            let _ = conn.execute_batch("ROLLBACK");
            if let rusqlite::Error::SqliteFailure(code, _) = &err {
                if code.code == rusqlite::ErrorCode::ConstraintViolation {
                    // The other transaction committed between our read and our insert.
                    let again = lookup_badge(&conn, &row.pubkey)?;
                    if let Some(existing) = again {
                        return Ok((existing, false));
                    }
                }
            }
            return Err(format!("could not store badge: {err}"));
        }
        conn.execute_batch("COMMIT")
            .map_err(|err| format!("could not commit: {err}"))?;
        Ok((row.clone(), true))
    }

    pub fn badge_count(&self) -> Result<i64, String> {
        let conn = self.lock();
        conn.query_row("SELECT COUNT(*) FROM badge", [], |row| row.get(0))
            .map_err(|err| err.to_string())
    }

    pub fn name_count(&self) -> Result<i64, String> {
        let conn = self.lock();
        conn.query_row("SELECT COUNT(*) FROM names", [], |row| row.get(0))
            .map_err(|err| err.to_string())
    }

    pub fn definitions(&self) -> Result<Vec<DefinitionRow>, String> {
        let conn = self.lock();
        let mut stmt = conn
            .prepare("SELECT d, event_id, event_json, created_unix_s FROM definition")
            .map_err(|err| err.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok(DefinitionRow {
                    d: row.get(0)?,
                    event_id: row.get(1)?,
                    event_json: row.get(2)?,
                    created_unix_s: row.get(3)?,
                })
            })
            .map_err(|err| err.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| err.to_string())
    }

    pub fn badges(&self) -> Result<Vec<BadgeRow>, String> {
        let conn = self.lock();
        let mut stmt = conn
            .prepare(
                "SELECT pubkey, event_id, event_json, policy_id, log_day, log_seq, admitted_unix_s, object_sha256, created_unix_s
                 FROM badge ORDER BY created_unix_s, event_id",
            )
            .map_err(|err| err.to_string())?;
        let rows = stmt
            .query_map([], |row| {
                Ok(BadgeRow {
                    pubkey: row.get(0)?,
                    event_id: row.get(1)?,
                    event_json: row.get(2)?,
                    policy_id: row.get(3)?,
                    log_day: row.get(4)?,
                    log_seq: row.get(5)?,
                    admitted_unix_s: row.get(6)?,
                    object_sha256: row.get(7)?,
                    created_unix_s: row.get(8)?,
                })
            })
            .map_err(|err| err.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| err.to_string())
    }

    /// Keep the first body for an event target. Index and nostr.json targets
    /// replace the body so a later claim does not publish a stale index.
    pub fn enqueue(
        &self,
        event_id: &str,
        target: &str,
        body: &str,
        now: i64,
    ) -> Result<(), String> {
        let conn = self.lock();
        let replace =
            target.ends_with("claims/index.json") || target.ends_with(".well-known/nostr.json");
        if replace {
            conn.execute(
                "INSERT INTO outbox (event_id, target, body, attempts, next_unix_s, acked)
                 VALUES (?1, ?2, ?3, 0, ?4, 0)
                 ON CONFLICT(event_id, target) DO UPDATE SET body = excluded.body, acked = 0, next_unix_s = excluded.next_unix_s",
                params![event_id, target, body, now],
            )
            .map_err(|err| err.to_string())?;
        } else {
            conn.execute(
                "INSERT INTO outbox (event_id, target, body, attempts, next_unix_s, acked)
                 VALUES (?1, ?2, ?3, 0, ?4, 0)
                 ON CONFLICT(event_id, target) DO NOTHING",
                params![event_id, target, body, now],
            )
            .map_err(|err| err.to_string())?;
        }
        Ok(())
    }

    pub fn due(&self, now: i64) -> Result<Vec<OutboxRow>, String> {
        let conn = self.lock();
        let mut stmt = conn
            .prepare(
                "SELECT event_id, target, body, attempts FROM outbox
                 WHERE acked = 0 AND next_unix_s <= ?1 ORDER BY next_unix_s, event_id, target",
            )
            .map_err(|err| err.to_string())?;
        let rows = stmt
            .query_map(params![now], |row| {
                Ok(OutboxRow {
                    event_id: row.get(0)?,
                    target: row.get(1)?,
                    body: row.get(2)?,
                    attempts: row.get(3)?,
                })
            })
            .map_err(|err| err.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| err.to_string())
    }

    pub fn ack(&self, event_id: &str, target: &str) -> Result<(), String> {
        let conn = self.lock();
        conn.execute(
            "UPDATE outbox SET acked = 1 WHERE event_id = ?1 AND target = ?2",
            params![event_id, target],
        )
        .map_err(|err| err.to_string())?;
        Ok(())
    }

    pub fn fail(
        &self,
        event_id: &str,
        target: &str,
        attempts: i64,
        next_unix_s: i64,
    ) -> Result<(), String> {
        let conn = self.lock();
        conn.execute(
            "UPDATE outbox SET attempts = ?3, next_unix_s = ?4 WHERE event_id = ?1 AND target = ?2",
            params![event_id, target, attempts, next_unix_s],
        )
        .map_err(|err| err.to_string())?;
        Ok(())
    }

    pub fn unacked(&self) -> Result<i64, String> {
        let conn = self.lock();
        conn.query_row("SELECT COUNT(*) FROM outbox WHERE acked = 0", [], |row| {
            row.get(0)
        })
        .map_err(|err| err.to_string())
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }
}

fn lookup_badge(conn: &Connection, pubkey: &str) -> Result<Option<BadgeRow>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT pubkey, event_id, event_json, policy_id, log_day, log_seq, admitted_unix_s, object_sha256, created_unix_s
             FROM badge WHERE pubkey = ?1",
        )
        .map_err(|err| err.to_string())?;
    let mut rows = stmt.query(params![pubkey]).map_err(|err| err.to_string())?;
    if let Some(row) = rows.next().map_err(|err| err.to_string())? {
        return Ok(Some(BadgeRow {
            pubkey: row.get(0).map_err(|err| err.to_string())?,
            event_id: row.get(1).map_err(|err| err.to_string())?,
            event_json: row.get(2).map_err(|err| err.to_string())?,
            policy_id: row.get(3).map_err(|err| err.to_string())?,
            log_day: row.get(4).map_err(|err| err.to_string())?,
            log_seq: row.get(5).map_err(|err| err.to_string())?,
            admitted_unix_s: row.get(6).map_err(|err| err.to_string())?,
            object_sha256: row.get(7).map_err(|err| err.to_string())?,
            created_unix_s: row.get(8).map_err(|err| err.to_string())?,
        }));
    }
    Ok(None)
}

#[derive(Debug, PartialEq, Eq)]
pub enum NonceInsert {
    Reused,
    Db,
}

// The flock fd stays open for the life of the store.
impl Drop for Store {
    fn drop(&mut self) {
        let _ = self.lock_file.take();
    }
}

#[derive(Clone, Debug)]
pub struct NameRow {
    pub name: String,
    pub pubkey: String,
    pub event_id: String,
    pub event_json: String,
    pub created_unix_s: i64,
}

#[derive(Clone, Copy, Debug)]
pub enum NameKey {
    Name,
    Pubkey,
}

#[derive(Debug)]
pub enum NameInsert {
    Fresh(NameRow),
    Existing(NameRow),
    HasName(NameRow),
    Taken,
}

fn name_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<NameRow> {
    Ok(NameRow {
        name: row.get(0)?,
        pubkey: row.get(1)?,
        event_id: row.get(2)?,
        event_json: row.get(3)?,
        created_unix_s: row.get(4)?,
    })
}

fn lookup_name(
    conn: &rusqlite::Connection,
    column: NameKey,
    value: &str,
) -> Result<Option<NameRow>, String> {
    let sql = match column {
        NameKey::Name => "SELECT name, pubkey, event_id, event_json, created_unix_s FROM names WHERE name = ?1",
        NameKey::Pubkey => "SELECT name, pubkey, event_id, event_json, created_unix_s FROM names WHERE pubkey = ?1",
    };
    let mut stmt = conn.prepare(sql).map_err(|err| err.to_string())?;
    let mut rows = stmt.query(params![value]).map_err(|err| err.to_string())?;
    match rows.next().map_err(|err| err.to_string())? {
        Some(row) => name_row(row).map(Some).map_err(|err| err.to_string()),
        None => Ok(None),
    }
}
