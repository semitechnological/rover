//! Help plugin — provides contextual help via `help` prefix.
//!
//! Match mode: `Prefix("help")` — activated when query starts with `help `.
//! The actual help content is generated asynchronously by the delegate using
//! the semantic index and AI session.

use rover_plugin_api::*;
use std::any::Any;

pub struct HelpPlugin {
    last_query: String,
    last_result: Option<String>,
    loading: bool,
}

impl HelpPlugin {
    pub fn new() -> Self {
        Self {
            last_query: String::new(),
            last_result: None,
            loading: false,
        }
    }

    /// Whether the help plugin is currently loading.
    pub fn is_loading(&self) -> bool {
        self.loading
    }

    /// Set the help result from an external source (semantic + AI).
    pub fn set_result(&mut self, result: Option<String>) {
        self.last_result = result;
        self.loading = false;
    }

    /// Mark the plugin as loading.
    pub fn set_loading(&mut self, loading: bool) {
        self.loading = loading;
    }

    /// The last query submitted for help.
    pub fn last_query(&self) -> &str {
        &self.last_query
    }
}

impl Default for HelpPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for HelpPlugin {
    fn id(&self) -> &str { "builtin.help" }
    fn name(&self) -> &str { "Help" }
    fn description(&self) -> &str { "Get help about Rover features" }

    fn on_load(&mut self, _ctx: &mut PluginContext) -> Result<(), PluginError> {
        Ok(())
    }

    fn match_mode(&self) -> MatchMode {
        MatchMode::Prefix("help".to_string())
    }

    fn priority(&self) -> i32 { 70 }

    fn on_query(&mut self, query: &str, _ctx: &PluginContext) -> Vec<MatchResult> {
        let help_query = query
            .strip_prefix("help ")
            .or_else(|| query.strip_prefix("help"))
            .unwrap_or("")
            .trim();

        self.last_query = help_query.to_string();

        if self.loading {
            return vec![MatchResult {
                id: "help_loading".to_string(),
                title: "Searching help...".to_string(),
                subtitle: Some(help_query.to_string()),
                icon: Some(IconHint::Emoji("question".to_string())),
                score: 0.9,
                plugin_id: self.id().to_string(),
                render_hint: Some(RenderHint::StatePill {
                    label: "HELP".to_string(),
                    color: "blue".to_string(),
                }),
            }];
        }

        if let Some(result) = &self.last_result {
            return vec![MatchResult {
                id: "help_result".to_string(),
                title: result.clone(),
                subtitle: None,
                icon: Some(IconHint::Emoji("bulb".to_string())),
                score: 0.9,
                plugin_id: self.id().to_string(),
                render_hint: None,
            }];
        }

        if help_query.is_empty() {
            return vec![MatchResult {
                id: "help_prompt".to_string(),
                title: "Type a topic to get help (e.g. help hotkey)".to_string(),
                subtitle: None,
                icon: Some(IconHint::Emoji("question".to_string())),
                score: 0.9,
                plugin_id: self.id().to_string(),
                render_hint: Some(RenderHint::StatePill {
                    label: "HELP".to_string(),
                    color: "blue".to_string(),
                }),
            }];
        }

        vec![MatchResult {
            id: "help_ask".to_string(),
            title: format!("Get help: {}", help_query),
            subtitle: Some("Press Enter to search".to_string()),
            icon: Some(IconHint::Emoji("question".to_string())),
            score: 0.9,
            plugin_id: self.id().to_string(),
            render_hint: Some(RenderHint::StatePill {
                label: "HELP".to_string(),
                color: "blue".to_string(),
            }),
        }]
    }

    fn on_confirm(
        &mut self,
        match_id: &str,
        _query: &str,
        _secondary: bool,
        _ctx: &mut PluginContext,
    ) -> ConfirmResult {
        match match_id {
            "help_result" => {
                if let Some(result) = &self.last_result {
                    return ConfirmResult::CopyToClipboard(result.clone());
                }
                ConfirmResult::Stay
            }
            "help_ask" => {
                // Signal the delegate to start async help lookup
                self.loading = true;
                ConfirmResult::Stay
            }
            _ => ConfirmResult::Stay,
        }
    }

    fn inline_content(&self, _ctx: &PluginContext) -> Option<InlineContent> {
        if self.loading {
            Some(InlineContent::Loading("Searching help...".to_string()))
        } else if let Some(result) = &self.last_result {
            Some(InlineContent::Text(result.clone()))
        } else {
            None
        }
    }

    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
}
