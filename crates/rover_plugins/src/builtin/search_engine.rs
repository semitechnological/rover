//! Search Engine plugin — resolves bang (`!g query`) and prefix (`g query`)
//! patterns into search URLs.
//!
//! Match mode: `AlwaysEvaluate` — checks every query for search patterns.

use rover_plugin_api::*;
use std::any::Any;

/// Extra search engine aliases beyond rover_browser::SEARCH_ENGINES.
const EXTRA_ENGINES: &[(&str, &str)] = &[
    ("amzn", "https://www.amazon.com/s?k={query}"),
    ("y", "https://search.yahoo.com/search?p={query}"),
    ("yahoo", "https://search.yahoo.com/search?p={query}"),
    ("bing", "https://www.bing.com/search?q={query}"),
    ("b", "https://www.bing.com/search?q={query}"),
    ("npm", "https://www.npmjs.com/search?q={query}"),
    ("crates", "https://crates.io/search?q={query}"),
    ("mdn", "https://developer.mozilla.org/en-US/search?q={query}"),
];

pub struct SearchEnginePlugin {
    last_url: Option<String>,
}

impl SearchEnginePlugin {
    pub fn new() -> Self {
        Self { last_url: None }
    }

    /// Core engine resolution: given a prefix and search query, return a URL.
    fn resolve_engine(prefix: &str, search_query: &str) -> Option<String> {
        if search_query.is_empty() {
            return None;
        }
        let prefix_lower = prefix.to_lowercase();
        for engine in rover_browser::SEARCH_ENGINES {
            if engine.long_prefix == prefix_lower
                || engine.short_prefix.map(|s| s == prefix_lower).unwrap_or(false)
            {
                let encoded = urlencoding::encode(search_query);
                return Some(engine.url_template.replace("{query}", &encoded));
            }
        }
        for (alias, template) in EXTRA_ENGINES {
            if prefix_lower == *alias {
                let encoded = urlencoding::encode(search_query);
                return Some(template.replace("{query}", &encoded));
            }
        }
        None
    }

    /// Resolve plain search prefix (google query, g query, yt query) → URL.
    fn resolve_search_prefix(query: &str) -> Option<String> {
        let (prefix, search_query) = query.split_once(' ')?;
        Self::resolve_engine(prefix, search_query.trim())
    }

    /// Resolve bang syntax (!g query → Google, !amzn shoes → Amazon, etc.)
    fn resolve_bang(query: &str) -> Option<String> {
        let without_bang = query.strip_prefix('!')?;
        let (prefix, search_query) = without_bang.split_once(' ')?;
        Self::resolve_engine(prefix, search_query.trim())
    }
}

impl Default for SearchEnginePlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for SearchEnginePlugin {
    fn id(&self) -> &str { "builtin.search_engine" }
    fn name(&self) -> &str { "Search Engines" }
    fn description(&self) -> &str { "Bang and prefix search (e.g. !g query, yt cats)" }

    fn on_load(&mut self, _ctx: &mut PluginContext) -> Result<(), PluginError> {
        Ok(())
    }

    fn match_mode(&self) -> MatchMode {
        MatchMode::AlwaysEvaluate
    }

    fn priority(&self) -> i32 { 90 }

    fn on_query(&mut self, query: &str, _ctx: &PluginContext) -> Vec<MatchResult> {
        self.last_url = None;
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return vec![];
        }

        // Try bang syntax first, then prefix
        let url = Self::resolve_bang(trimmed)
            .or_else(|| Self::resolve_search_prefix(trimmed));

        if let Some(url) = url {
            self.last_url = Some(url.clone());
            vec![MatchResult {
                id: "search_url".to_string(),
                title: url.clone(),
                subtitle: Some("Press Enter to open".to_string()),
                icon: Some(IconHint::Emoji("magnifying_glass".to_string())),
                score: 0.95,
                plugin_id: self.id().to_string(),
                render_hint: Some(RenderHint::StatePill {
                    label: "URL".to_string(),
                    color: "blue".to_string(),
                }),
            }]
        } else {
            vec![]
        }
    }

    fn on_confirm(
        &mut self,
        _match_id: &str,
        _query: &str,
        _secondary: bool,
        _ctx: &mut PluginContext,
    ) -> ConfirmResult {
        if let Some(url) = &self.last_url {
            ConfirmResult::OpenUrl(url.clone())
        } else {
            ConfirmResult::Stay
        }
    }

    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bang_google() {
        let mut plugin = SearchEnginePlugin::new();
        let ctx = PluginContext::new(PluginConfig::default());
        let results = plugin.on_query("!g rust lang", &ctx);
        assert_eq!(results.len(), 1);
        assert!(results[0].title.contains("google.com"));
        assert!(results[0].title.contains("rust%20lang"));
    }

    #[test]
    fn prefix_youtube() {
        let mut plugin = SearchEnginePlugin::new();
        let ctx = PluginContext::new(PluginConfig::default());
        let results = plugin.on_query("yt cats", &ctx);
        assert_eq!(results.len(), 1);
        assert!(results[0].title.contains("youtube.com"));
    }

    #[test]
    fn bang_amazon() {
        let mut plugin = SearchEnginePlugin::new();
        let ctx = PluginContext::new(PluginConfig::default());
        let results = plugin.on_query("!amzn shoes", &ctx);
        assert_eq!(results.len(), 1);
        assert!(results[0].title.contains("amazon.com"));
    }

    #[test]
    fn no_result_for_plain_text() {
        let mut plugin = SearchEnginePlugin::new();
        let ctx = PluginContext::new(PluginConfig::default());
        let results = plugin.on_query("hello world", &ctx);
        assert!(results.is_empty());
    }

    #[test]
    fn no_result_for_empty_search() {
        let mut plugin = SearchEnginePlugin::new();
        let ctx = PluginContext::new(PluginConfig::default());
        let results = plugin.on_query("!g ", &ctx);
        assert!(results.is_empty());
    }
}
