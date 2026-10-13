//! The clip library: `library.db`, a SQLite file. The Electron build wrote the same file
//! through sql.js, so existing libraries open as they are.

use std::sync::{Mutex, MutexGuard, OnceLock};

use anyhow::{anyhow, Result};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use crate::paths;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CloudStatus {
    Local,
    Uploading,
    Live,
    Error,
    Overwritten,
}

impl CloudStatus {
    fn as_str(self) -> &'static str {
        match self {
            CloudStatus::Local => "local",
            CloudStatus::Uploading => "uploading",
            CloudStatus::Live => "live",
            CloudStatus::Error => "error",
            CloudStatus::Overwritten => "overwritten",
        }
    }

    fn parse(raw: Option<String>) -> Self {
        match raw.as_deref() {
            Some("uploading") => CloudStatus::Uploading,
            Some("live") => CloudStatus::Live,
            Some("error") => CloudStatus::Error,
            Some("overwritten") => CloudStatus::Overwritten,
            _ => CloudStatus::Local,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LocalClip {
    pub id: String,
    pub cloud_id: Option<String>,
    pub path: String,
    pub thumb_path: Option<String>,
    pub created_at: i64,
    pub duration_ms: i64,
    pub width: i64,
    pub height: i64,
    pub bytes: i64,
    pub title: Option<String>,
    pub game_title: Option<String>,
    pub cloud_status: CloudStatus,
    pub folder_id: Option<String>,
}

impl LocalClip {
    /// The cloud copy is up and its link works.
    pub fn is_live_cloud(&self) -> bool {
        self.cloud_id.is_some()
            && matches!(self.cloud_status, CloudStatus::Live | CloudStatus::Overwritten)
    }
}

/// A library-only grouping: filing a clip never moves its file on disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipFolder {
    pub id: String,
    pub name: String,
    pub created_at: i64,
}

static DB: OnceLock<Mutex<Connection>> = OnceLock::new();

pub fn open() -> Result<()> {
    if DB.get().is_some() {
        return Ok(());
    }
    std::fs::create_dir_all(paths::data_dir())?;
    let conn = Connection::open(paths::library_db())?;
    migrate(&conn)?;
    let _ = DB.set(Mutex::new(conn));
    Ok(())
}

fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS clips (
            id TEXT PRIMARY KEY,
            cloud_id TEXT,
            path TEXT NOT NULL,
            thumb_path TEXT,
            created_at INTEGER NOT NULL,
            duration_ms INTEGER NOT NULL,
            width INTEGER,
            height INTEGER,
            bytes INTEGER,
            title TEXT,
            game_title TEXT,
            cloud_status TEXT
        );
        CREATE UNIQUE INDEX IF NOT EXISTS clips_path ON clips(path);
        CREATE TABLE IF NOT EXISTS folders (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            created_at INTEGER NOT NULL
        );",
    )?;
    for column in ["title TEXT", "folder_id TEXT"] {
        // Fails when the column is already there.
        let _ = conn.execute(&format!("ALTER TABLE clips ADD COLUMN {column}"), []);
    }
    Ok(())
}

fn db() -> Result<MutexGuard<'static, Connection>> {
    open()?;
    DB.get()
        .ok_or_else(|| anyhow!("Library is not open"))?
        .lock()
        .map_err(|_| anyhow!("Library lock poisoned"))
}

const COLUMNS: &str = "id, cloud_id, path, thumb_path, created_at, duration_ms, width, height, \
                       bytes, title, game_title, cloud_status, folder_id";

fn from_row(row: &Row) -> rusqlite::Result<LocalClip> {
    // sql.js stored JS numbers, which may be REAL for timestamps with a fraction.
    let int = |index: usize| -> rusqlite::Result<i64> {
        let value: rusqlite::types::Value = row.get(index)?;
        Ok(match value {
            rusqlite::types::Value::Integer(n) => n,
            rusqlite::types::Value::Real(n) => n.round() as i64,
            _ => 0,
        })
    };
    Ok(LocalClip {
        id: row.get(0)?,
        cloud_id: row.get(1)?,
        path: row.get(2)?,
        thumb_path: row.get(3)?,
        created_at: int(4)?,
        duration_ms: int(5)?,
        width: int(6)?,
        height: int(7)?,
        bytes: int(8)?,
        title: row.get(9)?,
        game_title: row.get(10)?,
        cloud_status: CloudStatus::parse(row.get(11)?),
        folder_id: row.get(12)?,
    })
}

