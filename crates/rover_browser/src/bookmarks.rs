use anyhow::Result;
use rover_db::RoverDb;
use std::sync::Arc;

pub struct Bookmark {
    pub id: i64,
    pub url: String,
    pub title: String,
    pub created_at: i64,
}

pub struct BookmarkManager {
    db: Arc<RoverDb>,
}

impl BookmarkManager {
    pub fn new(db: Arc<RoverDb>) -> Self {
        Self { db }
    }

    pub fn add(&self, url: &str, title: &str) -> Result<()> {
        self.db.add_bookmark(url, title)
    }

    pub fn remove(&self, id: i64) -> Result<()> {
        self.db.remove_bookmark(id)
    }

    pub fn search(&self, query: &str) -> Vec<Bookmark> {
        self.db
            .search_bookmarks(query)
            .unwrap_or_default()
            .into_iter()
            .map(|r| Bookmark {
                id: r.id,
                url: r.url,
                title: r.title,
                created_at: r.created_at,
            })
            .collect()
    }

    pub fn list(&self) -> Vec<Bookmark> {
        self.db
            .list_bookmarks()
            .unwrap_or_default()
            .into_iter()
            .map(|r| Bookmark {
                id: r.id,
                url: r.url,
                title: r.title,
                created_at: r.created_at,
            })
            .collect()
    }

    /// Returns the bookmark id for the current URL if it is bookmarked.
    pub fn find_by_url(&self, url: &str) -> Option<i64> {
        self.db.find_bookmark_by_url(url).unwrap_or(None)
    }
}
