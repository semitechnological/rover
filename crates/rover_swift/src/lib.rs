//! rover_swift — minimal Rust core, exported to Swift via eqswift/UniFFI.
//!
//! In the SwiftUI branch plugins live natively in Swift. This crate exposes:
//!   1. `FuzzyEngine`   — skim-based fuzzy scoring for Swift plugin results
//!   2. `RoverDatabase` — SQLite clipboard history + key-value settings

use rover_db::{RoverDb, RoverDbOptions};
use rover_fuzzy::{match_strings, normalize_query, StringMatchCandidate};
use std::sync::Arc;

eqswift::setup!();

// ─── FuzzyEngine ─────────────────────────────────────────────────────────────

/// Skim-algorithm fuzzy scorer. Call from Swift to rank candidates.
#[derive(eqswift::Object)]
pub struct FuzzyEngine;

#[eqswift::export]
impl FuzzyEngine {
    #[uniffi::constructor]
    pub fn new() -> Self {
        Self
    }

    /// Score and rank `candidates` against `query`.
    /// Returns items sorted best-first; non-matching candidates are excluded.
    pub fn rank(&self, query: String, candidates: Vec<String>) -> Vec<RankedItem> {
        let normalised = normalize_query(&query);
        let input: Vec<StringMatchCandidate> = candidates
            .iter()
            .enumerate()
            .map(|(i, s)| StringMatchCandidate::new(i, s))
            .collect();

        match_strings(&input, &normalised, candidates.len())
            .into_iter()
            .map(|m| RankedItem {
                candidate: candidates[m.candidate_id].clone(),
                score: m.score,
            })
            .collect()
    }
}

/// One ranked result from `FuzzyEngine::rank`.
#[derive(eqswift::Record, Clone)]
pub struct RankedItem {
    pub candidate: String,
    /// Skim score — higher is better match.
    pub score: i64,
}

// ─── RoverDatabase ────────────────────────────────────────────────────────────

/// SQLite-backed storage: clipboard history and key-value settings.
#[derive(eqswift::Object)]
pub struct RoverDatabase {
    db: Arc<RoverDb>,
}

#[eqswift::export]
impl RoverDatabase {
    /// Open (or create) the Rover database. Allows unencrypted fallback so
    /// Swift code doesn't need Keychain setup during development.
    #[uniffi::constructor]
    pub fn open() -> Result<Arc<Self>, DatabaseError> {
        let opts = RoverDbOptions {
            allow_unencrypted: true,
        };
        let db = RoverDb::open_with_options(opts).map_err(|e| DatabaseError::Open {
            message: e.to_string(),
        })?;
        Ok(Arc::new(Self { db: Arc::new(db) }))
    }

    // ── Clipboard history ────────────────────────────────────────────────

    /// Append a clipboard entry. Deduplication is handled by the DB layer.
    pub fn push_clipboard(&self, text: String) -> Result<(), DatabaseError> {
        self.db
            .add_clipboard_entry(&text)
            .map_err(|e| DatabaseError::Query {
                message: e.to_string(),
            })
    }

    /// Return all clipboard entries, newest first.
    pub fn clipboard_history(&self) -> Result<Vec<ClipboardEntry>, DatabaseError> {
        self.db
            .list_clipboard_history()
            .map_err(|e| DatabaseError::Query {
                message: e.to_string(),
            })
            .map(|rows| {
                rows.into_iter()
                    .map(|e| ClipboardEntry {
                        id: e.id,
                        content: e.content,
                        timestamp: e.timestamp,
                    })
                    .collect()
            })
    }

    /// Delete a clipboard entry by its id.
    pub fn delete_clipboard(&self, id: i64) -> Result<(), DatabaseError> {
        self.db
            .delete_clipboard_entry(id)
            .map_err(|e| DatabaseError::Query {
                message: e.to_string(),
            })
    }

    // ── Key-value settings ───────────────────────────────────────────────

    /// Read a setting. Returns `nil` if not set.
    pub fn get_setting(&self, key: String) -> Result<Option<String>, DatabaseError> {
        self.db
            .get_setting(&key)
            .map_err(|e| DatabaseError::Query {
                message: e.to_string(),
            })
    }

    /// Write a setting.
    pub fn set_setting(&self, key: String, value: String) -> Result<(), DatabaseError> {
        self.db
            .set_setting(&key, &value)
            .map_err(|e| DatabaseError::Query {
                message: e.to_string(),
            })
    }
}

/// A clipboard history entry.
#[derive(eqswift::Record, Clone)]
pub struct ClipboardEntry {
    pub id: i64,
    pub content: String,
    /// Unix timestamp (seconds since epoch).
    pub timestamp: i64,
}

/// Errors from `RoverDatabase`.
#[derive(eqswift::Error, Debug, thiserror::Error)]
pub enum DatabaseError {
    #[error("failed to open database: {message}")]
    Open { message: String },
    #[error("database error: {message}")]
    Query { message: String },
}
