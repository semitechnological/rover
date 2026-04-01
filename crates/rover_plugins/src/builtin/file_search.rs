//! File Search plugin — spotlight-style file finder.
//!
//! Match mode: `FuzzyCommand` — provides a "File Search" command that opens a
//! submenu.

use rover_commands::file_search;
use rover_plugin_api::*;

pub struct FileSearchPlugin {
    results: Vec<file_search::FileResult>,
}

impl FileSearchPlugin {
    pub fn new() -> Self {
        Self {
            results: Vec::new(),
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
            ConfirmResult::EnterSubmenu {
                title: "File Search".to_string(),
            }
        } else {
            ConfirmResult::Stay
        }
    }

    fn submenu_items(&mut self, query: &str, _ctx: &PluginContext) -> Vec<MatchResult> {
        self.results = file_search::search_files(query, 20);
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
                    let _ = std::process::Command::new("open")
                        .arg(&file.path)
                        .spawn();
                    return ConfirmResult::Dismiss;
                }
            }
        }
        ConfirmResult::Stay
    }
}
