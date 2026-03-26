//! AI Assistant plugin — Apple Intelligence via FoundationModels.
//!
//! Match mode: `Fallback` — fires when query >= 6 chars and no good fuzzy
//! matches. Also responds to `?` and `ai ` prefixes via `on_query` check.

use rover_plugin_api::*;
use std::any::Any;

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

    /// The last query submitted for AI processing.
    pub fn last_query(&self) -> &str {
        &self.last_query
    }

    /// Extract the actual query, stripping `?` or `ai ` prefix if present.
    fn extract_query(query: &str) -> &str {
        query
            .strip_prefix("? ")
            .or_else(|| query.strip_prefix('?'))
            .or_else(|| query.strip_prefix("ai "))
            .unwrap_or(query)
            .trim()
    }

    /// Whether the query explicitly requests AI (via prefix).
    fn is_explicit_ai(query: &str) -> bool {
        query.starts_with('?') || query.starts_with("ai ")
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
        // Primary mode is Fallback; the registry also checks this plugin
        // when query starts with `?` or `ai ` via the AlwaysEvaluate below.
        MatchMode::Fallback { min_query_len: 6 }
    }

    fn priority(&self) -> i32 { -10 } // Low priority — shown after other results

    fn on_query(&mut self, query: &str, _ctx: &PluginContext) -> Vec<MatchResult> {
        let actual_query = Self::extract_query(query);
        self.last_query = actual_query.to_string();

        if self.loading {
            return vec![MatchResult {
                id: "ai_loading".to_string(),
                title: "Thinking...".to_string(),
                subtitle: Some(actual_query.to_string()),
                icon: Some(IconHint::Emoji("brain".to_string())),
                score: 0.5,
                plugin_id: self.id().to_string(),
                render_hint: Some(RenderHint::StatePill {
                    label: "AI".to_string(),
                    color: "purple".to_string(),
                }),
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
                render_hint: Some(RenderHint::StatePill {
                    label: "AI".to_string(),
                    color: "purple".to_string(),
                }),
            }];
        }

        // Show "Ask AI" placeholder for explicit prefix or fallback queries
        if !actual_query.is_empty() {
            return vec![MatchResult {
                id: "ai_ask".to_string(),
                title: format!("Ask AI: {}", actual_query),
                subtitle: Some("Press Enter to ask".to_string()),
                icon: Some(IconHint::Emoji("sparkles".to_string())),
                score: 0.5,
                plugin_id: self.id().to_string(),
                render_hint: Some(RenderHint::StatePill {
                    label: "AI".to_string(),
                    color: "purple".to_string(),
                }),
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
        match match_id {
            "ai_result" => {
                if let Some(result) = &self.last_result {
                    return ConfirmResult::CopyToClipboard(result.clone());
                }
                ConfirmResult::Stay
            }
            "ai_ask" => {
                // Signal the delegate to start the async AI request
                self.loading = true;
                self.last_result = None;
                ConfirmResult::Stay
            }
            _ => ConfirmResult::Stay,
        }
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

    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
}