pub fn insert_clip(clip: &LocalClip) -> Result<()> {
    db()?.execute(
        &format!("INSERT INTO clips ({COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)"),
        params![
            clip.id,
            clip.cloud_id,
            clip.path,
            clip.thumb_path,
            clip.created_at,
            clip.duration_ms,
            clip.width,
            clip.height,
            clip.bytes,
            clip.title,
            clip.game_title,
            clip.cloud_status.as_str(),
            clip.folder_id
        ],
    )?;
    Ok(())
}

/// Read the clip, change it, write it back.
pub fn update_clip(id: &str, change: impl FnOnce(&mut LocalClip)) -> Result<LocalClip> {
    let mut clip = get_clip(id)?.ok_or_else(|| anyhow!("Clip not found"))?;
    change(&mut clip);
    clip.id = id.to_string();
    db()?.execute(
        "UPDATE clips SET
            cloud_id = ?1, path = ?2, thumb_path = ?3, created_at = ?4, duration_ms = ?5,
            width = ?6, height = ?7, bytes = ?8, title = ?9, game_title = ?10,
            cloud_status = ?11, folder_id = ?12
         WHERE id = ?13",
        params![
            clip.cloud_id,
            clip.path,
            clip.thumb_path,
            clip.created_at,
            clip.duration_ms,
            clip.width,
            clip.height,
            clip.bytes,
            clip.title,
            clip.game_title,
            clip.cloud_status.as_str(),
            clip.folder_id,
            id
        ],
    )?;
    Ok(clip)
}

pub fn set_cloud_status(id: &str, status: CloudStatus) -> Result<LocalClip> {
    update_clip(id, |clip| clip.cloud_status = status)
}

pub fn delete_clip(id: &str) -> Result<()> {
    db()?.execute("DELETE FROM clips WHERE id = ?1", [id])?;
    Ok(())
}

pub fn get_clip(id: &str) -> Result<Option<LocalClip>> {
    Ok(db()?
        .query_row(&format!("SELECT {COLUMNS} FROM clips WHERE id = ?1"), [id], from_row)
        .optional()?)
}

pub fn get_clip_by_path(path: &str) -> Result<Option<LocalClip>> {
    Ok(db()?
        .query_row(&format!("SELECT {COLUMNS} FROM clips WHERE path = ?1"), [path], from_row)
        .optional()?)
}

pub fn list_clips() -> Result<Vec<LocalClip>> {
    let db = db()?;
    let mut stmt = db.prepare(&format!("SELECT {COLUMNS} FROM clips ORDER BY created_at DESC"))?;
    let rows = stmt.query_map([], from_row)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn list_folders() -> Result<Vec<ClipFolder>> {
    let db = db()?;
    let mut stmt =
        db.prepare("SELECT id, name, created_at FROM folders ORDER BY name COLLATE NOCASE, created_at")?;
    let rows = stmt.query_map([], |row| {
        let created: rusqlite::types::Value = row.get(2)?;
        Ok(ClipFolder {
            id: row.get(0)?,
            name: row.get(1)?,
            created_at: match created {
                rusqlite::types::Value::Integer(n) => n,
                rusqlite::types::Value::Real(n) => n as i64,
                _ => 0,
            },
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn insert_folder(folder: &ClipFolder) -> Result<()> {
    db()?.execute(
        "INSERT INTO folders (id, name, created_at) VALUES (?1, ?2, ?3)",
        params![folder.id, folder.name, folder.created_at],
    )?;
    Ok(())
}

pub fn rename_folder(id: &str, name: &str) -> Result<()> {
    db()?.execute("UPDATE folders SET name = ?1 WHERE id = ?2", [name, id])?;
    Ok(())
}

/// Its clips go back to Unsorted; nothing is deleted.
pub fn delete_folder(id: &str) -> Result<()> {
    let mut db = db()?;
    let tx = db.transaction()?;
    tx.execute("UPDATE clips SET folder_id = NULL WHERE folder_id = ?1", [id])?;
    tx.execute("DELETE FROM folders WHERE id = ?1", [id])?;
    tx.commit()?;
    Ok(())
}

pub fn set_clips_folder(ids: &[String], folder_id: Option<&str>) -> Result<()> {
    let mut db = db()?;
    let tx = db.transaction()?;
    for id in ids {
        tx.execute("UPDATE clips SET folder_id = ?1 WHERE id = ?2", params![folder_id, id])?;
    }
    tx.commit()?;
    Ok(())
}
