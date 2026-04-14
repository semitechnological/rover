//! File Search plugin — spotlight-style file finder.
//!
//! Match mode: `FuzzyCommand` — provides a "File Search" command that opens a
//! submenu.

use rover_commands::file_search;
use rover_plugin_api::*;
use rover_semantic::SemanticIndex;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

pub struct FileSearchPlugin {
    results: Vec<file_search::FileResult>,
}

impl FileSearchPlugin {
    pub fn new() -> Self {
        Self {
            results: Vec::new(),
        }
    }

    fn semantic_index() -> Option<&'static Mutex<Option<SemanticIndex>>> {
        static INDEX: OnceLock<Mutex<Option<SemanticIndex>>> = OnceLock::new();
        let idx = INDEX.get_or_init(|| {
            let db_path = dirs::config_dir()
                .unwrap_or_else(|| PathBuf::from("/tmp"))
                .join("rover")
                .join("semantic.db");
            let index = SemanticIndex::new(&db_path).ok();
            Mutex::new(index)
        });
        Some(idx)
    }

    fn index_files_once() {
        static STARTED: AtomicBool = AtomicBool::new(false);
        if STARTED
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
            .is_err()
        {
            return;
        }
        std::thread::spawn(|| {
            let Some(lock) = Self::semantic_index() else {
                return;
            };
            let mut guard = match lock.lock() {
                Ok(g) => g,
                Err(_) => return,
            };
            let Some(index) = guard.as_mut() else {
                return;
            };
            let files = file_search::build_local_index(3000);

            // Build batch of documents for efficient embedding
            let batch: Vec<(String, String, String, serde_json::Value)> = files
                .iter()
                .map(|file| {
                    let id = format!("file:{}", file.path.display());
                    let text = format!("{} {}", file.name, file.path.display());
                    let metadata = serde_json::json!({
                        "name": file.name,
                        "path": file.path.display().to_string()
                    });
                    (id, "files".to_string(), text, metadata)
                })
                .collect();

            // Use batch upsert for faster indexing
            if let Err(e) = index.upsert_batch(&batch) {
                log::error!("Failed to batch index files: {e}");
                // Fallback to individual upserts
                for (id, _, text, metadata) in batch {
                    let _ = index.upsert(&id, "files", &text, metadata);
                }
            }
        });
    }

    fn semantic_search(query: &str, max: usize) -> Vec<file_search::FileResult> {
        if query.trim().len() < 2 {
            return Vec::new();
        }
        let Some(lock) = Self::semantic_index() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let guard = match lock.lock() {
            Ok(g) => g,
            Err(_) => return out,
        };
        let Some(index) = guard.as_ref() else {
            return out;
        };
        let hits = match index.search(query, Some("files"), max) {
            Ok(v) => v,
            Err(_) => return out,
        };
        for hit in hits {
            let path = hit
                .metadata
                .get("path")
                .and_then(|v| v.as_str())
                .map(PathBuf::from);
            let name = hit
                .metadata
                .get("name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            if let (Some(path), Some(name)) = (path, name) {
                out.push(file_search::FileResult { name, path });
            }
        }
        out
    }
}

impl Default for FileSearchPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for FileSearchPlugin {
    fn id(&self) -> &str {
        "builtin.file_search"
    }
    fn name(&self) -> &str {
        "File Search"
    }
    fn description(&self) -> &str {
        "Search for files on your system"
    }

    fn on_load(&mut self, _ctx: &mut PluginContext) -> Result<(), PluginError> {
        Self::index_files_once();
        Ok(())
    }

    fn match_mode(&self) -> MatchMode {
        MatchMode::FuzzyCommand
    }

    fn priority(&self) -> i32 {
        5
    }

    fn provides_submenu(&self) -> bool {
        true
    }

    fn commands(&self) -> Vec<CommandDescriptor> {
        vec![CommandDescriptor {
            id: "file_search".to_string(),
            name: "File Search".to_string(),
            description: "Search for files on your system".to_string(),
            icon: Some(IconHint::Emoji("magnifying_glass".to_string())),
            keywords: vec!["find".to_string(), "file".to_string(), "search".to_string()],
            has_submenu: true,
        }]
    }

    fn on_query(&mut self, _query: &str, _ctx: &PluginContext) -> Vec<MatchResult> {
        vec![]
    }

    fn on_confirm(
        &mut self,
        match_id: &str,
        _query: &str,
        _secondary: bool,
        _ctx: &mut PluginContext,
    ) -> ConfirmResult {
        if match_id == "file_search" {
            self.results.clear();
            ConfirmResult::EnterSubmenu {
                title: "File Search".to_string(),
            }
        } else {
            ConfirmResult::Stay
        }
    }

    fn submenu_items(&mut self, query: &str, _ctx: &PluginContext) -> Vec<MatchResult> {
        self.results = Self::semantic_search(query, 20);
        if self.results.is_empty() {
            self.results = file_search::search_files(query, 20);
        }
        self.results
            .iter()
            .enumerate()
            .map(|(i, file)| MatchResult {
                id: format!("file:{}", i),
                title: file.name.clone(),
                subtitle: Some(file.path.display().to_string()),
                icon: None,
                score: 1.0 - (i as f64 * 0.01),
                plugin_id: self.id().to_string(),
            })
            .collect()
    }

    fn on_submenu_confirm(
        &mut self,
        match_id: &str,
        _query: &str,
        _ctx: &mut PluginContext,
    ) -> ConfirmResult {
        if let Some(idx_str) = match_id.strip_prefix("file:") {
            if let Ok(idx) = idx_str.parse::<usize>() {
                if let Some(file) = self.results.get(idx) {
                    let _ = std::process::Command::new("open").arg(&file.path).spawn();
                    return ConfirmResult::Dismiss;
                }
            }
        }
        ConfirmResult::Stay
    }
}
