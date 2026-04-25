use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use rand::RngCore;
use rusqlite::Connection;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// RoverDb initialization options.
#[derive(Debug, Clone, Default)]
pub struct RoverDbOptions {
    /// If true, allow the database to open without encryption (not recommended).
    /// If false (default), encryption is required and opening without it fails.
    pub allow_unencrypted: bool,
}

pub struct RoverDb {
    conn: Arc<Mutex<Connection>>,
    cipher_key: Option<[u8; 32]>,
    is_encrypted: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ClipboardEntry {
    pub id: i64,
    pub content: String,
    pub timestamp: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HistoryRow {
    pub id: i64,
    pub url: String,
    pub title: String,
    pub visited_at: i64,
    pub visit_count: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BookmarkRow {
    pub id: i64,
    pub url: String,
    pub title: String,
    pub created_at: i64,
}

impl RoverDb {
    /// Open the database with default options (encryption required).
    pub fn open() -> Result<Self> {
        Self::open_with_options(RoverDbOptions::default())
    }

    /// Open the database with custom options.
    pub fn open_with_options(options: RoverDbOptions) -> Result<Self> {
        let path = Self::db_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(&path)?;

        let cipher_key = match Self::load_or_create_db_key() {
            Ok(key) => {
                log::info!("Encryption key loaded from Keychain");
                Some(key)
            },
            Err(e) => {
                log::error!(
                    "Failed to load or create encryption key: {}. Clipboard history will be stored UNENCRYPTED.",
                    e
                );
                if !options.allow_unencrypted {
                    return Err(anyhow::anyhow!(
                        "Encryption is required but encryption key is unavailable: {}. \
                         Set allow_unencrypted=true in RoverDbOptions to allow unencrypted storage (not recommended).",
                        e
                    ));
                }
                log::warn!("Proceeding without encryption - clipboard history will be stored in plaintext!");
                None
            },
        };

        let is_encrypted = cipher_key.is_some();
        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
            cipher_key,
            is_encrypted,
        };
        db.migrate()?;
        if db.cipher_key.is_some() {
            db.migrate_encrypt_existing()?;
        }
        Ok(db)
    }

    /// Returns whether this database instance is using encryption.
    pub fn is_encrypted(&self) -> bool {
        self.is_encrypted
    }

    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
            cipher_key: None,
            is_encrypted: false,
        };
        db.migrate()?;
        Ok(db)
    }

    fn db_path() -> Result<PathBuf> {
        let config = dirs::config_dir().ok_or_else(|| anyhow::anyhow!("no config dir"))?;
        Ok(config.join("rover").join("rover.db"))
    }

    // ── Keychain-backed encryption key ────────────────────────────────────────

