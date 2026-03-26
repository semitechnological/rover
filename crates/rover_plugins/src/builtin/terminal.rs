//! Terminal plugin — runs shell commands inline or opens a terminal window.
//!
//! Match mode: `Prefix(">")` — activated when query starts with `>` or `!`.
//! Also auto-suggests commands when terminal was recently focused (via
//! `ctx.terminal_was_focused`).

use rover_plugin_api::*;
use std::any::Any;

pub struct TerminalPlugin {
    last_output: Option<String>,
    running: bool,
}

impl TerminalPlugin {
    pub fn new() -> Self {
        Self {
            last_output: None,
            running: false,
        }
    }

    /// Set terminal output from an external execution.
    pub fn set_output(&mut self, output: Option<String>) {
        self.last_output = output;
        self.running = false;
    }

    /// Mark a command as running.
    pub fn set_running(&mut self, running: bool) {
        self.running = running;
    }
}

impl Default for TerminalPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for TerminalPlugin {
    fn id(&self) -> &str { "builtin.terminal" }
    fn name(&self) -> &str { "Terminal" }
    fn description(&self) -> &str { "Run shell commands or open a terminal window" }

    fn on_load(&mut self, _ctx: &mut PluginContext) -> Result<(), PluginError> {
        Ok(())
    }

    fn match_mode(&self) -> MatchMode {
        MatchMode::Prefix(">".to_string())
    }

    fn priority(&self) -> i32 { 50 }

    fn commands(&self) -> Vec<CommandDescriptor> {
        vec![CommandDescriptor {
            id: "open_terminal".to_string(),
            name: "Terminal".to_string(),
            description: "Open floating terminal window".to_string(),
            icon: Some(IconHint::Emoji("terminal".to_string())),
            keywords: vec!["shell".to_string(), "console".to_string(), "bash".to_string()],
            has_submenu: false,
        }]
    }

    fn on_query(&mut self, query: &str, ctx: &PluginContext) -> Vec<MatchResult> {
        // Check for explicit prefix
        let has_prefix = query.starts_with('>') || query.starts_with('!');

        // Auto-suggest: when terminal was focused, treat bare queries as commands
        let auto_suggest = !has_prefix && ctx.terminal_was_focused && !query.is_empty();

        if !has_prefix && !auto_suggest {
            return vec![];
        }

        let cmd = if has_prefix {
            query
                .strip_prefix('>')
                .or_else(|| query.strip_prefix('!'))
                .map(|s| s.trim())
                .unwrap_or("")
        } else {
            query.trim()
        };

        if cmd.is_empty() {
            return vec![MatchResult {
                id: "terminal_prompt".to_string(),
                title: "Type a command...".to_string(),
                subtitle: Some("Press Enter to run".to_string()),
                icon: Some(IconHint::Emoji("$".to_string())),
                score: 1.0,
                plugin_id: self.id().to_string(),
                render_hint: Some(RenderHint::StatePill {
                    label: "SHELL".to_string(),
                    color: "green".to_string(),
                }),
            }];
        }

        let mut results = vec![MatchResult {
            id: format!("run:{}", cmd),
            title: format!("$ {}", cmd),
            subtitle: Some("Press Enter to run".to_string()),
            icon: Some(IconHint::Emoji("$".to_string())),
            score: if auto_suggest { 0.7 } else { 1.0 },
            plugin_id: self.id().to_string(),
            render_hint: Some(RenderHint::StatePill {
                label: "SHELL".to_string(),
                color: "green".to_string(),
            }),
        }];

        // When auto-suggesting, lower the score so fuzzy app matches still rank higher
        if auto_suggest {
            results[0].score = 0.7;
        }

        results
    }

    fn on_confirm(
        &mut self,
        match_id: &str,
        _query: &str,
        _secondary: bool,
        _ctx: &mut PluginContext,
    ) -> ConfirmResult {
        if match_id == "open_terminal" {
            return ConfirmResult::OpenTerminal(None);
        }
        if let Some(cmd) = match_id.strip_prefix("run:") {
            return ConfirmResult::RunShell(cmd.to_string());
        }
        ConfirmResult::Stay
    }

    fn inline_content(&self, _ctx: &PluginContext) -> Option<InlineContent> {
        if self.running {
            Some(InlineContent::TerminalOutput {
                output: self.last_output.clone().unwrap_or_default(),
                running: true,
            })
        } else if let Some(output) = &self.last_output {
            Some(InlineContent::TerminalOutput {
                output: output.clone(),
                running: false,
            })
        } else {
            None
        }
    }

    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
}
