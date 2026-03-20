use anyhow::Result;
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub struct RoverDb {
    conn: Arc<Mutex<Connection>>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ClipboardEntry {
    pub id: i64,
    pub content: String,
    pub timestamp: i64,
}

impl RoverDb {
    pub fn open() -> Result<Self> {
        let path = Self::db_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&path)?;
        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.migrate()?;
        Ok(db)
    }

    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.migrate()?;
        Ok(db)
    }

    fn db_path() -> Result<PathBuf> {
        let config = dirs::config_dir().ok_or_else(|| anyhow::anyhow!("no config dir"))?;
        Ok(config.join("rover").join("rover.db"))
    }

    fn migrate(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS query_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                query TEXT NOT NULL,
                timestamp INTEGER NOT NULL DEFAULT (unixepoch())
            );

            CREATE TABLE IF NOT EXISTS clipboard_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                content TEXT NOT NULL,
                timestamp INTEGER NOT NULL DEFAULT (unixepoch())
            );

            CREATE TABLE IF NOT EXISTS command_usage (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                command_name TEXT NOT NULL,
                timestamp INTEGER NOT NULL DEFAULT (unixepoch())
            );

            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_query_history_timestamp
                ON query_history (timestamp);

            CREATE INDEX IF NOT EXISTS idx_clipboard_history_timestamp
                ON clipboard_history (timestamp);

            CREATE INDEX IF NOT EXISTS idx_command_usage_timestamp
                ON command_usage (timestamp);
            ",
        )?;
        Ok(())
    }

    // --- Query History ---

    pub fn add_query(&self, query: &str) -> Result<()> {
        if query.trim().is_empty() {
            return Ok(());
        }
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM query_history WHERE query = ?1",
            rusqlite::params![query],
        )?;
        conn.execute(
            "INSERT INTO query_history (query) VALUES (?1)",
            rusqlite::params![query],
        )?;
        conn.execute(
            "DELETE FROM query_history WHERE id NOT IN (
                SELECT id FROM query_history ORDER BY timestamp DESC LIMIT 200
            )",
            [],
        )?;
        Ok(())
    }

    pub fn clear_query_history(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM query_history", [])?;
        Ok(())
    }

    pub fn list_query_history(&self) -> Result<Vec<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT query FROM query_history ORDER BY timestamp ASC")?;
        let rows = stmt
            .query_map([], |row| row.get(0))?
            .collect::<std::result::Result<Vec<String>, _>>()?;
        Ok(rows)
    }

    // --- Clipboard History ---

    pub fn add_clipboard_entry(&self, content: &str) -> Result<()> {
        if content.trim().is_empty() {
            return Ok(());
        }
        let conn = self.conn.lock().unwrap();
        // Deduplicate: remove existing identical entry
        conn.execute(
            "DELETE FROM clipboard_history WHERE content = ?1",
            rusqlite::params![content],
        )?;
        conn.execute(
            "INSERT INTO clipboard_history (content) VALUES (?1)",
            rusqlite::params![content],
        )?;
        // Keep max 500 entries
        conn.execute(
            "DELETE FROM clipboard_history WHERE id NOT IN (
                SELECT id FROM clipboard_history ORDER BY timestamp DESC LIMIT 500
            )",
            [],
        )?;
        Ok(())
    }

    pub fn list_clipboard_history(&self) -> Result<Vec<ClipboardEntry>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, content, timestamp FROM clipboard_history ORDER BY timestamp DESC, id DESC",
        )?;
        let rows = stmt
            .query_map([], |row| {
                Ok(ClipboardEntry {
                    id: row.get(0)?,
                    content: row.get(1)?,
                    timestamp: row.get(2)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn delete_clipboard_entry(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM clipboard_history WHERE id = ?1",
            rusqlite::params![id],
        )?;
        Ok(())
    }

    pub fn clear_clipboard_history(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM clipboard_history", [])?;
        Ok(())
    }

    // --- Command Usage ---

    pub fn record_command_usage(&self, command_name: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO command_usage (command_name) VALUES (?1)",
            rusqlite::params![command_name],
        )?;
        Ok(())
    }

    // --- Settings ---

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = ?1")?;
        let mut rows = stmt.query_map(rusqlite::params![key], |row| row.get(0))?;
        match rows.next() {
            Some(Ok(val)) => Ok(Some(val)),
            _ => Ok(None),
        }
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
            rusqlite::params![key, value],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_history() {
        let db = RoverDb::open_in_memory().unwrap();
        db.add_query("hello").unwrap();
        db.add_query("world").unwrap();
        let history = db.list_query_history().unwrap();
        assert_eq!(history, vec!["hello", "world"]);

        // Duplicate moves to end
        db.add_query("hello").unwrap();
        let history = db.list_query_history().unwrap();
        assert_eq!(history, vec!["world", "hello"]);
    }

    #[test]
    fn test_clear_query_history() {
        let db = RoverDb::open_in_memory().unwrap();
        db.add_query("hello").unwrap();
        db.add_query("world").unwrap();
        assert_eq!(db.list_query_history().unwrap().len(), 2);

        db.clear_query_history().unwrap();
        assert!(db.list_query_history().unwrap().is_empty());
    }

    #[test]
    fn test_clipboard_history() {
        let db = RoverDb::open_in_memory().unwrap();
        db.add_clipboard_entry("first").unwrap();
        db.add_clipboard_entry("second").unwrap();
        let entries = db.list_clipboard_history().unwrap();
        assert_eq!(entries.len(), 2);
        // Newest first (DESC order)
        assert_eq!(entries[0].content, "second");
        assert_eq!(entries[1].content, "first");

        // Dedup: re-adding "first" moves it to top
        db.add_clipboard_entry("first").unwrap();
        let entries = db.list_clipboard_history().unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].content, "first");
    }

    #[test]
    fn test_settings() {
        let db = RoverDb::open_in_memory().unwrap();
        assert_eq!(db.get_setting("theme").unwrap(), None);
        db.set_setting("theme", "dark").unwrap();
        assert_eq!(db.get_setting("theme").unwrap(), Some("dark".to_string()));
    }
}
