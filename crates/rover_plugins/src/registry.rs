use crate::manifest;
use rover_plugin_api::*;
use std::collections::HashMap;
use std::path::Path;

/// Central registry that holds all loaded plugins and dispatches queries.
pub struct PluginRegistry {
    plugins: Vec<Box<dyn Plugin>>,
    enabled: HashMap<String, bool>,
    /// Discovered external plugin manifests (id -> (manifest, dir)).
    external_manifests: HashMap<String, (manifest::PluginManifest, std::path::PathBuf)>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
            enabled: HashMap::new(),
            external_manifests: HashMap::new(),
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
    ///
    /// Scans for `plugin.toml` manifests in subdirectories of `dir`. Currently
    /// only indexes manifests — actual WASM/dylib loading is reserved for the
    /// future. Discovered manifests are logged and their enabled state is
    /// registered so the settings UI can toggle them.
    pub fn load_external(&mut self, dir: &Path) -> Result<(), PluginError> {
        let manifests = manifest::scan_plugin_manifests(dir);
        for (m, _plugin_dir) in &manifests {
            log::info!(
                "Discovered external plugin: {} ({}) v{} [kind={}]",
                m.name, m.id, m.version, m.kind
            );
            self.enabled.entry(m.id.clone()).or_insert(m.enabled);
            // Future: instantiate WASM/dylib runtime and register as a Plugin.
        }
        self.external_manifests = manifests
            .into_iter()
            .map(|(m, dir)| (m.id.clone(), (m, dir)))
            .collect();
        Ok(())
    }

