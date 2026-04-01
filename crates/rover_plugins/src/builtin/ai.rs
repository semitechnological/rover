//! AI Assistant plugin — Apple Intelligence via FoundationModels.
//!
//! Match mode: `Fallback` — fires when query >= 6 chars and no good fuzzy
//! matches. Also responds to `Prefix("?")` and `Prefix("ai ")`.
//!
//! This is a stub: the actual AI session integration lives in `src/ai.rs`
//! and will be wired in when the delegate is refactored.

use rover_plugin_api::*;

pub struct AiPlugin {
    enabled: bool,
    last_query: String,
    last_result: Option<String>,
    loading: bool,
}

impl AiPlugin {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            last_query: String::new(),
            last_result: None,
            loading: false,
        }
    }

    /// Whether the AI plugin is currently loading a response.
    pub fn is_loading(&self) -> bool {
        self.loading
    }

    /// Set the result from an external AI session.
    pub fn set_result(&mut self, result: Option<String>) {
        self.last_result = result;
        self.loading = false;
    }

    /// Mark the plugin as loading.
    pub fn set_loading(&mut self, loading: bool) {
        self.loading = loading;
    }
}

impl Plugin for AiPlugin {
    fn id(&self) -> &str { "builtin.ai" }
    fn name(&self) -> &str { "AI Assistant" }
    fn description(&self) -> &str { "Apple Intelligence powered assistant" }

    fn on_load(&mut self, _ctx: &mut PluginContext) -> Result<(), PluginError> {
        if !self.enabled {
            return Err(PluginError::NotAvailable(
                "AI requires macOS 26+, Apple Silicon, and AI enabled in settings".to_string(),
            ));
        }
        Ok(())
    }

    fn match_mode(&self) -> MatchMode {
        // The AI plugin uses multiple modes; the registry handles this by
        // checking both Fallback and Prefix("?"). The primary mode is Fallback.
        MatchMode::Fallback { min_query_len: 6 }
    }

    fn priority(&self) -> i32 { -10 } // Low priority — shown after other results

    fn on_query(&mut self, query: &str, _ctx: &PluginContext) -> Vec<MatchResult> {
        self.last_query = query.to_string();

        // Stub: in the full implementation, this would trigger an async AI
        // query and return a loading indicator.
        if self.loading {
            return vec![MatchResult {
                id: "ai_loading".to_string(),
                title: "Thinking...".to_string(),
                subtitle: Some(query.to_string()),
                icon: Some(IconHint::Emoji("brain".to_string())),
                score: 0.5,
                plugin_id: self.id().to_string(),
            }];
        }

        if let Some(result) = &self.last_result {
            return vec![MatchResult {
                id: "ai_result".to_string(),
                title: result.clone(),
                subtitle: None,
                icon: Some(IconHint::Emoji("sparkles".to_string())),
                score: 0.5,
                plugin_id: self.id().to_string(),
            }];
        }

        vec![]
    }

    fn on_confirm(
        &mut self,
        match_id: &str,
        _query: &str,
        _secondary: bool,
        _ctx: &mut PluginContext,
    ) -> ConfirmResult {
        if match_id == "ai_result" {
            if let Some(result) = &self.last_result {
                return ConfirmResult::CopyToClipboard(result.clone());
            }
        }
        ConfirmResult::Stay
    }

    fn inline_content(&self, _ctx: &PluginContext) -> Option<InlineContent> {
        if self.loading {
            Some(InlineContent::Loading("Thinking...".to_string()))
        } else if let Some(result) = &self.last_result {
            Some(InlineContent::Text(result.clone()))
        } else {
            None
        }
    }
}
