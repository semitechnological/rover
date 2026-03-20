use rover_db::RoverDb;
use std::sync::Arc;

pub struct HistoryEntry {
    pub url: String,
    pub title: String,
    pub visited_at: i64,
}

pub struct HistoryManager {
    db: Arc<RoverDb>,
}

impl HistoryManager {
    pub fn new(db: Arc<RoverDb>) -> Self {
        Self { db }
    }

    /// Upsert a URL into history, incrementing visit_count on conflict.
    pub fn record(&self, url: &str, title: &str) {
        if let Err(e) = self.db.add_history(url, title) {
            log::warn!("history: failed to record {url}: {e}");
        }
    }

    /// Search history by a LIKE pattern against url and title.
    pub fn search(&self, query: &str, limit: usize) -> Vec<HistoryEntry> {
        self.db
            .search_history(query, limit)
            .unwrap_or_default()
            .into_iter()
            .map(|r| HistoryEntry {
                url: r.url,
                title: r.title,
                visited_at: r.visited_at,
            })
            .collect()
    }

    /// Return the most recently visited entries.
    pub fn recent(&self, limit: usize) -> Vec<HistoryEntry> {
        self.db
            .recent_history(limit)
            .unwrap_or_default()
            .into_iter()
            .map(|r| HistoryEntry {
                url: r.url,
                title: r.title,
                visited_at: r.visited_at,
            })
            .collect()
    }

    /// Delete all history.
    pub fn clear(&self) {
        if let Err(e) = self.db.clear_history() {
            log::warn!("history: failed to clear: {e}");
        }
    }
}