    /// Return loaded external plugin manifests.
    pub fn external_manifests(&self) -> &HashMap<String, (manifest::PluginManifest, std::path::PathBuf)> {
        &self.external_manifests
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

#[cfg(test)]
mod tests {
    use super::*;

    // ── Test helpers ──────────────────────────────────────────────────────

    /// A minimal test plugin with configurable behaviour.
    struct TestPlugin {
        plugin_id: String,
        plugin_name: String,
        plugin_priority: i32,
        mode: MatchMode,
        query_results: Vec<MatchResult>,
        commands: Vec<CommandDescriptor>,
        load_should_fail: bool,
    }

    impl TestPlugin {
        fn new(id: &str, name: &str, priority: i32, mode: MatchMode) -> Self {
            Self {
                plugin_id: id.to_string(),
                plugin_name: name.to_string(),
                plugin_priority: priority,
                mode,
                query_results: Vec::new(),
                commands: Vec::new(),
                load_should_fail: false,
            }
        }

        fn with_query_results(mut self, results: Vec<MatchResult>) -> Self {
            self.query_results = results;
            self
        }

        fn with_commands(mut self, commands: Vec<CommandDescriptor>) -> Self {
            self.commands = commands;
            self
        }

        fn failing_load(mut self) -> Self {
            self.load_should_fail = true;
            self
        }
    }

    impl Plugin for TestPlugin {
        fn id(&self) -> &str { &self.plugin_id }
        fn name(&self) -> &str { &self.plugin_name }
        fn description(&self) -> &str { "test plugin" }
        fn priority(&self) -> i32 { self.plugin_priority }
        fn match_mode(&self) -> MatchMode { self.mode.clone() }

        fn on_load(&mut self, _ctx: &mut PluginContext) -> Result<(), PluginError> {
            if self.load_should_fail {
                Err(PluginError::InitFailed("test failure".to_string()))
            } else {
                Ok(())
            }
        }

        fn commands(&self) -> Vec<CommandDescriptor> {
            self.commands.clone()
        }

        fn on_query(&mut self, _query: &str, _ctx: &PluginContext) -> Vec<MatchResult> {
            self.query_results.clone()
        }

        fn on_confirm(
            &mut self,
            match_id: &str,
            _query: &str,
            _secondary: bool,
            _ctx: &mut PluginContext,
        ) -> ConfirmResult {
            if match_id == "dismiss" {
                ConfirmResult::Dismiss
            } else {
                ConfirmResult::Stay
            }
        }
    }

    fn make_match(id: &str, title: &str, score: f64, plugin_id: &str) -> MatchResult {
        MatchResult {
            id: id.to_string(),
            title: title.to_string(),
            subtitle: None,
            icon: None,
            score,
            plugin_id: plugin_id.to_string(),
        }
    }

    fn test_ctx() -> PluginContext {
        PluginContext::new(PluginConfig::default())
    }

    // ── Tests ─────────────────────────────────────────────────────────────

    #[test]
    fn register_and_count() {
        let mut reg = PluginRegistry::new();
        assert_eq!(reg.count(), 0);

        reg.register(Box::new(TestPlugin::new(
            "test.a", "A", 0, MatchMode::AlwaysEvaluate,
        )));
        assert_eq!(reg.count(), 1);

        reg.register(Box::new(TestPlugin::new(
            "test.b", "B", 0, MatchMode::AlwaysEvaluate,
        )));
        assert_eq!(reg.count(), 2);
    }

    #[test]
    fn enable_disable() {
        let mut reg = PluginRegistry::new();
        reg.register(Box::new(TestPlugin::new(
            "test.a", "A", 0, MatchMode::AlwaysEvaluate,
        )));

        assert!(reg.is_enabled("test.a"));
        reg.set_enabled("test.a", false);
        assert!(!reg.is_enabled("test.a"));
        reg.set_enabled("test.a", true);
        assert!(reg.is_enabled("test.a"));
    }

    #[test]
    fn init_all_disables_failed_plugins() {
        let mut reg = PluginRegistry::new();
        reg.register(Box::new(
            TestPlugin::new("test.ok", "OK", 0, MatchMode::AlwaysEvaluate),
        ));
        reg.register(Box::new(
            TestPlugin::new("test.fail", "Fail", 0, MatchMode::AlwaysEvaluate).failing_load(),
        ));

        let mut ctx = test_ctx();
        reg.init_all(&mut ctx);

        assert!(reg.is_enabled("test.ok"));
        assert!(!reg.is_enabled("test.fail"));
    }

    #[test]
    fn query_merging_multiple_plugins() {
        let mut reg = PluginRegistry::new();

        let calc = TestPlugin::new("test.calc", "Calc", 100, MatchMode::AlwaysEvaluate)
            .with_query_results(vec![make_match("r1", "= 42", 1.0, "test.calc")]);
        let ai = TestPlugin::new(
            "test.ai",
            "AI",
            -10,
            MatchMode::Fallback { min_query_len: 3 },
        )
        .with_query_results(vec![make_match("r2", "AI answer", 0.5, "test.ai")]);

        reg.register(Box::new(calc));
        reg.register(Box::new(ai));

        let mut ctx = test_ctx();
        reg.init_all(&mut ctx);

        // With a short query and a good calc result, fallback should NOT fire
        let results = reg.query("2+2", false, true, &test_ctx());
        // Only calc should produce results
        assert!(results.iter().all(|r| r.plugin_id == "test.calc"));

        // With a longer query and no good matches and no calc, fallback fires
        let results = reg.query("hello world long query", false, false, &test_ctx());
        assert!(results.iter().any(|r| r.plugin_id == "test.ai"));
    }

    #[test]
    fn priority_ordering() {
        let mut reg = PluginRegistry::new();

        let high = TestPlugin::new("test.high", "High", 100, MatchMode::AlwaysEvaluate)
            .with_query_results(vec![make_match("h1", "High result", 0.5, "test.high")]);
        let low = TestPlugin::new("test.low", "Low", -10, MatchMode::AlwaysEvaluate)
            .with_query_results(vec![make_match("l1", "Low result", 0.9, "test.low")]);

        reg.register(Box::new(high));
        reg.register(Box::new(low));

        let mut ctx = test_ctx();
        reg.init_all(&mut ctx);

        let results = reg.query("test", false, false, &test_ctx());
        assert_eq!(results.len(), 2);
        // High priority plugin's result should come first despite lower score
        assert_eq!(results[0].plugin_id, "test.high");
        assert_eq!(results[1].plugin_id, "test.low");
    }

    #[test]
    fn confirm_dispatches_to_correct_plugin() {
        let mut reg = PluginRegistry::new();

        let plugin_a = TestPlugin::new("test.a", "A", 0, MatchMode::AlwaysEvaluate);
        let plugin_b = TestPlugin::new("test.b", "B", 0, MatchMode::AlwaysEvaluate);

        reg.register(Box::new(plugin_a));
        reg.register(Box::new(plugin_b));

        let mut ctx = test_ctx();
        reg.init_all(&mut ctx);

        // "dismiss" match_id triggers Dismiss in TestPlugin
        let result = reg.confirm("test.a", "dismiss", "", false, &mut ctx);
        assert!(matches!(result, ConfirmResult::Dismiss));

        // "other" match_id triggers Stay
        let result = reg.confirm("test.a", "other", "", false, &mut ctx);
        assert!(matches!(result, ConfirmResult::Stay));

        // Unknown plugin returns Stay
        let result = reg.confirm("test.unknown", "dismiss", "", false, &mut ctx);
        assert!(matches!(result, ConfirmResult::Stay));
    }

    #[test]
    fn disabled_plugin_not_queried() {
        let mut reg = PluginRegistry::new();

        let plugin = TestPlugin::new("test.disabled", "Disabled", 0, MatchMode::AlwaysEvaluate)
            .with_query_results(vec![make_match("d1", "Should not appear", 1.0, "test.disabled")]);

        reg.register(Box::new(plugin));
        reg.set_enabled("test.disabled", false);

        let results = reg.query("test", false, false, &test_ctx());
        assert!(results.is_empty());
    }

    #[test]
    fn prefix_mode_only_matches_with_prefix() {
        let mut reg = PluginRegistry::new();

        let term = TestPlugin::new(
            "test.term",
            "Terminal",
            50,
            MatchMode::Prefix(">".to_string()),
        )
        .with_query_results(vec![make_match("t1", "$ ls", 1.0, "test.term")]);

        reg.register(Box::new(term));

        let mut ctx = test_ctx();
        reg.init_all(&mut ctx);

        // Without prefix — no results
        let results = reg.query("ls", false, false, &test_ctx());
        assert!(results.is_empty());

        // With prefix — results
        let results = reg.query("> ls", false, false, &test_ctx());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].plugin_id, "test.term");
    }

