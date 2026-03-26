use rover_plugin_api::*;
use std::collections::HashMap;
use std::path::Path;

/// Central registry that holds all loaded plugins and dispatches queries.
pub struct PluginRegistry {
    plugins: Vec<Box<dyn Plugin>>,
    enabled: HashMap<String, bool>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
            enabled: HashMap::new(),
        }
    }

    /// Register a built-in (compiled-in) plugin.
    pub fn register(&mut self, plugin: Box<dyn Plugin>) {
        let id = plugin.id().to_string();
        self.enabled.entry(id).or_insert(true);
        self.plugins.push(plugin);
    }

    /// Check whether a plugin is enabled.
    pub fn is_enabled(&self, id: &str) -> bool {
        self.enabled.get(id).copied().unwrap_or(true)
    }

    /// Enable or disable a plugin.
    pub fn set_enabled(&mut self, id: &str, enabled: bool) {
        self.enabled.insert(id.to_string(), enabled);
    }

    /// Load external plugins from a directory (future: WASM/dylib).
    /// Currently a no-op stub.
    pub fn load_external(&mut self, _dir: &Path) -> Result<(), PluginError> {
        // Future: scan dir for .wasm / .dylib files, load them, call on_load.
        Ok(())
    }

    /// Initialize all enabled plugins.
    pub fn init_all(&mut self, ctx: &mut PluginContext) {
        let mut failed = Vec::new();
        for plugin in &mut self.plugins {
            let id = plugin.id().to_string();
            if !self.enabled.get(&id).copied().unwrap_or(true) {
                log::info!("Plugin '{}' is disabled, skipping init", id);
                continue;
            }
            match plugin.on_load(ctx) {
                Ok(()) => {
                    log::info!("Plugin '{}' ({}) loaded successfully", plugin.name(), id);
                }
                Err(e) => {
                    log::warn!("Plugin '{}' ({}) failed to load: {}", plugin.name(), id, e);
                    failed.push(id);
                }
            }
        }
        for id in failed {
            self.enabled.insert(id, false);
        }
    }

    /// Unload all plugins (called on app quit).
    pub fn unload_all(&mut self) {
        for plugin in &mut self.plugins {
            plugin.on_unload();
        }
    }

    /// Collect all command descriptors from all enabled plugins.
    pub fn all_commands(&self) -> Vec<(String, CommandDescriptor)> {
        let mut result = Vec::new();
        for plugin in &self.plugins {
            let id = plugin.id().to_string();
            if !self.is_enabled(&id) {
                continue;
            }
            for cmd in plugin.commands() {
                result.push((id.clone(), cmd));
            }
        }
        result
    }

    /// Query all enabled plugins and merge results.
    ///
    /// The caller provides context about the current query state:
    /// - `query`: the raw user input
    /// - `has_good_fuzzy_matches`: whether the fuzzy matcher found high-quality
    ///   matches from FuzzyCommand plugins
    /// - `has_calc_result`: whether the calculator produced a result
    pub fn query(
        &mut self,
        query: &str,
        has_good_fuzzy_matches: bool,
        has_calc_result: bool,
        ctx: &PluginContext,
    ) -> Vec<MatchResult> {
        let mut results = Vec::new();

        for plugin in &mut self.plugins {
            let id = plugin.id().to_string();
            if !self.enabled.get(&id).copied().unwrap_or(true) {
                continue;
            }

            let should_query = match plugin.match_mode() {
                MatchMode::AlwaysEvaluate => true,
                MatchMode::Prefix(ref prefix) => query.starts_with(prefix.as_str()),
                MatchMode::Fallback { min_query_len } => {
                    query.len() >= min_query_len && !has_good_fuzzy_matches && !has_calc_result
                }
                MatchMode::FuzzyCommand => {
                    // FuzzyCommand plugins are queried by the registry's
                    // fuzzy matching layer, not directly here.
                    false
                }
            };

            if should_query {
                let mut plugin_results = plugin.on_query(query, ctx);
                // Tag results with the plugin id
                for r in &mut plugin_results {
                    if r.plugin_id.is_empty() {
                        r.plugin_id = id.clone();
                    }
                }
                results.extend(plugin_results);
            }
        }

        // Sort by score descending, then by plugin priority
        results.sort_by(|a, b| {
            let pa = self.plugin_priority(&a.plugin_id);
            let pb = self.plugin_priority(&b.plugin_id);
            pb.cmp(&pa).then(b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal))
        });

        results
    }

    /// Confirm a match result from a specific plugin.
    pub fn confirm(
        &mut self,
        plugin_id: &str,
        match_id: &str,
        query: &str,
        secondary: bool,
        ctx: &mut PluginContext,
    ) -> ConfirmResult {
        for plugin in &mut self.plugins {
            if plugin.id() == plugin_id {
                return plugin.on_confirm(match_id, query, secondary, ctx);
            }
        }
        log::warn!("No plugin found for id '{}'", plugin_id);
        ConfirmResult::Stay
    }

    /// Confirm a submenu item.
    pub fn submenu_confirm(
        &mut self,
        plugin_id: &str,
        match_id: &str,
        query: &str,
        ctx: &mut PluginContext,
    ) -> ConfirmResult {
        for plugin in &mut self.plugins {
            if plugin.id() == plugin_id {
                return plugin.on_submenu_confirm(match_id, query, ctx);
            }
        }
        ConfirmResult::Stay
    }

    /// Get submenu items from a specific plugin.
    pub fn submenu_items(
        &mut self,
        plugin_id: &str,
        query: &str,
        ctx: &PluginContext,
    ) -> Vec<MatchResult> {
        for plugin in &mut self.plugins {
            if plugin.id() == plugin_id {
                return plugin.submenu_items(query, ctx);
            }
        }
        vec![]
    }

    /// Collect context items (pills) from all enabled plugins.
    pub fn context_items(&self, ctx: &PluginContext) -> Vec<ContextItem> {
        let mut items = Vec::new();
        for plugin in &self.plugins {
            if !self.is_enabled(plugin.id()) {
                continue;
            }
            items.extend(plugin.context_items(ctx));
        }
        items
    }

    /// Collect inline content from all enabled plugins.
    pub fn inline_content(&self, ctx: &PluginContext) -> Vec<(String, InlineContent)> {
        let mut results = Vec::new();
        for plugin in &self.plugins {
            if !self.is_enabled(plugin.id()) {
                continue;
            }
            if let Some(content) = plugin.inline_content(ctx) {
                results.push((plugin.id().to_string(), content));
            }
        }
        results
    }

    /// Get a reference to a plugin by ID.
    pub fn get(&self, id: &str) -> Option<&dyn Plugin> {
        self.plugins.iter().find(|p| p.id() == id).map(|p| &**p)
    }

    /// Get a mutable reference to a plugin by ID.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut Box<dyn Plugin>> {
        self.plugins.iter_mut().find(|p| p.id() == id)
    }

    /// Get the priority of a plugin by ID.
    fn plugin_priority(&self, id: &str) -> i32 {
        self.get(id).map(|p| p.priority()).unwrap_or(0)
    }

    /// Number of registered plugins.
    pub fn count(&self) -> usize {
        self.plugins.len()
    }

    /// Iterate over all plugins.
    pub fn iter(&self) -> impl Iterator<Item = &dyn Plugin> {
        self.plugins.iter().map(|p| &**p)
    }

    /// Iterate mutably over all plugins.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Box<dyn Plugin>> + '_ {
        self.plugins.iter_mut()
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}
