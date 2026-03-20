//! Semantic search for Rover using fastembed embeddings and SQLite storage.
//!
//! `SemanticIndex` persists documents in SQLite (one row = one chunk of text +
//! its embedding as a raw f32 blob). An in-memory cache of all embeddings is
//! kept in sync so that searches are O(n) cosine similarity without hitting
//! disk on every query.
//!
//! # Initialisation cost
//! `SemanticIndex::new()` is **blocking** and may take several seconds on first
//! run while the embedding model is downloaded from HuggingFace (~33 MB for
//! `AllMiniLML6V2`). Call it from a background thread / task so the UI stays
//! responsive.
//!
//! # Collections
//! Documents are grouped into named collections: `"tabs"`, `"commands"`,
//! `"files"`, `"pages"` etc.  `search()` can filter to a single collection or
//! search all of them.

use anyhow::{Context as _, Result};
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use rusqlite::{params, Connection};
use std::path::Path;

// ─── Types ────────────────────────────────────────────────────────────────────

/// A ranked search hit returned by [`SemanticIndex::search`].
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub id: String,
    pub collection: String,
    pub text: String,
    pub score: f32,
    pub metadata: serde_json::Value,
}

// Internal: in-memory embedding cache row.
struct CachedDoc {
    id: String,
    embedding: Vec<f32>,
}

// ─── SemanticIndex ────────────────────────────────────────────────────────────

pub struct SemanticIndex {
    conn: Connection,
    model: TextEmbedding,
    cache: Vec<CachedDoc>,
}

