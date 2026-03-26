//! Quit plugin — detects `quit`/`close`/`exit` queries and quits the
//! frontmost macOS application.
//!
//! Match mode: `AlwaysEvaluate` — checks every query for quit patterns.

use rover_plugin_api::*;
use std::any::Any;

pub struct QuitPlugin;

impl QuitPlugin {
    pub fn new() -> Self {
        Self
    }

    fn matches_quit(query: &str) -> bool {
        let q = query.trim().to_lowercase();
        matches!(q.as_str(), "quit" | "close" | "exit" | "kill" | "force quit")
    }
}

impl Default for QuitPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for QuitPlugin {
    fn id(&self) -> &str { "builtin.quit" }
    fn name(&self) -> &str { "Quit App" }
    fn description(&self) -> &str { "Quit or force-quit the frontmost application" }

    fn on_load(&mut self, _ctx: &mut PluginContext) -> Result<(), PluginError> {
        Ok(())
    }

    fn match_mode(&self) -> MatchMode {
        MatchMode::AlwaysEvaluate
    }

    fn priority(&self) -> i32 { 80 }

    fn on_query(&mut self, query: &str, ctx: &PluginContext) -> Vec<MatchResult> {
        if !Self::matches_quit(query) {
            return vec![];
        }

        let app_name = ctx
            .frontmost_app
            .as_deref()
            .unwrap_or("frontmost app");

        let q = query.trim().to_lowercase();
        let is_force = q == "force quit" || q == "kill";

        vec![MatchResult {
            id: if is_force { "force_quit".to_string() } else { "quit".to_string() },
            title: if is_force {
                format!("Force Quit {}", app_name)
            } else {
                format!("Quit {}", app_name)
            },
            subtitle: Some("Press Enter to confirm".to_string()),
            icon: Some(IconHint::Emoji("x".to_string())),
            score: 0.9,
            plugin_id: self.id().to_string(),
            render_hint: Some(RenderHint::StatePill {
                label: if is_force { "FORCE QUIT".to_string() } else { "QUIT".to_string() },
                color: "red".to_string(),
            }),
        }]
    }

    fn on_confirm(
        &mut self,
        match_id: &str,
        _query: &str,
        _secondary: bool,
        ctx: &mut PluginContext,
    ) -> ConfirmResult {
        let app_name = ctx
            .frontmost_app
            .as_deref()
            .unwrap_or("frontmost app")
            .to_string();

        match match_id {
            "force_quit" => {
                // Force quit via AppleScript
                let script = format!(
                    r#"tell application "System Events" to set frontApp to name of first application process whose frontmost is true
tell application frontApp to quit"#
                );
                let _ = std::process::Command::new("osascript")
                    .arg("-e")
                    .arg(&script)
                    .spawn();
                ConfirmResult::Dismiss
            }
            "quit" => ConfirmResult::QuitApp(app_name),
            _ => ConfirmResult::Stay,
        }
    }

    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_quit_patterns() {
        assert!(QuitPlugin::matches_quit("quit"));
        assert!(QuitPlugin::matches_quit("Quit"));
        assert!(QuitPlugin::matches_quit("CLOSE"));
        assert!(QuitPlugin::matches_quit("exit"));
        assert!(QuitPlugin::matches_quit("kill"));
        assert!(QuitPlugin::matches_quit("force quit"));
        assert!(!QuitPlugin::matches_quit("hello"));
        assert!(!QuitPlugin::matches_quit("quitting"));
    }

    #[test]
    fn query_returns_result_with_app_name() {
        let mut plugin = QuitPlugin::new();
        let mut ctx = PluginContext::new(PluginConfig::default());
        ctx.frontmost_app = Some("Safari".to_string());
        let results = plugin.on_query("quit", &ctx);
        assert_eq!(results.len(), 1);
        assert!(results[0].title.contains("Safari"));
    }

    #[test]
    fn no_result_for_non_quit() {
        let mut plugin = QuitPlugin::new();
        let ctx = PluginContext::new(PluginConfig::default());
        let results = plugin.on_query("hello", &ctx);
        assert!(results.is_empty());
    }
}
