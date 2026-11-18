use rusqlite::{params, Connection, OptionalExtension};
use std::{
    path::{Path, PathBuf},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

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
             );",
        )?;
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

pub fn default_profile_dir() -> PathBuf {
    if let Some(app_data) = std::env::var_os("APPDATA") {
        return PathBuf::from(app_data).join("VertexBrowser");
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join(".vertex-browser");
    }
    std::env::temp_dir().join("VertexBrowser")
}

fn unix_time_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH)
        .unwrap_or_default().as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn does_not_record_internal_pages() {
        let storage = BrowserStorage::open(Path::new(":memory:")).unwrap();
        storage.record_visit("about:blank", "New tab").unwrap();
        assert_eq!(storage.history_count().unwrap(), 0);
    }
}
