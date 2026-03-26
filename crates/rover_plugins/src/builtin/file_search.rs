//! File Search plugin — Spotlight-backed file finder.
//!
//! Match mode: `FuzzyCommand` — provides a "File Search" command that opens a
//! submenu with live results from mdfind (falls back to read_dir).
//!
//! Search is performed asynchronously: `submenu_items` returns stale/empty
//! results immediately and sets `loading = true`; the delegate detects this
//! and spawns a background task that calls `set_results` when done.

use rover_commands::file_search;
use rover_plugin_api::*;
use std::any::Any;

pub struct FileSearchPlugin {
    results: Vec<file_search::FileResult>,
    /// The query that produced `results` (may lag the current query while loading).
    pending_query: String,
    /// True while a background search is in flight.
    loading: bool,
}

impl FileSearchPlugin {
    pub fn new() -> Self {
        Self {
            results: Vec::new(),
            pending_query: String::new(),
            loading: false,
        }
    }

    pub fn is_loading(&self) -> bool {
        self.loading
    }

    pub fn pending_query(&self) -> &str {
        &self.pending_query
    }

    /// Called by the delegate's background task when results arrive.
    /// Silently discards results that arrived for a stale query.
    pub fn set_results(&mut self, query: &str, results: Vec<file_search::FileResult>) {
        if self.pending_query == query {
            self.results = results;
            self.loading = false;
        }
    }
}

impl Default for FileSearchPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for FileSearchPlugin {
    fn id(&self) -> &str { "builtin.file_search" }
    fn name(&self) -> &str { "File Search" }
    fn description(&self) -> &str { "Search for files on your system" }

    fn on_load(&mut self, _ctx: &mut PluginContext) -> Result<(), PluginError> {
        Ok(())
    }

    fn match_mode(&self) -> MatchMode {
        MatchMode::FuzzyCommand
    }

    fn priority(&self) -> i32 { 5 }

    fn provides_submenu(&self) -> bool { true }

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
            self.pending_query.clear();
            self.loading = false;
            ConfirmResult::EnterSubmenu {
                title: "File Search".to_string(),
            }
        } else {
            ConfirmResult::Stay
        }
    }

    fn submenu_items(&mut self, query: &str, _ctx: &PluginContext) -> Vec<MatchResult> {
        // If the query changed, mark as loading; the delegate will spawn the search.
        if query != self.pending_query {
            self.pending_query = query.to_string();
            self.loading = true;
        }

        // Show a placeholder when there are no cached results yet.
        if self.results.is_empty() {
            let title = if query.is_empty() {
                "Type to search files...".to_string()
            } else {
                "Searching...".to_string()
            };
            return vec![MatchResult {
                id: "file_search_loading".to_string(),
                title,
                subtitle: None,
                icon: Some(IconHint::Emoji("magnifying_glass".to_string())),
                score: 1.0,
                plugin_id: self.id().to_string(),
                render_hint: None,
            }];
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
                render_hint: None,
            })
            .collect()
    }

    fn on_submenu_confirm(
        &mut self,
        match_id: &str,
        _query: &str,
        _ctx: &mut PluginContext,
    ) -> ConfirmResult {
        if match_id == "file_search_loading" {
            return ConfirmResult::Stay;
        }
        if let Some(idx_str) = match_id.strip_prefix("file:") {
            if let Ok(idx) = idx_str.parse::<usize>() {
                if let Some(file) = self.results.get(idx) {
                    let _ = std::process::Command::new("open")
                        .arg(&file.path)
                        .spawn();
                    return ConfirmResult::Dismiss;
                }
            }
        }
        ConfirmResult::Stay
    }

    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
}
