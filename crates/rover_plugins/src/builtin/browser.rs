//! Browser plugin — provides the "Open Browser" command, browser tab context
//! items (pills), tab filtering, and tab management actions.

use rover_plugin_api::*;
use std::any::Any;

/// Info about a browser tab (mirror of rover_browser types without the dep).
#[derive(Debug, Clone)]
pub struct TabInfo {
    pub title: String,
    pub url: String,
    pub is_active: bool,
    /// Milliseconds since this tab was last active (for recency sort).
    pub last_active_ms_ago: u64,
}

pub struct BrowserPlugin {
    tabs: Vec<TabInfo>,
    /// Indices of tabs matching the current query, sorted by relevance.
    filtered: Vec<usize>,
    /// Whether the browser was focused before Rover activated.
    browser_was_focused: bool,
}

impl BrowserPlugin {
    pub fn new() -> Self {
        Self {
            tabs: Vec::new(),
            filtered: Vec::new(),
            browser_was_focused: false,
        }
    }

    /// Update the tab list from the live browser state.
    pub fn set_tabs(&mut self, tabs: Vec<TabInfo>) {
        self.filtered = (0..tabs.len()).collect();
        self.tabs = tabs;
    }

    /// Set whether the browser was focused before Rover activated.
    pub fn set_browser_was_focused(&mut self, focused: bool) {
        self.browser_was_focused = focused;
    }

    fn filter_tabs(&mut self, query: &str) {
        if query.is_empty() || self.tabs.is_empty() {
            self.filtered = (0..self.tabs.len()).collect();
        } else {
            let q = query.to_lowercase();
            self.filtered = self
                .tabs
                .iter()
                .enumerate()
                .filter(|(_, tab)| {
                    tab.title.to_lowercase().contains(&q)
                        || tab.url.to_lowercase().contains(&q)
                })
                .map(|(i, _)| i)
                .collect();
        }

        // Sort: active tab first, then by recency
        let active_idx = self.tabs.iter().position(|t| t.is_active);
        self.filtered.sort_by(|&a, &b| {
            let a_active = Some(a) == active_idx;
            let b_active = Some(b) == active_idx;
            b_active
                .cmp(&a_active)
                .then(self.tabs[a].last_active_ms_ago.cmp(&self.tabs[b].last_active_ms_ago))
        });
    }
}

impl Default for BrowserPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for BrowserPlugin {
    fn id(&self) -> &str { "builtin.browser" }
    fn name(&self) -> &str { "Browser" }
    fn description(&self) -> &str { "Keyboard-driven WebKit browser" }

    fn on_load(&mut self, _ctx: &mut PluginContext) -> Result<(), PluginError> {
        Ok(())
    }

    fn match_mode(&self) -> MatchMode {
        MatchMode::FuzzyCommand
    }

    fn priority(&self) -> i32 { 5 }

    fn commands(&self) -> Vec<CommandDescriptor> {
        vec![CommandDescriptor {
            id: "open_browser".to_string(),
            name: "Open Browser".to_string(),
            description: "Open the keyboard-driven browser".to_string(),
            icon: Some(IconHint::Emoji("globe".to_string())),
            keywords: vec!["web".to_string(), "safari".to_string(), "browse".to_string()],
            has_submenu: false,
        }]
    }

    fn on_query(&mut self, query: &str, _ctx: &PluginContext) -> Vec<MatchResult> {
        // Only show tab results when browser was recently focused
        if !self.browser_was_focused || self.tabs.is_empty() {
            return vec![];
        }

        self.filter_tabs(query);

        self.filtered
            .iter()
            .filter_map(|&i| {
                let tab = self.tabs.get(i)?;
                let label = if tab.title.is_empty() {
                    tab.url.clone()
                } else {
                    tab.title.clone()
                };
                Some(MatchResult {
                    id: format!("tab:{}", i),
                    title: label,
                    subtitle: Some(tab.url.clone()),
                    icon: Some(IconHint::Emoji("globe".to_string())),
                    score: if tab.is_active { 1.0 } else { 0.8 - (i as f64 * 0.01) },
                    plugin_id: self.id().to_string(),
                    render_hint: Some(RenderHint::StatePill {
                        label: if tab.is_active { "ACTIVE".to_string() } else { "TAB".to_string() },
                        color: if tab.is_active { "green".to_string() } else { "gray".to_string() },
                    }),
                })
            })
            .collect()
    }

    fn on_confirm(
        &mut self,
        match_id: &str,
        _query: &str,
        _secondary: bool,
        _ctx: &mut PluginContext,
    ) -> ConfirmResult {
        if match_id == "open_browser" {
            return ConfirmResult::OpenWindow(WindowRequest::Browser { url: None });
        }
        if let Some(idx_str) = match_id.strip_prefix("tab:") {
            if let Ok(idx) = idx_str.parse::<usize>() {
                if let Some(tab) = self.tabs.get(idx) {
                    return ConfirmResult::OpenUrl(tab.url.clone());
                }
            }
        }
        ConfirmResult::Stay
    }

    fn context_items(&self, _ctx: &PluginContext) -> Vec<ContextItem> {
        self.tabs
            .iter()
            .enumerate()
            .map(|(i, tab)| ContextItem {
                id: format!("tab:{}", i),
                label: if tab.title.is_empty() {
                    tab.url.clone()
                } else {
                    tab.title.clone()
                },
                icon: Some(IconHint::Emoji("globe".to_string())),
                is_active: tab.is_active,
            })
            .collect()
    }

    fn actions_for_match(&self, match_id: &str) -> Vec<ActionDescriptor> {
        if match_id.starts_with("tab:") {
            vec![
                ActionDescriptor {
                    id: "switch".to_string(),
                    name: "Switch to tab".to_string(),
                    shortcut: Some("Enter".to_string()),
                },
                ActionDescriptor {
                    id: "close".to_string(),
                    name: "Close tab".to_string(),
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
        ctx: &mut PluginContext,
    ) -> ConfirmResult {
        match action_id {
            "switch" => self.on_confirm(match_id, "", false, ctx),
            "close" => {
                if let Some(idx_str) = match_id.strip_prefix("tab:") {
                    if let Ok(idx) = idx_str.parse::<usize>() {
                        if idx < self.tabs.len() {
                            self.tabs.remove(idx);
                            self.filtered = (0..self.tabs.len()).collect();
                        }
                    }
                }
                ConfirmResult::Stay
            }
            _ => ConfirmResult::Stay,
        }
    }

    fn open_window(&mut self, _ctx: &mut PluginContext) -> Option<WindowRequest> {
        Some(WindowRequest::Browser { url: None })
    }

    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
}