    fn load_or_create_db_key() -> Result<[u8; 32]> {
        let result = std::process::Command::new("security")
            .args(["find-generic-password", "-s", "rover.db.key", "-w"])
            .output();

        if let Ok(out) = result {
            if out.status.success() {
                let hex = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if hex.len() == 64 {
                    let bytes: std::result::Result<Vec<u8>, _> = (0..64)
                        .step_by(2)
                        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16))
                        .collect();
                    if let Ok(b) = bytes {
                        if b.len() == 32 {
                            let mut key = [0u8; 32];
                            key.copy_from_slice(&b);
                            return Ok(key);
                        }
                    }
                }
            }
        }

        // No key found — generate one and store in Keychain
        let mut key = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut key);
        let hex: String = key.iter().map(|b| format!("{:02x}", b)).collect();
        let status = std::process::Command::new("security")
            .args([
                "add-generic-password",
                "-s",
                "rover.db.key",
                "-a",
                "rover",
                "-w",
                &hex,
            ])
            .status()
            .context("failed to execute security command")?;

        if !status.success() {
            return Err(anyhow::anyhow!(
                "Failed to store encryption key in Keychain (exit code: {:?}). \
                 Clipboard history will not be encrypted.",
                status.code()
            ));
        }
        log::info!("Generated and stored new encryption key in Keychain");
        Ok(key)
    }

    // ── Encryption helpers ────────────────────────────────────────────────────

    fn encrypt(&self, plaintext: &str) -> String {
        let key = match &self.cipher_key {
            Some(k) => k,
            None => return plaintext.to_string(),
        };
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
        let mut nonce_bytes = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        match cipher.encrypt(nonce, plaintext.as_bytes()) {
            Ok(ciphertext) => {
                let mut combined = nonce_bytes.to_vec();
                combined.extend(ciphertext);
                BASE64.encode(&combined)
            },
            Err(e) => {
                log::error!("clipboard encryption failed, storing encrypted marker: {e}");
                // Never store plaintext on encryption failure — use a placeholder
                "[encryption failed]".to_string()
            },
        }
    }

    fn decrypt(&self, stored: &str) -> String {
        let key = match &self.cipher_key {
            Some(k) => k,
            None => return stored.to_string(),
        };
        let decoded = match BASE64.decode(stored) {
            Ok(d) if d.len() > 12 => d,
            _ => return stored.to_string(), // legacy plaintext fallback
        };
        let (nonce_bytes, ciphertext) = decoded.split_at(12);
        let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
        let nonce = Nonce::from_slice(nonce_bytes);
        match cipher.decrypt(nonce, ciphertext) {
            Ok(plaintext) => String::from_utf8_lossy(&plaintext).into_owned(),
            Err(_) => stored.to_string(), // legacy plaintext fallback
        }
    }

    /// Stable deduplication identifier: SHA-256(key || plaintext).
    /// When no key is present (tests/fallback), returns the plaintext itself.
    fn content_hash(&self, plaintext: &str) -> String {
        let key = match &self.cipher_key {
            Some(k) => k,
            None => return plaintext.to_string(),
        };
        let mut h = Sha256::new();
        h.update(key);
        h.update(plaintext.as_bytes());
        h.finalize().iter().map(|b| format!("{:02x}", b)).collect()
    }

    // ── Schema migrations ─────────────────────────────────────────────────────

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
                content_hash TEXT NOT NULL DEFAULT '',
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

            CREATE TABLE IF NOT EXISTS browser_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                url TEXT NOT NULL UNIQUE,
                title TEXT NOT NULL DEFAULT '',
                visited_at INTEGER NOT NULL DEFAULT (unixepoch()),
                visit_count INTEGER NOT NULL DEFAULT 1
            );
            CREATE INDEX IF NOT EXISTS idx_browser_history_visited_at ON browser_history(visited_at DESC);
            CREATE INDEX IF NOT EXISTS idx_browser_history_url ON browser_history(url);

            CREATE TABLE IF NOT EXISTS bookmarks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                url TEXT NOT NULL UNIQUE,
                title TEXT NOT NULL DEFAULT '',
                created_at INTEGER NOT NULL DEFAULT (unixepoch())
            );

            CREATE TABLE IF NOT EXISTS browser_session (
                tab_index INTEGER NOT NULL,
                url TEXT NOT NULL,
                title TEXT NOT NULL DEFAULT '',
                is_active INTEGER NOT NULL DEFAULT 0
            );
            ",
        )?;
        // Add content_hash column to existing databases that pre-date this schema
        let _ = conn.execute(
            "ALTER TABLE clipboard_history ADD COLUMN content_hash TEXT NOT NULL DEFAULT ''",
            [],
        );
        Ok(())
    }

    /// One-time migration: encrypt any legacy plaintext clipboard rows.
    /// Rows with an empty content_hash are pre-encryption and need upgrading.
    fn migrate_encrypt_existing(&self) -> Result<()> {
        let already = {
            let conn = self.conn.lock().unwrap();
            conn.query_row(
                "SELECT value FROM settings WHERE key = 'clipboard_encrypted_v1'",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap_or_default()
                == "true"
        };
        if already {
            return Ok(());
        }

        let rows: Vec<(i64, String)> = {
            let conn = self.conn.lock().unwrap();
            let mut stmt =
                conn.prepare("SELECT id, content FROM clipboard_history WHERE content_hash = ''")?;
            let r: std::result::Result<Vec<(i64, String)>, rusqlite::Error> = stmt
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
                .collect();
            r?
        };

        for (id, content) in rows {
            let encrypted = self.encrypt(&content);
            let hash = self.content_hash(&content);
            let conn = self.conn.lock().unwrap();
            conn.execute(
                "UPDATE clipboard_history SET content = ?1, content_hash = ?2 WHERE id = ?3",
                rusqlite::params![encrypted, hash, id],
            )?;
        }

        {
            let conn = self.conn.lock().unwrap();
            conn.execute(
                "INSERT OR REPLACE INTO settings (key, value) VALUES ('clipboard_encrypted_v1', 'true')",
                [],
            )?;
        }
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
        let encrypted = self.encrypt(content);
        let hash = self.content_hash(content);
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM clipboard_history WHERE content_hash = ?1",
            rusqlite::params![hash],
        )?;
        conn.execute(
            "INSERT INTO clipboard_history (content, content_hash) VALUES (?1, ?2)",
            rusqlite::params![encrypted, hash],
        )?;
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
        let raw: std::result::Result<Vec<(i64, String, i64)>, rusqlite::Error> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
            .collect();
        let raw = raw?;
        drop(stmt);
        drop(conn);
        Ok(raw
            .into_iter()
            .map(|(id, content, timestamp)| ClipboardEntry {
                id,
                content: self.decrypt(&content),
                timestamp,
            })
            .collect())
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
        conn.execute(
            "DELETE FROM command_usage WHERE id NOT IN (
                SELECT id FROM command_usage ORDER BY timestamp DESC LIMIT 1000
            )",
            [],
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

    // --- Browser History ---

    /// Upsert a URL into browser_history, incrementing visit_count on conflict.
    pub fn add_history(&self, url: &str, title: &str) -> Result<()> {
        if url.trim().is_empty() || url == "about:blank" {
            return Ok(());
        }
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO browser_history (url, title, visited_at, visit_count)
             VALUES (?1, ?2, unixepoch(), 1)
             ON CONFLICT(url) DO UPDATE SET
               title = excluded.title,
               visited_at = excluded.visited_at,
               visit_count = visit_count + 1",
            rusqlite::params![url, title],
        )?;
        conn.execute(
            "DELETE FROM browser_history WHERE id NOT IN (
                SELECT id FROM browser_history ORDER BY visited_at DESC LIMIT 5000
            )",
            [],
        )?;
        Ok(())
    }

    pub fn search_history(&self, query: &str, limit: usize) -> Result<Vec<HistoryRow>> {
        let conn = self.conn.lock().unwrap();
        let escaped = query.replace('%', "\\%").replace('_', "\\_");
        let pattern = format!("%{escaped}%");
        let mut stmt = conn.prepare(
            "SELECT id, url, title, visited_at, visit_count FROM browser_history
             WHERE url LIKE ?1 ESCAPE '\\' OR title LIKE ?1 ESCAPE '\\'
             ORDER BY visited_at DESC
             LIMIT ?2",
        )?;
        let rows = stmt
            .query_map(rusqlite::params![pattern, limit as i64], |row| {
                Ok(HistoryRow {
                    id: row.get(0)?,
                    url: row.get(1)?,
                    title: row.get(2)?,
                    visited_at: row.get(3)?,
                    visit_count: row.get(4)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn recent_history(&self, limit: usize) -> Result<Vec<HistoryRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, url, title, visited_at, visit_count FROM browser_history
             ORDER BY visited_at DESC
             LIMIT ?1",
        )?;
        let rows = stmt
            .query_map(rusqlite::params![limit as i64], |row| {
                Ok(HistoryRow {
                    id: row.get(0)?,
                    url: row.get(1)?,
                    title: row.get(2)?,
                    visited_at: row.get(3)?,
                    visit_count: row.get(4)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn clear_history(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM browser_history", [])?;
        Ok(())
    }

    // --- Bookmarks ---

    pub fn add_bookmark(&self, url: &str, title: &str) -> Result<()> {
        if url.trim().is_empty() {
            return Ok(());
        }
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO bookmarks (url, title, created_at) VALUES (?1, ?2, unixepoch())",
            rusqlite::params![url, title],
        )?;
        Ok(())
    }

    pub fn remove_bookmark(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM bookmarks WHERE id = ?1", rusqlite::params![id])?;
        Ok(())
    }

    pub fn search_bookmarks(&self, query: &str) -> Result<Vec<BookmarkRow>> {
        let conn = self.conn.lock().unwrap();
        let escaped = query.replace('%', "\\%").replace('_', "\\_");
        let pattern = format!("%{escaped}%");
        let mut stmt = conn.prepare(
            "SELECT id, url, title, created_at FROM bookmarks
             WHERE url LIKE ?1 ESCAPE '\\' OR title LIKE ?1 ESCAPE '\\'
             ORDER BY created_at DESC",
        )?;
        let rows = stmt
            .query_map(rusqlite::params![pattern], |row| {
                Ok(BookmarkRow {
                    id: row.get(0)?,
                    url: row.get(1)?,
                    title: row.get(2)?,
                    created_at: row.get(3)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn list_bookmarks(&self) -> Result<Vec<BookmarkRow>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT id, url, title, created_at FROM bookmarks ORDER BY created_at DESC")?;
        let rows = stmt
            .query_map([], |row| {
                Ok(BookmarkRow {
                    id: row.get(0)?,
                    url: row.get(1)?,
                    title: row.get(2)?,
                    created_at: row.get(3)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    // --- Browser Session ---

    /// Save the current tab list for session restore.
    pub fn save_session(&self, tabs: &[(String, String, bool)]) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM browser_session", [])?;
        let mut stmt = conn.prepare(
            "INSERT INTO browser_session (tab_index, url, title, is_active) VALUES (?1, ?2, ?3, ?4)",
        )?;
        for (i, (url, title, active)) in tabs.iter().enumerate() {
            stmt.execute(rusqlite::params![i as i64, url, title, *active as i64])?;
        }
        Ok(())
    }

    /// Load the saved session. Returns (url, title, is_active) tuples.
    pub fn load_session(&self) -> Result<Vec<(String, String, bool)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT url, title, is_active FROM browser_session ORDER BY tab_index ASC")?;
        let rows = stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)? != 0,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Clear saved session data.
    pub fn clear_session(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM browser_session", [])?;
        Ok(())
    }

    /// Returns the bookmark id for the given URL if it exists, or None.
    pub fn find_bookmark_by_url(&self, url: &str) -> Result<Option<i64>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id FROM bookmarks WHERE url = ?1")?;
        let mut rows = stmt.query_map(rusqlite::params![url], |row| row.get(0))?;
        match rows.next() {
            Some(Ok(id)) => Ok(Some(id)),
            _ => Ok(None),
        }
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
        // Overwrite
        db.set_setting("theme", "light").unwrap();
        assert_eq!(db.get_setting("theme").unwrap(), Some("light".to_string()));
    }

    #[test]
    fn test_clipboard_delete() {
        let db = RoverDb::open_in_memory().unwrap();
        db.add_clipboard_entry("keep").unwrap();
        db.add_clipboard_entry("delete_me").unwrap();
        let entries = db.list_clipboard_history().unwrap();
        let delete_id = entries
            .iter()
            .find(|e| e.content == "delete_me")
            .unwrap()
            .id;
        db.delete_clipboard_entry(delete_id).unwrap();
        let entries = db.list_clipboard_history().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].content, "keep");
    }

    #[test]
    fn test_clipboard_clear() {
        let db = RoverDb::open_in_memory().unwrap();
        db.add_clipboard_entry("a").unwrap();
        db.add_clipboard_entry("b").unwrap();
        db.clear_clipboard_history().unwrap();
        assert!(db.list_clipboard_history().unwrap().is_empty());
    }

    #[test]
    fn test_empty_entries_ignored() {
        let db = RoverDb::open_in_memory().unwrap();
        db.add_query("").unwrap();
        db.add_query("   ").unwrap();
        assert!(db.list_query_history().unwrap().is_empty());

        db.add_clipboard_entry("").unwrap();
        db.add_clipboard_entry("  ").unwrap();
        assert!(db.list_clipboard_history().unwrap().is_empty());
    }

    #[test]
    fn test_browser_history() {
        let db = RoverDb::open_in_memory().unwrap();
        db.add_history("https://example.com", "Example").unwrap();
        db.add_history("https://rust-lang.org", "Rust").unwrap();

        let recent = db.recent_history(10).unwrap();
        assert_eq!(recent.len(), 2);
        // Both inserted in same second — just verify both present
        let urls: Vec<&str> = recent.iter().map(|r| r.url.as_str()).collect();
        assert!(urls.contains(&"https://example.com"));
        assert!(urls.contains(&"https://rust-lang.org"));

        // Revisit increments count and updates title
        db.add_history("https://example.com", "Example Updated")
            .unwrap();
        let recent = db.recent_history(10).unwrap();
        let example = recent
            .iter()
            .find(|r| r.url == "https://example.com")
            .unwrap();
        assert_eq!(example.visit_count, 2);
        assert_eq!(example.title, "Example Updated");
    }

    #[test]
    fn test_browser_history_search() {
        let db = RoverDb::open_in_memory().unwrap();
        db.add_history("https://github.com/rust-lang", "Rust on GitHub")
            .unwrap();
        db.add_history("https://docs.rs", "Docs.rs").unwrap();

        let results = db.search_history("rust", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].url, "https://github.com/rust-lang");

        // Search by URL
        let results = db.search_history("docs.rs", 10).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_browser_history_ignores_blank() {
        let db = RoverDb::open_in_memory().unwrap();
        db.add_history("about:blank", "").unwrap();
        db.add_history("", "Empty").unwrap();
        assert!(db.recent_history(10).unwrap().is_empty());
    }

    #[test]
    fn test_clear_history() {
        let db = RoverDb::open_in_memory().unwrap();
        db.add_history("https://example.com", "Ex").unwrap();
        db.clear_history().unwrap();
        assert!(db.recent_history(10).unwrap().is_empty());
    }

    #[test]
    fn test_bookmarks() {
        let db = RoverDb::open_in_memory().unwrap();
        db.add_bookmark("https://example.com", "Example").unwrap();
        db.add_bookmark("https://rust-lang.org", "Rust").unwrap();

        let all = db.list_bookmarks().unwrap();
        assert_eq!(all.len(), 2);

        // Find by URL
        let id = db.find_bookmark_by_url("https://example.com").unwrap();
        assert!(id.is_some());
        let id = db.find_bookmark_by_url("https://nope.com").unwrap();
        assert!(id.is_none());
    }

    #[test]
    fn test_bookmark_search() {
        let db = RoverDb::open_in_memory().unwrap();
        db.add_bookmark("https://github.com", "GitHub").unwrap();
        db.add_bookmark("https://docs.rs", "Docs").unwrap();

        let results = db.search_bookmarks("git").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "GitHub");
    }

    #[test]
    fn test_bookmark_remove() {
        let db = RoverDb::open_in_memory().unwrap();
        db.add_bookmark("https://example.com", "Ex").unwrap();
        let all = db.list_bookmarks().unwrap();
        assert_eq!(all.len(), 1);

        db.remove_bookmark(all[0].id).unwrap();
        assert!(db.list_bookmarks().unwrap().is_empty());
    }

    #[test]
    fn test_bookmark_empty_ignored() {
        let db = RoverDb::open_in_memory().unwrap();
        db.add_bookmark("", "Empty").unwrap();
        db.add_bookmark("  ", "Spaces").unwrap();
        assert!(db.list_bookmarks().unwrap().is_empty());
    }

    #[test]
    fn test_search_escapes_like_wildcards() {
        let db = RoverDb::open_in_memory().unwrap();
        db.add_history("https://example.com/100%_done", "100% Done")
            .unwrap();
        db.add_history("https://other.com/safe", "Safe Page")
            .unwrap();

        // Searching for "%" should only match the URL/title containing a literal %
        let results = db.search_history("%", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].url, "https://example.com/100%_done");

        // Searching for "_" should only match the URL/title containing a literal _
        let results = db.search_history("_", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].url, "https://example.com/100%_done");
    }

    #[test]
    fn test_bookmark_search_escapes_like_wildcards() {
        let db = RoverDb::open_in_memory().unwrap();
        db.add_bookmark("https://example.com/100%", "100% Complete")
            .unwrap();
        db.add_bookmark("https://other.com/safe", "Safe").unwrap();

        let results = db.search_bookmarks("%").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].url, "https://example.com/100%");
    }

    #[test]
    fn test_command_usage_cap() {
        let db = RoverDb::open_in_memory().unwrap();
        for i in 0..1005 {
            db.record_command_usage(&format!("cmd_{i}")).unwrap();
        }
        let conn = db.conn.lock().unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM command_usage", [], |r| r.get(0))
            .unwrap();
        assert!(count <= 1000);
    }

    #[test]
    fn test_browser_history_cap() {
        let db = RoverDb::open_in_memory().unwrap();
        for i in 0..5005 {
            db.add_history(&format!("https://example.com/{i}"), "title")
                .unwrap();
        }
        let conn = db.conn.lock().unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM browser_history", [], |r| r.get(0))
            .unwrap();
        assert!(count <= 5000);
    }
}
