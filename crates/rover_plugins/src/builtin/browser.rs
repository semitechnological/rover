//! Browser plugin — provides the "Open Browser" command and browser tab
//! context items (pills).
//!
//! This is a stub: the actual WebKit browser window management lives in
//! `crates/rover_browser/` and will be fully wired when the delegate is
//! refactored.

use rover_plugin_api::*;

/// Info about a browser tab (mirror of rover_browser types without the dep).
#[derive(Debug, Clone)]
pub struct TabInfo {
    pub title: String,
    pub url: String,
    pub is_active: bool,
}

pub struct BrowserPlugin {
    tabs: Vec<TabInfo>,
}

impl BrowserPlugin {
    pub fn new() -> Self {
        Self { tabs: Vec::new() }
    }

    /// Update the tab list from the live browser state.
    pub fn set_tabs(&mut self, tabs: Vec<TabInfo>) {
        self.tabs = tabs;
    }
}

impl Default for BrowserPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for BrowserPlugin {
    fn id(&self) -> &str {
        "builtin.browser"
    }
    fn name(&self) -> &str {
        "Browser"
    }
    fn description(&self) -> &str {
        "Keyboard-driven WebKit browser"
    }

    fn on_load(&mut self, _ctx: &mut PluginContext) -> Result<(), PluginError> {
        Ok(())
    }

    fn match_mode(&self) -> MatchMode {
        MatchMode::FuzzyCommand
    }

    fn priority(&self) -> i32 {
        5
    }

    fn commands(&self) -> Vec<CommandDescriptor> {
        vec![CommandDescriptor {
            id: "open_browser".to_string(),
            name: "Open Browser".to_string(),
            description: "Open the keyboard-driven browser".to_string(),
            icon: Some(IconHint::Emoji("globe".to_string())),
            keywords: vec![
                "web".to_string(),
                "safari".to_string(),
                "browse".to_string(),
            ],
            has_submenu: false,
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
        if match_id == "open_browser" {
            return ConfirmResult::OpenWindow(WindowRequest::Browser { url: None });
        }
        ConfirmResult::Stay
    }

    fn context_items(&self, _ctx: &PluginContext) -> Vec<ContextItem> {
        self.tabs
            .iter()
            .enumerate()
            .map(|(i, tab)| ContextItem {
                plugin_id: self.id().to_string(),
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

    fn open_window(&mut self, _ctx: &mut PluginContext) -> Option<WindowRequest> {
        Some(WindowRequest::Browser { url: None })
    }
}
