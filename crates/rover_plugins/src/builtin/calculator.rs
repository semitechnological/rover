//! Calculator plugin — evaluates math expressions inline.
//!
//! Match mode: `AlwaysEvaluate` — runs on every keystroke. Returns a result
//! only when the query is a valid math expression.

use rover_commands::calculator;
use rover_plugin_api::*;
use std::any::Any;

pub struct CalculatorPlugin {
    last_result: Option<String>,
    last_query: String,
}

impl CalculatorPlugin {
    pub fn new() -> Self {
        Self {
            last_result: None,
            last_query: String::new(),
        }
    }

    /// Whether the last query produced a calculator result.
    pub fn has_result(&self) -> bool {
        self.last_result.is_some()
    }

    /// The last evaluated result, if any.
    pub fn last_result(&self) -> Option<&str> {
        self.last_result.as_deref()
    }

    fn format_result(value: f64) -> String {
        if value == value.floor() && value.abs() < 1e15 {
            format!("{}", value as i64)
        } else {
            // Up to 10 decimal places, trim trailing zeros
            let s = format!("{:.10}", value);
            let s = s.trim_end_matches('0');
            let s = s.trim_end_matches('.');
            s.to_string()
        }
    }
}

impl Default for CalculatorPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for CalculatorPlugin {
    fn id(&self) -> &str { "builtin.calculator" }
    fn name(&self) -> &str { "Calculator" }
    fn description(&self) -> &str { "Evaluate math expressions inline" }

    fn on_load(&mut self, _ctx: &mut PluginContext) -> Result<(), PluginError> {
        Ok(())
    }

    fn match_mode(&self) -> MatchMode {
        MatchMode::AlwaysEvaluate
    }

    fn priority(&self) -> i32 { 100 }

    fn on_query(&mut self, query: &str, _ctx: &PluginContext) -> Vec<MatchResult> {
        self.last_query = query.to_string();
        self.last_result = None;

        let trimmed = query.trim();
        if trimmed.is_empty() {
            return vec![];
        }

        match calculator::evaluate(trimmed) {
            Some(value) => {
                let formatted = Self::format_result(value);
                // Don't show result if it's identical to the input (e.g. "42" → 42)
                if formatted == trimmed {
                    return vec![];
                }
                self.last_result = Some(formatted.clone());
                vec![MatchResult {
                    id: "calc_result".to_string(),
                    title: format!("= {}", formatted),
                    subtitle: Some(format!("{} = {}", trimmed, formatted)),
                    icon: Some(IconHint::Emoji("=".to_string())),
                    score: 1.0,
                    plugin_id: self.id().to_string(),
                    render_hint: None,
                }]
            }
            None => vec![],
        }
    }

    fn on_confirm(
        &mut self,
        _match_id: &str,
        _query: &str,
        _secondary: bool,
        _ctx: &mut PluginContext,
    ) -> ConfirmResult {
        if let Some(result) = &self.last_result {
            ConfirmResult::CopyToClipboard(result.clone())
        } else {
            ConfirmResult::Stay
        }
    }

    fn render_match(&self, _match_id: &str, _selected: bool) -> RenderHint {
        if let Some(result) = &self.last_result {
            RenderHint::WithSubtitle {
                subtitle: format!("Copy \"{}\" to clipboard", result),
            }
        } else {
            RenderHint::Default
        }
    }

    fn actions_for_match(&self, _match_id: &str) -> Vec<ActionDescriptor> {
        if self.last_result.is_some() {
            vec![ActionDescriptor {
                id: "copy".to_string(),
                name: "Copy result".to_string(),
                shortcut: Some("Enter".to_string()),
            }]
        } else {
            vec![]
        }
    }

    fn execute_action(
        &mut self,
        _match_id: &str,
        action_id: &str,
        _ctx: &mut PluginContext,
    ) -> ConfirmResult {
        if action_id == "copy" {
            if let Some(result) = &self.last_result {
                return ConfirmResult::CopyToClipboard(result.clone());
            }
        }
        ConfirmResult::Stay
    }

    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_evaluation() {
        let mut plugin = CalculatorPlugin::new();
        let ctx = PluginContext::new(PluginConfig::default());
        let results = plugin.on_query("2 + 3", &ctx);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "= 5");
        assert!(plugin.has_result());
    }

    #[test]
    fn no_result_for_plain_text() {
        let mut plugin = CalculatorPlugin::new();
        let ctx = PluginContext::new(PluginConfig::default());
        let results = plugin.on_query("hello world", &ctx);
        assert!(results.is_empty());
        assert!(!plugin.has_result());
    }

    #[test]
    fn no_result_for_identity() {
        let mut plugin = CalculatorPlugin::new();
        let ctx = PluginContext::new(PluginConfig::default());
        let results = plugin.on_query("42", &ctx);
        assert!(results.is_empty());
    }

    #[test]
    fn decimal_formatting() {
        assert_eq!(CalculatorPlugin::format_result(3.14), "3.14");
        assert_eq!(CalculatorPlugin::format_result(5.0), "5");
        assert_eq!(CalculatorPlugin::format_result(0.1 + 0.2), "0.3");
    }
}