    #[test]
    fn fuzzy_command_mode_not_directly_queried() {
        let mut reg = PluginRegistry::new();

        let app = TestPlugin::new("test.app", "App", 10, MatchMode::FuzzyCommand)
            .with_query_results(vec![make_match("a1", "Safari", 1.0, "test.app")]);

        reg.register(Box::new(app));

        let mut ctx = test_ctx();
        reg.init_all(&mut ctx);

        // FuzzyCommand plugins are NOT queried via registry.query() —
        // their on_query is only called by the fuzzy matching layer.
        let results = reg.query("safari", false, false, &test_ctx());
        assert!(results.is_empty());
    }

    #[test]
    fn all_commands_from_enabled_plugins() {
        let mut reg = PluginRegistry::new();

        let a = TestPlugin::new("test.a", "A", 0, MatchMode::FuzzyCommand).with_commands(vec![
            CommandDescriptor {
                id: "cmd1".to_string(),
                name: "Command One".to_string(),
                description: "First command".to_string(),
                icon: None,
                keywords: vec![],
                has_submenu: false,
            },
        ]);
        let b = TestPlugin::new("test.b", "B", 0, MatchMode::FuzzyCommand).with_commands(vec![
            CommandDescriptor {
                id: "cmd2".to_string(),
                name: "Command Two".to_string(),
                description: "Second command".to_string(),
                icon: None,
                keywords: vec![],
                has_submenu: false,
            },
        ]);

        reg.register(Box::new(a));
        reg.register(Box::new(b));

        let commands = reg.all_commands();
        assert_eq!(commands.len(), 2);

        // Disable one plugin
        reg.set_enabled("test.b", false);
        let commands = reg.all_commands();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].0, "test.a");
    }

    #[test]
    fn get_plugin_by_id() {
        let mut reg = PluginRegistry::new();
        reg.register(Box::new(TestPlugin::new(
            "test.x", "X", 0, MatchMode::AlwaysEvaluate,
        )));

        assert!(reg.get("test.x").is_some());
        assert!(reg.get("test.nonexistent").is_none());
    }

    #[test]
    fn unload_all_runs() {
        let mut reg = PluginRegistry::new();
        reg.register(Box::new(TestPlugin::new(
            "test.a", "A", 0, MatchMode::AlwaysEvaluate,
        )));
        // Should not panic
        reg.unload_all();
    }

    #[test]
    fn feature_flag_gating() {
        // This test verifies that the feature-flag mechanism works: a plugin
        // registered then disabled is excluded from queries and commands.
        let mut reg = PluginRegistry::new();

        let plugin = TestPlugin::new("test.gated", "Gated", 0, MatchMode::AlwaysEvaluate)
            .with_query_results(vec![make_match("g1", "Gated result", 1.0, "test.gated")])
            .with_commands(vec![CommandDescriptor {
                id: "gated_cmd".to_string(),
                name: "Gated Command".to_string(),
                description: "".to_string(),
                icon: None,
                keywords: vec![],
                has_submenu: false,
            }]);

        reg.register(Box::new(plugin));

        // Enabled: visible in queries and commands
        assert_eq!(reg.query("test", false, false, &test_ctx()).len(), 1);
        assert_eq!(reg.all_commands().len(), 1);

        // Disabled: invisible
        reg.set_enabled("test.gated", false);
        assert_eq!(reg.query("test", false, false, &test_ctx()).len(), 0);
        assert_eq!(reg.all_commands().len(), 0);

        // Re-enabled: visible again
        reg.set_enabled("test.gated", true);
        assert_eq!(reg.query("test", false, false, &test_ctx()).len(), 1);
        assert_eq!(reg.all_commands().len(), 1);
    }
}
