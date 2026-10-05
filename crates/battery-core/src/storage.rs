//! FULL-synchronous SQLite transactions are the write-ahead boundary for system changes.
use crate::{Activity, ActuatorKind, Error, Result, Session, Settings, Snapshot, Telemetry};
use chrono::{DateTime, Duration, Utc};
use rusqlite::{Connection, OptionalExtension, params};
use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub struct JournalEntry {
    pub id: i64,
    pub snapshot: Snapshot,
    pub desired: u32,
}

pub struct Storage {
    connection: Connection,
    _lock: File,
    pub directory: PathBuf,
}

impl Storage {
    pub fn open(directory: &Path, simulation: bool) -> Result<Self> {
        std::fs::create_dir_all(directory)?;
        let realm = if simulation {
            "simulation"
        } else {
            "battery-deadline"
        };
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(directory.join(format!("{realm}.lockfile")))?;
        fs2::FileExt::try_lock_exclusive(&lock).map_err(|_| Error::AlreadyRunning)?;
        let connection = Connection::open(directory.join(format!("{realm}.sqlite3")))?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        connection.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON;
            CREATE TABLE IF NOT EXISTS settings (id INTEGER PRIMARY KEY CHECK(id=1), value TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS sessions (id TEXT PRIMARY KEY, started_at TEXT NOT NULL, ended_at TEXT, value TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS telemetry_samples (id INTEGER PRIMARY KEY, session_id TEXT, timestamp TEXT NOT NULL, percentage REAL, power_w REAL, remaining_wh REAL);
            CREATE INDEX IF NOT EXISTS telemetry_time ON telemetry_samples(timestamp);
            CREATE TABLE IF NOT EXISTS actions (id INTEGER PRIMARY KEY, session_id TEXT, timestamp TEXT NOT NULL, value TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS actuator_effects (kind TEXT NOT NULL, target TEXT NOT NULL, average REAL NOT NULL, samples INTEGER NOT NULL, PRIMARY KEY(kind,target));
            CREATE TABLE IF NOT EXISTS recovery_journal (id INTEGER PRIMARY KEY, session_id TEXT, timestamp TEXT NOT NULL, snapshot TEXT NOT NULL, desired INTEGER NOT NULL, phase TEXT NOT NULL, outcome TEXT);
            PRAGMA user_version=1;")?;
        let storage = Self {
            connection,
            _lock: lock,
            directory: directory.into(),
        };
        storage.retention(Utc::now())?;
        Ok(storage)
    }
    pub fn settings(&self) -> Result<Settings> {
        let text: Option<String> = self
            .connection
            .query_row("SELECT value FROM settings WHERE id=1", [], |r| r.get(0))
            .optional()?;
        let settings = text
            .map(|s| serde_json::from_str::<Settings>(&s))
            .transpose()?
            .unwrap_or_default();
        settings.validate()?;
        Ok(settings)
    }
    pub fn save_settings(&self, settings: &Settings) -> Result<()> {
        settings.validate()?;
        self.connection.execute("INSERT INTO settings(id,value) VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET value=excluded.value",[serde_json::to_string(settings)?])?;
        Ok(())
    }
    pub fn save_session(&self, s: &Session) -> Result<()> {
        self.connection.execute("INSERT INTO sessions(id,started_at,ended_at,value) VALUES(?1,?2,?3,?4) ON CONFLICT(id) DO UPDATE SET ended_at=excluded.ended_at,value=excluded.value",
            params![s.id,s.started_at.to_rfc3339(),s.ended_at.map(|d|d.to_rfc3339()),serde_json::to_string(s)?])?;
        Ok(())
    }
    pub fn history(&self) -> Result<Vec<Session>> {
        let mut query = self
            .connection
            .prepare("SELECT value FROM sessions ORDER BY started_at DESC LIMIT 200")?;
        query
            .query_map([], |r| r.get::<_, String>(0))?
            .map(|r| Ok(serde_json::from_str(&r?)?))
            .collect()
    }
    pub fn close_interrupted_sessions(&self, now: DateTime<Utc>) -> Result<()> {
        for mut session in self.history()?.into_iter().filter(|s| s.ended_at.is_none()) {
            session.ended_at = Some(now);
            session.result = "interrupted".into();
            self.save_session(&session)?;
        }
        Ok(())
    }
    pub fn sample(&self, session: Option<&str>, t: &Telemetry, power: Option<f64>) -> Result<()> {
        self.connection.execute("INSERT INTO telemetry_samples(session_id,timestamp,percentage,power_w,remaining_wh) VALUES(?1,?2,?3,?4,?5)",
            params![session,t.timestamp.to_rfc3339(),t.percentage,power,t.remaining_wh])?;
        Ok(())
    }
    pub fn activity(&self, session: Option<&str>, mut a: Activity) -> Result<Activity> {
        self.connection.execute(
            "INSERT INTO actions(session_id,timestamp,value) VALUES(?1,?2,?3)",
            params![
                session,
                a.timestamp.to_rfc3339(),
                serde_json::to_string(&a)?
            ],
        )?;
        a.id = self.connection.last_insert_rowid();
        Ok(a)
    }
    pub fn recent_activity(&self, session: Option<&str>) -> Result<Vec<Activity>> {
        let mut query=self.connection.prepare("SELECT id,value FROM actions WHERE (?1 IS NULL OR session_id=?1) ORDER BY id DESC LIMIT 80")?;
        query
            .query_map([session], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?))
            })?
            .map(|r| {
                let (id, value) = r?;
                let mut a: Activity = serde_json::from_str(&value)?;
                a.id = id;
                Ok(a)
            })
            .collect()
    }
    /// Commit the original, planned target, and pending state before any hardware operation.
    pub fn prepare(
        &mut self,
        session: Option<&str>,
        snapshot: &Snapshot,
        desired: u32,
        now: DateTime<Utc>,
    ) -> Result<i64> {
        let transaction = self.connection.transaction()?;
        transaction.execute("INSERT INTO recovery_journal(session_id,timestamp,snapshot,desired,phase) VALUES(?1,?2,?3,?4,'pending')",
            params![session,now.to_rfc3339(),serde_json::to_string(snapshot)?,desired])?;
        let id = transaction.last_insert_rowid();
        transaction.commit()?;
        Ok(id)
    }
    pub fn mark_applied(&self, id: i64) -> Result<()> {
        self.connection.execute(
            "UPDATE recovery_journal SET phase='applied' WHERE id=?1",
            [id],
        )?;
        Ok(())
    }
    pub fn mark_restored(&self, id: i64, outcome: &str) -> Result<()> {
        self.connection.execute(
            "UPDATE recovery_journal SET phase='restored',outcome=?2 WHERE id=?1",
            params![id, outcome],
        )?;
        Ok(())
    }
    pub fn pending(&self) -> Result<Vec<JournalEntry>> {
        let mut query=self.connection.prepare("SELECT id,snapshot,desired FROM recovery_journal WHERE phase!='restored' ORDER BY id DESC")?;
        query
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, u32>(2)?,
                ))
            })?
            .map(|r| {
                let (id, snapshot, desired) = r?;
                Ok(JournalEntry {
                    id,
                    snapshot: serde_json::from_str(&snapshot)?,
                    desired,
                })
            })
            .collect()
    }
    /// A later manual value supersedes the entire chain for this target.
    pub fn supersede(&self, kind: ActuatorKind, target: &str) -> Result<()> {
        for e in self.pending()? {
            if e.snapshot.kind() == kind && e.snapshot.target() == target {
                self.mark_restored(e.id, "user_override")?;
            }
        }
        Ok(())
    }
    pub fn record_effect(&self, kind: ActuatorKind, target: &str, saving: f64) -> Result<()> {
        if !saving.is_finite() {
            return Ok(());
        }
        self.connection.execute("INSERT INTO actuator_effects(kind,target,average,samples) VALUES(?1,?2,?3,1) ON CONFLICT(kind,target) DO UPDATE SET average=(average*min(samples,9)+excluded.average)/(min(samples,9)+1),samples=samples+1",
            params![serde_json::to_string(&kind)?,target,saving])?;
        Ok(())
    }
    pub fn effects(&self, caps: &[crate::Capability]) -> Result<HashMap<ActuatorKind, f64>> {
        let mut result = HashMap::new();
        for cap in caps {
            let value: Option<f64> = self
                .connection
                .query_row(
                    "SELECT average FROM actuator_effects WHERE kind=?1 AND target=?2",
                    params![serde_json::to_string(&cap.kind)?, cap.target],
                    |r| r.get(0),
                )
                .optional()?;
            if let Some(value) = value {
                result.insert(cap.kind, value);
            }
        }
        Ok(result)
    }
    pub fn retention(&self, now: DateTime<Utc>) -> Result<()> {
        // Runtime stores one point / 10s, retained for seven days. No unbounded per-second archive.
        self.connection.execute(
            "DELETE FROM telemetry_samples WHERE timestamp<?1",
            [(now - Duration::days(7)).to_rfc3339()],
        )?;
        self.connection.execute(
            "DELETE FROM actions WHERE timestamp<?1",
            [(now - Duration::days(180)).to_rfc3339()],
        )?;
        self.connection.execute(
            "DELETE FROM recovery_journal WHERE phase='restored' AND timestamp<?1",
            [(now - Duration::days(30)).to_rfc3339()],
        )?;
        self.connection.execute(
            "DELETE FROM sessions WHERE ended_at IS NOT NULL AND started_at<?1",
            [(now - Duration::days(180)).to_rfc3339()],
        )?;
        self.connection
            .execute_batch("PRAGMA wal_checkpoint(PASSIVE);")?;
        Ok(())
    }
}
