use rusqlite::{params, Connection, OptionalExtension};
use std::{
    path::{Path, PathBuf},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEntry {
    pub url: String,
    pub title: String,
    pub visited_at: i64,
    pub visit_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookmarkEntry {
    pub url: String,
    pub title: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncryptedVaultRow {
    pub id: String,
    pub origin: String,
    pub title: String,
    pub username_enc: String,
    pub password_enc: String,
    pub notes_enc: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadEntry {
    pub id: i64,
    pub url: String,
    pub file_path: String,
    pub file_name: String,
    pub status: String,
    pub received_bytes: u64,
    pub total_bytes: Option<u64>,
    pub created_at: i64,
    pub partial_path: Option<String>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

pub struct BrowserStorage {
    connection: Mutex<Connection>,
}

impl BrowserStorage {
    pub fn open(path: &Path) -> rusqlite::Result<Self> {
        if let Some(parent) = path.parent().filter(|parent| !parent.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)
                .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
        }
        let connection = Connection::open(path)?;
        connection.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             PRAGMA foreign_keys=ON;
             PRAGMA busy_timeout=5000;
             CREATE TABLE IF NOT EXISTS history (
                 id INTEGER PRIMARY KEY,
                 url TEXT NOT NULL UNIQUE,
                 title TEXT NOT NULL DEFAULT '',
                 visited_at INTEGER NOT NULL,
                 visit_count INTEGER NOT NULL DEFAULT 1
             );
             CREATE INDEX IF NOT EXISTS idx_history_visited ON history(visited_at DESC);
             CREATE TABLE IF NOT EXISTS bookmarks (
                 id INTEGER PRIMARY KEY,
                 url TEXT NOT NULL UNIQUE,
                 title TEXT NOT NULL DEFAULT '',
                 created_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS settings (
                 key TEXT PRIMARY KEY,
                 value TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS downloads (
                 id INTEGER PRIMARY KEY,
                 url TEXT NOT NULL,
                 file_path TEXT NOT NULL,
                 file_name TEXT NOT NULL,
                 status TEXT NOT NULL DEFAULT 'queued',
                 received_bytes INTEGER NOT NULL DEFAULT 0,
                 total_bytes INTEGER,
                 partial_path TEXT,
                 etag TEXT,
                 last_modified TEXT,
                 created_at INTEGER NOT NULL,
                 updated_at INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_downloads_created ON downloads(created_at DESC);
             CREATE TABLE IF NOT EXISTS vault_meta (
                 id INTEGER PRIMARY KEY CHECK (id = 1),
                 salt_b64 TEXT NOT NULL,
                 verifier_enc TEXT NOT NULL,
                 kdf_params TEXT NOT NULL,
                 created_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS password_entries (
                 id TEXT PRIMARY KEY,
                 origin TEXT NOT NULL,
                 title TEXT NOT NULL,
                 username_enc TEXT NOT NULL,
                 password_enc TEXT NOT NULL,
                 notes_enc TEXT,
                 created_at INTEGER NOT NULL,
                 updated_at INTEGER NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_password_entries_origin ON password_entries(origin);",
        )?;
        // Additive migration for profiles created before resumable downloads.
        for (column, declaration) in [
            ("partial_path", "TEXT"),
            ("etag", "TEXT"),
            ("last_modified", "TEXT"),
        ] {
            let exists: bool = {
                let mut statement = connection.prepare("PRAGMA table_info(downloads)")?;
                let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
                columns.filter_map(Result::ok).any(|name| name == column)
            };
            if !exists {
                connection.execute_batch(&format!("ALTER TABLE downloads ADD COLUMN {column} {declaration}"))?;
            }
        }
        Ok(Self { connection: Mutex::new(connection) })
    }

    pub fn record_visit(&self, url: &str, title: &str) -> rusqlite::Result<()> {
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Ok(());
        }
        let now = unix_time_ms();
        self.connection.lock().expect("database mutex poisoned").execute(
            "INSERT INTO history(url,title,visited_at,visit_count) VALUES(?1,?2,?3,1)
             ON CONFLICT(url) DO UPDATE SET title=excluded.title,
             visited_at=excluded.visited_at, visit_count=history.visit_count+1",
            params![url, title, now],
        )?;
        Ok(())
    }

    pub fn update_page_title(&self, url: &str, title: &str) -> rusqlite::Result<()> {
        self.connection.lock().expect("database mutex poisoned").execute(
            "UPDATE history SET title=?1 WHERE url=?2",
            params![title, url],
        )?;
        Ok(())
    }

    pub fn add_bookmark(&self, url: &str, title: &str) -> rusqlite::Result<()> {
        self.connection.lock().expect("database mutex poisoned").execute(
            "INSERT INTO bookmarks(url,title,created_at) VALUES(?1,?2,?3)
             ON CONFLICT(url) DO UPDATE SET title=excluded.title",
            params![url, title, unix_time_ms()],
        )?;
        Ok(())
    }

    pub fn remove_bookmark(&self, url: &str) -> rusqlite::Result<()> {
        self.connection.lock().expect("database mutex poisoned").execute(
            "DELETE FROM bookmarks WHERE url=?1",
            params![url],
        )?;
        Ok(())
    }

    pub fn is_bookmarked(&self, url: &str) -> rusqlite::Result<bool> {
        let connection = self.connection.lock().expect("database mutex poisoned");
        let found: Option<i64> = connection.query_row(
            "SELECT id FROM bookmarks WHERE url=?1",
            params![url],
            |row| row.get(0),
        ).optional()?;
        Ok(found.is_some())
    }

    pub fn set_setting(&self, key: &str, value: &str) -> rusqlite::Result<()> {
        self.connection.lock().expect("database mutex poisoned").execute(
            "INSERT INTO settings(key,value) VALUES(?1,?2)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn get_setting(&self, key: &str) -> rusqlite::Result<Option<String>> {
        self.connection.lock().expect("database mutex poisoned").query_row(
            "SELECT value FROM settings WHERE key=?1",
            params![key],
            |row| row.get(0),
        ).optional()
    }

    pub fn site_permissions(&self) -> rusqlite::Result<Vec<(String, String)>> {
        let connection = self.connection.lock().expect("database mutex poisoned");
        let mut statement = connection.prepare(
            "SELECT key, value FROM settings WHERE key LIKE 'permission:%' ORDER BY key"
        )?;
        let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.collect()
    }

    pub fn delete_setting(&self, key: &str) -> rusqlite::Result<()> {
        self.connection.lock().expect("database mutex poisoned").execute(
            "DELETE FROM settings WHERE key=?1",
            params![key],
        )?;
        Ok(())
    }

    /// Persist the restorable HTTP(S) tab URLs and the selected tab index.
    pub fn save_session(&self, urls: &[String], active_index: usize) -> rusqlite::Result<()> {
        let session = serde_json::json!({
            "version": 1,
            "urls": urls,
            "active_index": active_index,
        });
        self.set_setting("browser_session_v1", &session.to_string())
    }

    /// Load a previously saved session. Invalid or obsolete data is ignored safely.
    pub fn load_session(&self) -> rusqlite::Result<Option<(Vec<String>, usize)>> {
        let Some(raw) = self.get_setting("browser_session_v1")? else {
            return Ok(None);
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
            return Ok(None);
        };
        if value.get("version").and_then(serde_json::Value::as_u64) != Some(1) {
            return Ok(None);
        }
        let Some(items) = value.get("urls").and_then(serde_json::Value::as_array) else {
            return Ok(None);
        };
        let urls: Vec<String> = items.iter()
            .filter_map(serde_json::Value::as_str)
            .filter_map(|raw_url| url::Url::parse(raw_url).ok())
            .filter(|url| matches!(url.scheme(), "http" | "https"))
            .map(|url| url.to_string())
            .take(50)
            .collect();
        if urls.is_empty() {
            return Ok(None);
        }
        let requested_index = value.get("active_index")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0) as usize;
        let active_index = requested_index.min(urls.len() - 1);
        Ok(Some((urls, active_index)))
    }

    /// Persist the recently closed HTTP(S) tabs, newest first.
    pub fn save_closed_tabs(&self, urls: &[String]) -> rusqlite::Result<()> {
        let safe_urls: Vec<String> = urls.iter()
            .filter_map(|raw_url| url::Url::parse(raw_url).ok())
            .filter(|url| matches!(url.scheme(), "http" | "https"))
            .map(|url| url.to_string())
            .rev()
            .take(10)
            .collect();
        self.set_setting("closed_tabs_v1", &serde_json::to_string(&safe_urls)
            .expect("serializing a list of URLs cannot fail"))
    }

    /// Load recently closed HTTP(S) tabs in reopening order (most recent first).
    pub fn load_closed_tabs(&self) -> rusqlite::Result<Vec<String>> {
        let Some(raw) = self.get_setting("closed_tabs_v1")? else {
            return Ok(Vec::new());
        };
        let Ok(items) = serde_json::from_str::<Vec<String>>(&raw) else {
            return Ok(Vec::new());
        };
        Ok(items.into_iter()
            .filter_map(|raw_url| url::Url::parse(&raw_url).ok())
            .filter(|url| matches!(url.scheme(), "http" | "https"))
            .map(|url| url.to_string())
            .take(10)
            .collect())
    }

    pub fn get_vault_meta(&self) -> rusqlite::Result<Option<(String, String, String)>> {
        self.connection.lock().expect("database mutex poisoned").query_row(
            "SELECT salt_b64,verifier_enc,kdf_params FROM vault_meta WHERE id=1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        ).optional()
    }

    pub fn save_vault_meta(&self, salt_b64: &str, verifier_enc: &str, kdf_params: &str) -> rusqlite::Result<()> {
        self.connection.lock().expect("database mutex poisoned").execute(
            "INSERT OR REPLACE INTO vault_meta(id,salt_b64,verifier_enc,kdf_params,created_at)
             VALUES(1,?1,?2,?3,?4)",
            params![salt_b64, verifier_enc, kdf_params, unix_time_ms()],
        )?;
        Ok(())
    }

    /// Atomically replace encrypted credentials and verifier metadata.
    pub fn replace_vault_data(&self, rows: &[EncryptedVaultRow], salt_b64: &str, verifier_enc: &str, kdf_params: &str) -> rusqlite::Result<()> {
        let mut connection = self.connection.lock().expect("database mutex poisoned");
        let transaction = connection.transaction()?;
        transaction.execute("DELETE FROM password_entries", [])?;
        {
            let mut statement = transaction.prepare(
                "INSERT INTO password_entries(id,origin,title,username_enc,password_enc,notes_enc,created_at,updated_at)
                 VALUES(?1,?2,?3,?4,?5,?6,?7,?8)"
            )?;
            for row in rows {
                statement.execute(params![row.id,row.origin,row.title,row.username_enc,row.password_enc,row.notes_enc,row.created_at,row.updated_at])?;
            }
        }
        transaction.execute(
            "INSERT OR REPLACE INTO vault_meta(id,salt_b64,verifier_enc,kdf_params,created_at) VALUES(1,?1,?2,?3,?4)",
            params![salt_b64,verifier_enc,kdf_params,unix_time_ms()],
        )?;
        transaction.commit()
    }

    pub fn upsert_password_row(&self, row: &EncryptedVaultRow) -> rusqlite::Result<()> {
        self.connection.lock().expect("database mutex poisoned").execute(
            "INSERT INTO password_entries(id,origin,title,username_enc,password_enc,notes_enc,created_at,updated_at)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8)
             ON CONFLICT(id) DO UPDATE SET origin=excluded.origin,title=excluded.title,
             username_enc=excluded.username_enc,password_enc=excluded.password_enc,
             notes_enc=excluded.notes_enc,updated_at=excluded.updated_at",
            params![row.id,row.origin,row.title,row.username_enc,row.password_enc,row.notes_enc,row.created_at,row.updated_at],
        )?;
        Ok(())
    }

    pub fn list_password_rows(&self) -> rusqlite::Result<Vec<EncryptedVaultRow>> {
        let connection = self.connection.lock().expect("database mutex poisoned");
        let mut statement = connection.prepare(
            "SELECT id,origin,title,username_enc,password_enc,notes_enc,created_at,updated_at
             FROM password_entries ORDER BY updated_at DESC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(EncryptedVaultRow {
                id: row.get(0)?,
                origin: row.get(1)?,
                title: row.get(2)?,
                username_enc: row.get(3)?,
                password_enc: row.get(4)?,
                notes_enc: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            })
        })?;
        rows.collect()
    }

    pub fn delete_password_row(&self, id: &str) -> rusqlite::Result<()> {
        self.connection.lock().expect("database mutex poisoned").execute(
            "DELETE FROM password_entries WHERE id=?1", params![id],
        )?;
        Ok(())
    }

    /// Create a persistent download record before starting the transfer.
    pub fn create_download(&self, url: &str, file_path: &str, file_name: &str) -> rusqlite::Result<i64> {
        let now = unix_time_ms();
        let connection = self.connection.lock().expect("database mutex poisoned");
        connection.execute(
            "INSERT INTO downloads(url,file_path,file_name,status,created_at,updated_at)
             VALUES(?1,?2,?3,'queued',?4,?4)",
            params![url, file_path, file_name, now],
        )?;
        Ok(connection.last_insert_rowid())
    }

    /// Update transfer progress. A missing total means the server did not provide a content length.
    pub fn update_download_progress(
        &self, id: i64, received: u64, total: Option<u64>,
        partial_path: &Path, etag: Option<&str>, last_modified: Option<&str>,
    ) -> rusqlite::Result<()> {
        self.connection.lock().expect("database mutex poisoned").execute(
            "UPDATE downloads SET status='downloading',received_bytes=?1,total_bytes=?2,
             partial_path=?3,etag=?4,last_modified=?5,updated_at=?6 WHERE id=?7",
            params![received as i64, total.map(|value| value as i64), partial_path.display().to_string(),
                etag, last_modified, unix_time_ms(), id],
        )?;
        Ok(())
    }

    pub fn finish_download(&self, id: i64, received: u64) -> rusqlite::Result<()> {
        self.connection.lock().expect("database mutex poisoned").execute(
            "UPDATE downloads SET status='completed',received_bytes=?1,total_bytes=COALESCE(total_bytes,?1),
             partial_path=NULL,etag=NULL,last_modified=NULL,updated_at=?2 WHERE id=?3",
            params![received as i64, unix_time_ms(), id],
        )?;
        Ok(())
    }

    pub fn fail_download(&self, id: i64) -> rusqlite::Result<()> {
        self.connection.lock().expect("database mutex poisoned").execute(
            "UPDATE downloads SET status='failed',updated_at=?1 WHERE id=?2",
            params![unix_time_ms(), id],
        )?;
        Ok(())
    }

    pub fn prepare_download_retry(
        &self, id: i64, received: u64, total: Option<u64>, partial_path: &Path,
        etag: Option<&str>, last_modified: Option<&str>,
    ) -> rusqlite::Result<()> {
        self.connection.lock().expect("database mutex poisoned").execute(
            "UPDATE downloads SET status='queued',received_bytes=?1,total_bytes=?2,
             partial_path=?3,etag=?4,last_modified=?5,updated_at=?6
             WHERE id=?7 AND status IN ('failed','cancelled','paused')",
            params![received as i64, total.map(|value| value as i64), partial_path.display().to_string(),
                etag, last_modified, unix_time_ms(), id],
        )?;
        Ok(())
    }

    /// Pause a transfer while retaining its partial file and resume validators.
    /// Include 'cancelled' to resolve the small race where the worker observes
    /// the stop token immediately before this state update reaches SQLite.
    pub fn pause_download(&self, id: i64) -> rusqlite::Result<()> {
        self.connection.lock().expect("database mutex poisoned").execute(
            "UPDATE downloads SET status='paused',updated_at=?1 WHERE id=?2 AND status IN ('queued','downloading','cancelled')",
            params![unix_time_ms(), id],
        )?;
        Ok(())
    }

    pub fn cancel_download(&self, id: i64) -> rusqlite::Result<()> {
        self.connection.lock().expect("database mutex poisoned").execute(
            "UPDATE downloads SET status='cancelled',updated_at=?1 WHERE id=?2 AND status IN ('queued','downloading')",
            params![unix_time_ms(), id],
        )?;
        Ok(())
    }

    /// Mark transfers left active by a previous process as failed on next startup.
    pub fn recover_interrupted_downloads(&self) -> rusqlite::Result<usize> {
        let changed = self.connection.lock().expect("database mutex poisoned").execute(
            "UPDATE downloads SET status='failed',updated_at=?1 WHERE status IN ('queued','downloading')",
            params![unix_time_ms()],
        )?;
        Ok(changed)
    }

    pub fn recent_downloads(&self, limit: u32) -> rusqlite::Result<Vec<DownloadEntry>> {
        let connection = self.connection.lock().expect("database mutex poisoned");
        let mut statement = connection.prepare(
            "SELECT id,url,file_path,file_name,status,received_bytes,total_bytes,created_at,partial_path,etag,last_modified
             FROM downloads ORDER BY created_at DESC LIMIT ?1",
        )?;
        let rows = statement.query_map(params![limit], |row| {
            Ok(DownloadEntry {
                id: row.get(0)?,
                url: row.get(1)?,
                file_path: row.get(2)?,
                file_name: row.get(3)?,
                status: row.get(4)?,
                received_bytes: row.get::<_, i64>(5)? as u64,
                total_bytes: row.get::<_, Option<i64>>(6)?.map(|value| value as u64),
                created_at: row.get(7)?,
                partial_path: row.get(8)?,
                etag: row.get(9)?,
                last_modified: row.get(10)?,
            })
        })?;
        rows.collect()
    }

    pub fn delete_download(&self, id: i64) -> rusqlite::Result<()> {
        self.connection.lock().expect("database mutex poisoned").execute(
            "DELETE FROM downloads WHERE id=?1", params![id],
        )?;
        Ok(())
    }

    /// Return the newest history entries first. The caller controls the maximum result count.
    pub fn recent_history(&self, limit: u32) -> rusqlite::Result<Vec<HistoryEntry>> {
        let connection = self.connection.lock().expect("database mutex poisoned");
        let mut statement = connection.prepare(
            "SELECT url,title,visited_at,visit_count FROM history
             ORDER BY visited_at DESC LIMIT ?1",
        )?;
        let rows = statement.query_map(params![limit], |row| {
            Ok(HistoryEntry {
                url: row.get(0)?,
                title: row.get(1)?,
                visited_at: row.get(2)?,
                visit_count: row.get::<_, i64>(3)? as u64,
            })
        })?;
        rows.collect()
    }

    /// Return saved pages ordered by most recently bookmarked first.
    pub fn all_bookmarks(&self) -> rusqlite::Result<Vec<BookmarkEntry>> {
        let connection = self.connection.lock().expect("database mutex poisoned");
        let mut statement = connection.prepare(
            "SELECT url,title,created_at FROM bookmarks ORDER BY created_at DESC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(BookmarkEntry {
                url: row.get(0)?,
                title: row.get(1)?,
                created_at: row.get(2)?,
            })
        })?;
        rows.collect()
    }

    pub fn delete_history_entry(&self, url: &str) -> rusqlite::Result<()> {
        self.connection.lock().expect("database mutex poisoned").execute(
            "DELETE FROM history WHERE url=?1", params![url],
        )?;
        Ok(())
    }

    pub fn clear_history(&self) -> rusqlite::Result<()> {
        self.connection.lock().expect("database mutex poisoned")
            .execute("DELETE FROM history", [])?;
        Ok(())
    }

    pub fn history_count(&self) -> rusqlite::Result<u64> {
        let count: i64 = self.connection.lock().expect("database mutex poisoned")
            .query_row("SELECT COUNT(*) FROM history", [], |row| row.get(0))?;
        Ok(count as u64)
    }

    pub fn bookmarks_count(&self) -> rusqlite::Result<u64> {
        let count: i64 = self.connection.lock().expect("database mutex poisoned")
            .query_row("SELECT COUNT(*) FROM bookmarks", [], |row| row.get(0))?;
        Ok(count as u64)
    }
}

fn profile_root_dir() -> PathBuf {
    if let Some(app_data) = std::env::var_os("APPDATA") {
        return PathBuf::from(app_data).join("VertexBrowser");
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join(".vertex-browser");
    }
    std::env::temp_dir().join("VertexBrowser")
}

/// Profile names are deliberately restricted to portable ASCII path components.
pub fn validate_profile_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() || name.len() > 32
        || !name.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return Err("Назва профілю: 1–32 символи латиницею, цифрами, дефісом або підкресленням.".to_owned());
    }
    Ok(name.to_ascii_lowercase())
}

pub fn active_profile_name() -> String {
    let marker = profile_root_dir().join("active-profile.txt");
    std::fs::read_to_string(marker)
        .ok()
        .and_then(|name| validate_profile_name(&name).ok())
        .unwrap_or_else(|| "default".to_owned())
}

pub fn list_profile_names() -> Vec<String> {
    let root = profile_root_dir();
    let mut names = vec!["default".to_owned()];
    let profiles = root.join("profiles");
    if let Ok(entries) = std::fs::read_dir(profiles) {
        for entry in entries.flatten() {
            if entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
                if let Some(name) = entry.file_name().to_str().and_then(|value| validate_profile_name(value).ok()) {
                    if name != "default" && !names.contains(&name) { names.push(name); }
                }
            }
        }
    }
    names.sort();
    names
}

/// Atomically select the profile used on the next browser start.
pub fn set_active_profile_name(name: &str) -> Result<String, String> {
    let name = validate_profile_name(name)?;
    let root = profile_root_dir();
    std::fs::create_dir_all(root.join("profiles"))
        .map_err(|error| format!("Не вдалося створити каталог профілів: {error}"))?;
    if name != "default" {
        std::fs::create_dir_all(root.join("profiles").join(&name))
            .map_err(|error| format!("Не вдалося створити профіль: {error}"))?;
    }
    let marker = root.join("active-profile.txt");
    let temporary = root.join("active-profile.txt.tmp");
    std::fs::write(&temporary, &name)
        .map_err(|error| format!("Не вдалося записати вибір профілю: {error}"))?;
    if marker.exists() { std::fs::remove_file(&marker).map_err(|error| error.to_string())?; }
    std::fs::rename(&temporary, &marker)
        .map_err(|error| format!("Не вдалося активувати профіль: {error}"))?;
    Ok(name)
}

pub fn default_profile_dir() -> PathBuf {
    let root = profile_root_dir();
    let name = active_profile_name();
    if name == "default" {
        // Preserve existing installations and their original SQLite database.
        root
    } else {
        root.join("profiles").join(name)
    }
}

fn unix_time_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH)
        .unwrap_or_default().as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn profile_names_reject_path_traversal_and_nonportable_names() {
        assert_eq!(validate_profile_name(" Work_1 ").unwrap(), "work_1");
        assert!(validate_profile_name("../outside").is_err());
        assert!(validate_profile_name("a/b").is_err());
        assert!(validate_profile_name("").is_err());
        assert!(validate_profile_name(&"x".repeat(33)).is_err());
    }

    #[test]
    fn migrates_legacy_download_table_for_resume_metadata() {
        let path = std::env::temp_dir().join(format!("vertex-legacy-profile-{}.sqlite3", uuid::Uuid::new_v4()));
        {
            let connection = Connection::open(&path).unwrap();
            connection.execute_batch(
                "CREATE TABLE downloads (
                    id INTEGER PRIMARY KEY,
                    url TEXT NOT NULL,
                    file_path TEXT NOT NULL,
                    file_name TEXT NOT NULL,
                    status TEXT NOT NULL DEFAULT 'queued',
                    received_bytes INTEGER NOT NULL DEFAULT 0,
                    total_bytes INTEGER,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                );"
            ).unwrap();
        }
        let storage = BrowserStorage::open(&path).unwrap();
        let id = storage.create_download("https://example.com/file", "file", "file").unwrap();
        let entry = storage.recent_downloads(1).unwrap().remove(0);
        assert_eq!(entry.id, id);
        // A newly queued download has no partial file until the transfer starts.
        assert_eq!(entry.partial_path, None);
        assert_eq!(entry.etag, None);
        assert_eq!(entry.last_modified, None);
        drop(storage);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn stores_history_bookmarks_and_settings() {
        let storage = BrowserStorage::open(Path::new(":memory:")).unwrap();
        storage.record_visit("https://example.com/", "Example").unwrap();
        storage.record_visit("https://example.com/", "Example updated").unwrap();
        assert_eq!(storage.history_count().unwrap(), 1);
        storage.add_bookmark("https://example.com/", "Example").unwrap();
        assert!(storage.is_bookmarked("https://example.com/").unwrap());
        assert_eq!(storage.bookmarks_count().unwrap(), 1);
        storage.set_setting("theme", "dark").unwrap();
        assert_eq!(storage.get_setting("theme").unwrap().as_deref(), Some("dark"));
        storage.remove_bookmark("https://example.com/").unwrap();
        assert!(!storage.is_bookmarked("https://example.com/").unwrap());
    }

    #[test]
    fn returns_recent_history_and_bookmarks() {
        let storage = BrowserStorage::open(Path::new(":memory:")).unwrap();
        storage.record_visit("https://one.example/", "One").unwrap();
        storage.record_visit("https://two.example/", "Two").unwrap();
        let history = storage.recent_history(10).unwrap();
        assert_eq!(history.len(), 2);
        assert!(history.iter().any(|entry| entry.url == "https://one.example/"));
        storage.add_bookmark("https://one.example/", "One").unwrap();
        let bookmarks = storage.all_bookmarks().unwrap();
        assert_eq!(bookmarks.len(), 1);
        assert_eq!(bookmarks[0].title, "One");
        storage.delete_history_entry("https://one.example/").unwrap();
        assert_eq!(storage.history_count().unwrap(), 1);
        storage.clear_history().unwrap();
        assert_eq!(storage.history_count().unwrap(), 0);
    }

    #[test]
    fn marks_interrupted_downloads_failed_on_startup() {
        let storage = BrowserStorage::open(Path::new(":memory:")).unwrap();
        let queued = storage.create_download("https://example.com/a", "a", "a").unwrap();
        let active = storage.create_download("https://example.com/b", "b", "b").unwrap();
        storage.update_download_progress(active, 12, Some(100), Path::new("active.part"), Some("\"etag-v1\""), None).unwrap();
        assert_eq!(storage.recover_interrupted_downloads().unwrap(), 2);
        let entries = storage.recent_downloads(10).unwrap();
        assert!(entries.iter().all(|entry| entry.status == "failed"));
        assert_eq!(entries.iter().find(|entry| entry.id == queued).unwrap().received_bytes, 0);
    }

    #[test]
    fn retry_resets_progress_but_keeps_download_record() {
        let storage = BrowserStorage::open(Path::new(":memory:")).unwrap();
        let id = storage.create_download("https://example.com/file", "file", "file").unwrap();
        storage.update_download_progress(
            id, 500, Some(1000), Path::new("file.part"), Some("\"etag\""), None,
        ).unwrap();
        storage.fail_download(id).unwrap();
        storage.prepare_download_retry(id, 0, None, Path::new("file.part"), None, None).unwrap();
        let entry = storage.recent_downloads(1).unwrap().remove(0);
        assert_eq!(entry.status, "queued");
        assert_eq!(entry.received_bytes, 0);
        assert_eq!(entry.total_bytes, None);
        assert_eq!(entry.partial_path.as_deref(), Some("file.part"));
        assert_eq!(entry.etag, None);
    }

    #[test]
    fn persists_download_lifecycle_and_progress() {
        let storage = BrowserStorage::open(Path::new(":memory:")).unwrap();
        let id = storage.create_download(
            "https://example.com/file.zip",
            "C:/Users/Test/Downloads/file.zip",
            "file.zip",
        ).unwrap();
        let queued = storage.recent_downloads(10).unwrap();
        assert_eq!(queued[0].status, "queued");
        storage.update_download_progress(id, 512, Some(1024), Path::new("file.part"), Some("\"etag-v1\""), Some("Wed, 21 Oct 2015 07:28:00 GMT")).unwrap();
        let downloading = storage.recent_downloads(10).unwrap();
        assert_eq!(downloading[0].received_bytes, 512);
        assert_eq!(downloading[0].total_bytes, Some(1024));
        assert_eq!(downloading[0].status, "downloading");
        assert_eq!(downloading[0].partial_path.as_deref(), Some("file.part"));
        assert_eq!(downloading[0].etag.as_deref(), Some("\"etag-v1\""));
        assert_eq!(downloading[0].last_modified.as_deref(), Some("Wed, 21 Oct 2015 07:28:00 GMT"));
        storage.finish_download(id, 1024).unwrap();
        let completed = storage.recent_downloads(10).unwrap();
        assert_eq!(completed[0].status, "completed");
        assert_eq!(completed[0].partial_path, None);
        assert_eq!(completed[0].etag, None);
        storage.delete_download(id).unwrap();
        assert!(storage.recent_downloads(10).unwrap().is_empty());
    }

    #[test]
    fn saves_and_restores_only_valid_http_sessions() {
        let storage = BrowserStorage::open(Path::new(":memory:")).unwrap();
        let urls = vec![
            "https://example.com/".to_owned(),
            "http://localhost:8080/".to_owned(),
            "file:///secret".to_owned(),
            "javascript:alert(1)".to_owned(),
            "not a url".to_owned(),
        ];
        storage.save_session(&urls, 1).unwrap();
        let (restored, active) = storage.load_session().unwrap().unwrap();
        assert_eq!(restored, vec![
            "https://example.com/".to_owned(),
            "http://localhost:8080/".to_owned(),
        ]);
        assert_eq!(active, 1);
    }

    #[test]
    fn persists_recently_closed_tabs_in_reopen_order() {
        let storage = BrowserStorage::open(Path::new(":memory:")).unwrap();
        storage.save_closed_tabs(&[
            "https://old.example/".to_owned(),
            "https://new.example/".to_owned(),
            "file:///private".to_owned(),
        ]).unwrap();
        assert_eq!(storage.load_closed_tabs().unwrap(), vec![
            "https://new.example/".to_owned(),
            "https://old.example/".to_owned(),
        ]);
    }

    #[test]
    fn lists_and_revokes_persisted_site_permissions() {
        let storage = BrowserStorage::open(Path::new(":memory:")).unwrap();
        storage.set_setting("permission:https://example.com:Geolocation", "allow").unwrap();
        storage.set_setting("permission:https://example.com:Camera", "deny").unwrap();
        let entries = storage.site_permissions().unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().any(|(key, value)| key.ends_with(":Geolocation") && value == "allow"));
        storage.delete_setting("permission:https://example.com:Geolocation").unwrap();
        let remaining = storage.site_permissions().unwrap();
        assert_eq!(remaining.len(), 1);
        assert!(remaining[0].0.ends_with(":Camera"));
    }

    #[test]
    fn clearing_recently_closed_tabs_persists_an_empty_list() {
        let storage = BrowserStorage::open(Path::new(":memory:")).unwrap();
        storage.save_closed_tabs(&["https://example.com/".to_owned()]).unwrap();
        assert_eq!(storage.load_closed_tabs().unwrap().len(), 1);
        storage.save_closed_tabs(&[]).unwrap();
        assert!(storage.load_closed_tabs().unwrap().is_empty());
    }

    #[test]
    fn limits_recently_closed_tabs_to_ten() {
        let storage = BrowserStorage::open(Path::new(":memory:")).unwrap();
        let urls = (0..15).map(|index| format!("https://{index}.example/")).collect::<Vec<_>>();
        storage.save_closed_tabs(&urls).unwrap();
        let restored = storage.load_closed_tabs().unwrap();
        assert_eq!(restored.len(), 10);
        assert_eq!(restored[0], "https://14.example/");
        assert_eq!(restored[9], "https://5.example/");
    }

    #[test]
    fn ignores_corrupt_or_empty_saved_sessions() {
        let storage = BrowserStorage::open(Path::new(":memory:")).unwrap();
        storage.set_setting("browser_session_v1", "not-json").unwrap();
        assert!(storage.load_session().unwrap().is_none());
        storage.save_session(&["file:///private".to_owned()], 0).unwrap();
        assert!(storage.load_session().unwrap().is_none());
    }

    #[test]
    fn does_not_record_internal_pages() {
        let storage = BrowserStorage::open(Path::new(":memory:")).unwrap();
        storage.record_visit("about:blank", "New tab").unwrap();
        assert_eq!(storage.history_count().unwrap(), 0);
    }
}