impl SemanticIndex {
    /// Opens (or creates) the SQLite database at `db_path` and loads the
    /// embedding model.  Downloads the model on first run.
    pub fn new(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)
            .with_context(|| format!("open semantic DB at {}", db_path.display()))?;

        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             CREATE TABLE IF NOT EXISTS documents (
                 id          TEXT    PRIMARY KEY,
                 collection  TEXT    NOT NULL DEFAULT 'default',
                 text        TEXT    NOT NULL,
                 embedding   BLOB    NOT NULL,
                 metadata    TEXT    NOT NULL DEFAULT '{}',
                 indexed_at  INTEGER NOT NULL DEFAULT (unixepoch())
             );
             CREATE INDEX IF NOT EXISTS documents_collection
                 ON documents (collection);",
        )?;

        log::info!("rover_semantic: loading embedding model…");
        let model = TextEmbedding::try_new(
            InitOptions::new(EmbeddingModel::AllMiniLML6V2)
                .with_show_download_progress(true),
        )
        .context("load embedding model")?;
        log::info!("rover_semantic: model ready");

        // Warm the in-memory embedding cache from the DB.
        let cache = Self::load_cache(&conn)?;
        log::info!("rover_semantic: loaded {} cached embeddings", cache.len());

        Ok(Self { conn, model, cache })
    }

    // ── CRUD ──────────────────────────────────────────────────────────────────

    /// Embeds `text` and stores it under `id` in `collection`.
    /// Replaces any existing document with the same `id`.
    pub fn upsert(
        &mut self,
        id: &str,
        collection: &str,
        text: &str,
        metadata: serde_json::Value,
    ) -> Result<()> {
        let embedding = self.embed_one(text)?;
        let blob = serialize(&embedding);
        let meta_str = metadata.to_string();

        self.conn.execute(
            "INSERT INTO documents (id, collection, text, embedding, metadata, indexed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, unixepoch())
             ON CONFLICT(id) DO UPDATE SET
                 collection = excluded.collection,
                 text       = excluded.text,
                 embedding  = excluded.embedding,
                 metadata   = excluded.metadata,
                 indexed_at = unixepoch()",
            params![id, collection, text, blob, meta_str],
        )?;

        // Update in-memory cache
        if let Some(entry) = self.cache.iter_mut().find(|e| e.id == id) {
            entry.embedding = embedding;
        } else {
            self.cache.push(CachedDoc { id: id.to_string(), embedding });
        }

        Ok(())
    }

    /// Removes a document by id. No-op if not found.
    pub fn remove(&mut self, id: &str) -> Result<()> {
        self.conn.execute("DELETE FROM documents WHERE id = ?1", params![id])?;
        self.cache.retain(|e| e.id != id);
        Ok(())
    }

    /// Removes all documents in a collection.
    pub fn clear_collection(&mut self, collection: &str) -> Result<()> {
        // Collect ids first so we can clean the cache.
        let ids: Vec<String> = {
            let mut stmt = self.conn.prepare(
                "SELECT id FROM documents WHERE collection = ?1",
            )?;
            let ids = stmt.query_map(params![collection], |row| row.get(0))?
                .collect::<rusqlite::Result<_>>()?;
            ids
        };
        self.conn.execute(
            "DELETE FROM documents WHERE collection = ?1",
            params![collection],
        )?;
        self.cache.retain(|e| !ids.contains(&e.id));
        Ok(())
    }

    // ── Search ────────────────────────────────────────────────────────────────

    /// Returns the `top_k` most semantically similar documents to `query`.
    /// Pass `collection = Some("tabs")` to restrict the search.
    pub fn search(
        &self,
        query: &str,
        collection: Option<&str>,
        top_k: usize,
    ) -> Result<Vec<SearchResult>> {
        if self.cache.is_empty() || top_k == 0 {
            return Ok(vec![]);
        }

        let query_emb = self.embed_one(query)?;

        // Score all cached docs; optionally filter by collection.
        // We fetch collection/text/metadata from SQLite only for the top hits,
        // keeping the hot path allocation-light.
        let mut scored: Vec<(&CachedDoc, f32)> = self
            .cache
            .iter()
            .map(|doc| {
                let score = cosine_similarity(&query_emb, &doc.embedding);
                (doc, score)
            })
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(top_k * 4); // over-fetch to account for collection filter

        let mut results = Vec::with_capacity(top_k);
        for (doc, score) in scored {
            if results.len() >= top_k {
                break;
            }
            let row: Option<(String, String, String)> = {
                let sql = if collection.is_some() {
                    "SELECT collection, text, metadata FROM documents WHERE id = ?1 AND collection = ?2"
                } else {
                    "SELECT collection, text, metadata FROM documents WHERE id = ?1"
                };
                let mut stmt = self.conn.prepare_cached(sql)?;
                if let Some(col) = collection {
                    stmt.query_row(params![doc.id, col], |r| {
                        Ok((r.get(0)?, r.get(1)?, r.get(2)?))
                    }).ok()
                } else {
                    stmt.query_row(params![doc.id], |r| {
                        Ok((r.get(0)?, r.get(1)?, r.get(2)?))
                    }).ok()
                }
            };
            if let Some((col, text, meta_str)) = row {
                let metadata = serde_json::from_str(&meta_str).unwrap_or(serde_json::Value::Null);
                results.push(SearchResult {
                    id: doc.id.clone(),
                    collection: col,
                    text,
                    score,
                    metadata,
                });
            }
        }

        Ok(results)
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    pub fn len(&self) -> usize {
        self.cache.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// Embeds a batch of texts. Useful for bulk indexing (e.g. crawling a site).
    pub fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        self.model
            .embed(texts.to_vec(), None)
            .context("embed batch")
    }

    // private

    fn embed_one(&self, text: &str) -> Result<Vec<f32>> {
        let mut batch = self.model.embed(vec![text], None).context("embed")?;
        batch.pop().context("empty embedding result")
    }

    fn load_cache(conn: &Connection) -> Result<Vec<CachedDoc>> {
        let mut stmt = conn.prepare("SELECT id, embedding FROM documents")?;
        let rows = stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let blob: Vec<u8> = row.get(1)?;
            Ok((id, blob))
        })?;

        let mut cache = Vec::new();
        for row in rows {
            let (id, blob) = row?;
            cache.push(CachedDoc { id, embedding: deserialize(&blob) });
        }
        Ok(cache)
    }
}

// ─── Math ─────────────────────────────────────────────────────────────────────

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 { 0.0 } else { dot / (norm_a * norm_b) }
}

fn serialize(emb: &[f32]) -> Vec<u8> {
    emb.iter().flat_map(|f| f.to_le_bytes()).collect()
}

fn deserialize(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_roundtrip() {
        let original = vec![1.0f32, -0.5, 0.0, 42.0];
        let bytes = serialize(&original);
        let recovered = deserialize(&bytes);
        assert_eq!(original, recovered);
    }

    #[test]
    fn test_cosine_similarity_identical() {
        let v = vec![1.0f32, 0.0, 0.0];
        assert!((cosine_similarity(&v, &v) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0f32, 0.0];
        let b = vec![0.0f32, 1.0];
        assert!((cosine_similarity(&a, &b)).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let a = vec![1.0f32, 0.0];
        let b = vec![-1.0f32, 0.0];
        assert!((cosine_similarity(&a, &b) + 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_mismatched_len() {
        let a = vec![1.0f32];
        let b = vec![1.0f32, 0.0];
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }
}
