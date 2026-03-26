//! Clipboard History plugin — stores and searches past clipboard entries.
//!
//! Match mode: `FuzzyCommand` — provides a "Clipboard History" command that
//! opens a submenu. The submenu lists all clipboard entries, filterable by
//! query.

use rover_db::RoverDb;
use rover_plugin_api::*;
use std::sync::Arc;

pub struct ClipboardPlugin {
    db: Option<Arc<RoverDb>>,
    /// Cached entries, loaded when the submenu opens.
    entries: Vec<rover_db::ClipboardEntry>,
    /// Filtered indices into `entries`.
    filtered: Vec<usize>,
}

impl ClipboardPlugin {
    pub fn new(db: Arc<RoverDb>) -> Self {
        Self {
            db: Some(db),
            entries: Vec::new(),
            filtered: Vec::new(),
        }
    }

    fn load_entries(&mut self) {
        if let Some(db) = &self.db {
            self.entries = db.list_clipboard_history().unwrap_or_default();
            self.filtered = (0..self.entries.len()).collect();
        }
    }

    fn filter_entries(&mut self, query: &str) {
        if query.is_empty() {
            self.filtered = (0..self.entries.len()).collect();
            return;
        }
        let q = query.to_lowercase();
        self.filtered = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.content.to_lowercase().contains(&q))
            .map(|(i, _)| i)
            .collect();
    }
}

impl Plugin for ClipboardPlugin {
    fn id(&self) -> &str { "builtin.clipboard" }
    fn name(&self) -> &str { "Clipboard History" }
    fn description(&self) -> &str { "View and paste from clipboard history" }

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
            id: "clipboard_history".to_string(),
            name: "Clipboard History".to_string(),
            description: "View and paste from clipboard history".to_string(),
            icon: Some(IconHint::Emoji("clipboard".to_string())),
            keywords: vec!["paste".to_string(), "copy".to_string(), "clipboard".to_string()],
            has_submenu: true,
        }]
    }

    fn on_query(&mut self, _query: &str, _ctx: &PluginContext) -> Vec<MatchResult> {
        // FuzzyCommand mode: the registry handles fuzzy matching against our
        // commands(). We don't produce results here directly — only via submenu.
        vec![]
    }

    fn on_confirm(
        &mut self,
        match_id: &str,
        _query: &str,
        _secondary: bool,
        _ctx: &mut PluginContext,
    ) -> ConfirmResult {
        if match_id == "clipboard_history" {
            // Load entries and enter submenu
            self.load_entries();
            ConfirmResult::EnterSubmenu {
                title: "Clipboard History".to_string(),
            }
        } else {
            ConfirmResult::Stay
        }
    }

    fn submenu_items(&mut self, query: &str, _ctx: &PluginContext) -> Vec<MatchResult> {
        // Ensure entries are loaded
        if self.entries.is_empty() {
            self.load_entries();
        }
        self.filter_entries(query);

        self.filtered
            .iter()
            .filter_map(|&i| {
                let entry = self.entries.get(i)?;
                let preview: String = entry
                    .content
                    .chars()
                    .take(80)
                    .collect::<String>()
                    .replace('\n', " ");
                Some(MatchResult {
                    id: format!("clip:{}", i),
                    title: preview,
                    subtitle: Some(format!("id:{}", entry.id)),
                    icon: None,
                    score: 1.0 - (i as f64 * 0.001), // Most recent first
                    plugin_id: self.id().to_string(),
                })
            })
            .collect()
    }

    fn on_submenu_confirm(
        &mut self,
        match_id: &str,
        _query: &str,
        _ctx: &mut PluginContext,
    ) -> ConfirmResult {
        if let Some(idx_str) = match_id.strip_prefix("clip:") {
            if let Ok(idx) = idx_str.parse::<usize>() {
                if let Some(entry) = self.entries.get(idx) {
                    return ConfirmResult::CopyToClipboard(entry.content.clone());
                }
            }
        }
        ConfirmResult::Stay
    }

    fn actions_for_match(&self, match_id: &str) -> Vec<ActionDescriptor> {
        if match_id.starts_with("clip:") {
            vec![
                ActionDescriptor {
                    id: "paste".to_string(),
                    name: "Copy to clipboard".to_string(),
                    shortcut: Some("Enter".to_string()),
                },
                ActionDescriptor {
                    id: "delete".to_string(),
                    name: "Delete entry".to_string(),
                    shortcut: None,
                },
            ]
        } else {
            vec![]
        }
    }

    fn execute_action(
        &mut self,
        match_id: &str,
        action_id: &str,
        _ctx: &mut PluginContext,
    ) -> ConfirmResult {
        if let Some(idx_str) = match_id.strip_prefix("clip:") {
            if let Ok(idx) = idx_str.parse::<usize>() {
                match action_id {
                    "paste" => {
                        if let Some(entry) = self.entries.get(idx) {
                            return ConfirmResult::CopyToClipboard(entry.content.clone());
                        }
                    }
                    "delete" => {
                        if let Some(entry) = self.entries.get(idx) {
                            if let Some(db) = &self.db {
                                let _ = db.delete_clipboard_entry(entry.id);
                            }
                            self.load_entries();
                        }
                        return ConfirmResult::Stay;
                    }
                    _ => {}
                }
            }
        }
        ConfirmResult::Stay
    }
}
