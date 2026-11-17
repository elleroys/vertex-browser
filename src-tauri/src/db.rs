use crate::error::VortexResult;
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BookmarkFolder {
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub sort_order: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BookmarkItem {
    pub id: String,
    pub title: String,
    pub url: String,
    pub folder_id: Option<String>,
    pub favicon: Option<String>,
    pub on_bar: bool,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryItem {
    pub id: String,
    pub title: String,
    pub url: String,
    pub favicon: Option<String>,
    pub visited_at: i64,
    pub visit_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadRecord {
    pub id: String,
    pub filename: String,
    pub url: String,
    pub save_path: String,
    pub total_bytes: i64,
    pub received_bytes: i64,
    pub status: String, // "downloading" | "paused" | "completed" | "failed" | "canceled"
    pub mime_type: Option<String>,
    pub supports_resume: bool,
    pub started_at: i64,
    pub completed_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionTabRecord {
    pub id: String,
    pub url: String,
    pub title: String,
    pub favicon: Option<String>,
    pub is_pinned: bool,
    pub is_active: bool,
    pub sort_order: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserSettings {
    pub theme: String,
    pub startup_page: String,
    pub search_engine: String,
    pub show_bookmarks_bar: bool,
    pub adblock_enabled: bool,
    pub adblock_auto_update: bool,
    pub memory_saver_mode: String, // "off" | "standard" | "aggressive"
    pub memory_saver_suspend_mins: i64,
    pub memory_saver_discard_mins: i64,
    pub download_dir: String,
    pub ask_download_location: bool,
    pub do_not_track: bool,
    pub block_third_party_cookies: bool,
    pub restore_tabs_on_startup: bool,
}

impl Default for BrowserSettings {
    fn default() -> Self {
        Self {
            theme: "dark".to_string(),
            startup_page: "vortex://newtab".to_string(),
            search_engine: "google".to_string(),
            show_bookmarks_bar: true,
            adblock_enabled: true,
            adblock_auto_update: true,
            memory_saver_mode: "standard".to_string(),
            memory_saver_suspend_mins: 10,
            memory_saver_discard_mins: 30,
            download_dir: "Downloads".to_string(),
            ask_download_location: false,
            do_not_track: true,
            block_third_party_cookies: true,
            restore_tabs_on_startup: true,
        }
    }
}

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn open(path: &Path) -> VortexResult<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;

        // Оптимізація SQLite для низької затримки та мінімального споживання RAM
        conn.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            PRAGMA busy_timeout = 5000;
            PRAGMA foreign_keys = ON;
            PRAGMA temp_store = MEMORY;
            PRAGMA cache_size = -16000;
            PRAGMA mmap_size = 67108864;
            ",
        )?;

        let db = Self {
            conn: Mutex::new(conn),
        };
        db.init_schema()?;
        Ok(db)
    }

    fn init_schema(&self) -> VortexResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS bookmark_folders (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                parent_id TEXT,
                sort_order INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS bookmarks (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                url TEXT NOT NULL,
                folder_id TEXT,
                favicon TEXT,
                on_bar INTEGER NOT NULL DEFAULT 1,
                created_at INTEGER NOT NULL,
                FOREIGN KEY(folder_id) REFERENCES bookmark_folders(id) ON DELETE SET NULL
            );

            CREATE TABLE IF NOT EXISTS history (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                url TEXT NOT NULL,
                favicon TEXT,
                visited_at INTEGER NOT NULL,
                visit_count INTEGER NOT NULL DEFAULT 1
            );
            CREATE INDEX IF NOT EXISTS idx_history_visited ON history(visited_at DESC);
            CREATE INDEX IF NOT EXISTS idx_history_url ON history(url);

            CREATE TABLE IF NOT EXISTS downloads (
                id TEXT PRIMARY KEY,
                filename TEXT NOT NULL,
                url TEXT NOT NULL,
                save_path TEXT NOT NULL,
                total_bytes INTEGER NOT NULL DEFAULT 0,
                received_bytes INTEGER NOT NULL DEFAULT 0,
                status TEXT NOT NULL,
                mime_type TEXT,
                supports_resume INTEGER NOT NULL DEFAULT 0,
                started_at INTEGER NOT NULL,
                completed_at INTEGER
            );

            CREATE TABLE IF NOT EXISTS tabs_session (
                id TEXT PRIMARY KEY,
                url TEXT NOT NULL,
                title TEXT NOT NULL,
                favicon TEXT,
                is_pinned INTEGER NOT NULL DEFAULT 0,
                is_active INTEGER NOT NULL DEFAULT 0,
                sort_order INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS adblock_whitelist (
                domain TEXT PRIMARY KEY,
                added_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS adblock_stats (
                domain TEXT PRIMARY KEY,
                blocked_count INTEGER NOT NULL DEFAULT 0,
                last_blocked_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS adblock_lists (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                url TEXT NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 1,
                rule_count INTEGER NOT NULL DEFAULT 0,
                last_updated_at INTEGER
            );

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
            CREATE INDEX IF NOT EXISTS idx_passwords_origin ON password_entries(origin);

            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value_json TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS benchmark_runs (
                id TEXT PRIMARY KEY,
                created_at INTEGER NOT NULL,
                payload_json TEXT NOT NULL
            );
            ",
        )?;

        // Початкові папки закладок, якщо база порожня
        let folder_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM bookmark_folders",
            [],
            |r| r.get(0),
        )?;
        if folder_count == 0 {
            conn.execute(
                "INSERT INTO bookmark_folders (id, name, parent_id, sort_order) VALUES (?1, ?2, NULL, 0)",
                params!["quick-bar", "Панель закладок"],
            )?;
            conn.execute(
                "INSERT INTO bookmark_folders (id, name, parent_id, sort_order) VALUES (?1, ?2, NULL, 1)",
                params!["work", "Робота і розробка"],
            )?;
        }

        Ok(())
    }

    pub fn get_settings(&self) -> VortexResult<BrowserSettings> {
        let conn = self.conn.lock().unwrap();
        let raw: Option<String> = conn
            .query_row(
                "SELECT value_json FROM settings WHERE key = 'browser_settings'",
                [],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(json) = raw {
            if let Ok(parsed) = serde_json::from_str::<BrowserSettings>(&json) {
                return Ok(parsed);
            }
        }
        Ok(BrowserSettings::default())
    }

    pub fn save_settings(&self, settings: &BrowserSettings) -> VortexResult<()> {
        let conn = self.conn.lock().unwrap();
        let json = serde_json::to_string(settings)
            .map_err(|e| crate::error::VortexError::Validation(e.to_string()))?;
        conn.execute(
            "INSERT INTO settings (key, value_json) VALUES ('browser_settings', ?1)
             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json",
            params![json],
        )?;
        Ok(())
    }

    // --- AdBlock Whitelist & Stats ---

    pub fn list_adblock_whitelist(&self) -> VortexResult<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT domain FROM adblock_whitelist ORDER BY added_at DESC")?;
        let rows = stmt
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn add_adblock_whitelist(&self, domain: &str) -> VortexResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO adblock_whitelist (domain, added_at) VALUES (?1, ?2)",
            params![domain.to_lowercase(), Utc::now().timestamp_millis()],
        )?;
        Ok(())
    }

    pub fn remove_adblock_whitelist(&self, domain: &str) -> VortexResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM adblock_whitelist WHERE domain = ?1",
            params![domain.to_lowercase()],
        )?;
        Ok(())
    }

    pub fn increment_adblock_stat(&self, domain: &str, delta: i64) -> VortexResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO adblock_stats (domain, blocked_count, last_blocked_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(domain) DO UPDATE SET
               blocked_count = blocked_count + excluded.blocked_count,
               last_blocked_at = excluded.last_blocked_at",
            params![domain.to_lowercase(), delta, Utc::now().timestamp_millis()],
        )?;
        Ok(())
    }

    pub fn total_adblock_blocked(&self) -> VortexResult<i64> {
        let conn = self.conn.lock().unwrap();
        let total: Option<i64> =
            conn.query_row("SELECT SUM(blocked_count) FROM adblock_stats", [], |r| r.get(0))?;
        Ok(total.unwrap_or(0))
    }

    // --- Закладки ---

    pub fn list_bookmark_folders(&self) -> VortexResult<Vec<BookmarkFolder>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, parent_id, sort_order FROM bookmark_folders ORDER BY sort_order ASC",
        )?;
        let rows = stmt
            .query_map([], |r| {
                Ok(BookmarkFolder {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    parent_id: r.get(2)?,
                    sort_order: r.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn list_bookmarks(&self) -> VortexResult<Vec<BookmarkItem>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, title, url, folder_id, favicon, on_bar, created_at FROM bookmarks ORDER BY created_at DESC",
        )?;
        let rows = stmt
            .query_map([], |r| {
                Ok(BookmarkItem {
                    id: r.get(0)?,
                    title: r.get(1)?,
                    url: r.get(2)?,
                    folder_id: r.get(3)?,
                    favicon: r.get(4)?,
                    on_bar: r.get::<_, i64>(5)? != 0,
                    created_at: r.get(6)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn upsert_bookmark(&self, item: &BookmarkItem) -> VortexResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO bookmarks (id, title, url, folder_id, favicon, on_bar, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(id) DO UPDATE SET
               title = excluded.title,
               url = excluded.url,
               folder_id = excluded.folder_id,
               favicon = excluded.favicon,
               on_bar = excluded.on_bar",
            params![
                item.id,
                item.title,
                item.url,
                item.folder_id,
                item.favicon,
                if item.on_bar { 1 } else { 0 },
                item.created_at
            ],
        )?;
        Ok(())
    }

    pub fn delete_bookmark(&self, id: &str) -> VortexResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM bookmarks WHERE id = ?1", params![id])?;
        Ok(())
    }

    // --- Історія ---

    pub fn record_history(&self, title: &str, url: &str, favicon: Option<&str>) -> VortexResult<()> {
        if url.starts_with("vortex://") || url.starts_with("about:") {
            return Ok(());
        }
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().timestamp_millis();
        let existing_id: Option<String> = conn
            .query_row(
                "SELECT id FROM history WHERE url = ?1 ORDER BY visited_at DESC LIMIT 1",
                params![url],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(id) = existing_id {
            conn.execute(
                "UPDATE history SET title = ?1, favicon = COALESCE(?2, favicon), visited_at = ?3, visit_count = visit_count + 1 WHERE id = ?4",
                params![title, favicon, now, id],
            )?;
        } else {
            let id = uuid::Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO history (id, title, url, favicon, visited_at, visit_count) VALUES (?1, ?2, ?3, ?4, ?5, 1)",
                params![id, title, url, favicon, now],
            )?;
        }
        Ok(())
    }

    pub fn list_history(&self, query: Option<&str>, limit: i64) -> VortexResult<Vec<HistoryItem>> {
        let conn = self.conn.lock().unwrap();
        let q = query.unwrap_or("").trim().to_lowercase();
        let pattern = format!("%{}%", q);
        let mut stmt = conn.prepare(
            "SELECT id, title, url, favicon, visited_at, visit_count
             FROM history
             WHERE (?1 = '' OR LOWER(title) LIKE ?2 OR LOWER(url) LIKE ?2)
             ORDER BY visited_at DESC
             LIMIT ?3",
        )?;
        let rows = stmt
            .query_map(params![q, pattern, limit], |r| {
                Ok(HistoryItem {
                    id: r.get(0)?,
                    title: r.get(1)?,
                    url: r.get(2)?,
                    favicon: r.get(3)?,
                    visited_at: r.get(4)?,
                    visit_count: r.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn delete_history_item(&self, id: &str) -> VortexResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM history WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn clear_history_since(&self, min_timestamp_ms: i64) -> VortexResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM history WHERE visited_at >= ?1",
            params![min_timestamp_ms],
        )?;
        Ok(())
    }

    // --- Завантаження ---

    pub fn upsert_download(&self, dl: &DownloadRecord) -> VortexResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO downloads (id, filename, url, save_path, total_bytes, received_bytes, status, mime_type, supports_resume, started_at, completed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(id) DO UPDATE SET
               total_bytes = excluded.total_bytes,
               received_bytes = excluded.received_bytes,
               status = excluded.status,
               supports_resume = excluded.supports_resume,
               completed_at = excluded.completed_at",
            params![
                dl.id,
                dl.filename,
                dl.url,
                dl.save_path,
                dl.total_bytes,
                dl.received_bytes,
                dl.status,
                dl.mime_type,
                if dl.supports_resume { 1 } else { 0 },
                dl.started_at,
                dl.completed_at
            ],
        )?;
        Ok(())
    }

    pub fn list_downloads(&self) -> VortexResult<Vec<DownloadRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, filename, url, save_path, total_bytes, received_bytes, status, mime_type, supports_resume, started_at, completed_at
             FROM downloads ORDER BY started_at DESC",
        )?;
        let rows = stmt
            .query_map([], |r| {
                Ok(DownloadRecord {
                    id: r.get(0)?,
                    filename: r.get(1)?,
                    url: r.get(2)?,
                    save_path: r.get(3)?,
                    total_bytes: r.get(4)?,
                    received_bytes: r.get(5)?,
                    status: r.get(6)?,
                    mime_type: r.get(7)?,
                    supports_resume: r.get::<_, i64>(8)? != 0,
                    started_at: r.get(9)?,
                    completed_at: r.get(10)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    // --- Сесія вкладок ---

    pub fn save_session_tabs(&self, tabs: &[SessionTabRecord]) -> VortexResult<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM tabs_session", [])?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO tabs_session (id, url, title, favicon, is_pinned, is_active, sort_order)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )?;
            for t in tabs {
                stmt.execute(params![
                    t.id,
                    t.url,
                    t.title,
                    t.favicon,
                    if t.is_pinned { 1 } else { 0 },
                    if t.is_active { 1 } else { 0 },
                    t.sort_order
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn load_session_tabs(&self) -> VortexResult<Vec<SessionTabRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, url, title, favicon, is_pinned, is_active, sort_order FROM tabs_session ORDER BY sort_order ASC",
        )?;
        let rows = stmt
            .query_map([], |r| {
                Ok(SessionTabRecord {
                    id: r.get(0)?,
                    url: r.get(1)?,
                    title: r.get(2)?,
                    favicon: r.get(3)?,
                    is_pinned: r.get::<_, i64>(4)? != 0,
                    is_active: r.get::<_, i64>(5)? != 0,
                    sort_order: r.get(6)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    // --- Сховище паролів ---

    pub fn get_vault_meta(&self) -> VortexResult<Option<(String, String, String)>> {
        let conn = self.conn.lock().unwrap();
        let row = conn
            .query_row(
                "SELECT salt_b64, verifier_enc, kdf_params FROM vault_meta WHERE id = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        Ok(row)
    }

    pub fn save_vault_meta(
        &self,
        salt_b64: &str,
        verifier_enc: &str,
        kdf_params: &str,
    ) -> VortexResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO vault_meta (id, salt_b64, verifier_enc, kdf_params, created_at)
             VALUES (1, ?1, ?2, ?3, ?4)",
            params![salt_b64, verifier_enc, kdf_params, Utc::now().timestamp_millis()],
        )?;
        Ok(())
    }

    pub fn upsert_password_row(&self, row: &EncryptedVaultRow) -> VortexResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO password_entries (id, origin, title, username_enc, password_enc, notes_enc, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(id) DO UPDATE SET
               origin = excluded.origin,
               title = excluded.title,
               username_enc = excluded.username_enc,
               password_enc = excluded.password_enc,
               notes_enc = excluded.notes_enc,
               updated_at = excluded.updated_at",
            params![
                row.id,
                row.origin,
                row.title,
                row.username_enc,
                row.password_enc,
                row.notes_enc,
                row.created_at,
                row.updated_at
            ],
        )?;
        Ok(())
    }

    pub fn list_password_rows(&self) -> VortexResult<Vec<EncryptedVaultRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, origin, title, username_enc, password_enc, notes_enc, created_at, updated_at
             FROM password_entries ORDER BY updated_at DESC",
        )?;
        let rows = stmt
            .query_map([], |r| {
                Ok(EncryptedVaultRow {
                    id: r.get(0)?,
                    origin: r.get(1)?,
                    title: r.get(2)?,
                    username_enc: r.get(3)?,
                    password_enc: r.get(4)?,
                    notes_enc: r.get(5)?,
                    created_at: r.get(6)?,
                    updated_at: r.get(7)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn delete_password_row(&self, id: &str) -> VortexResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM password_entries WHERE id = ?1", params![id])?;
        Ok(())
    }
}
