//! Plugin manifest loading for external plugins.
//!
//! External plugins live in `~/.config/rover/plugins/<plugin_id>/plugin.toml`.
//! Each manifest describes the plugin metadata and how to load it.

use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Parsed plugin manifest from `plugin.toml`.
#[derive(Debug, Clone, Deserialize)]
pub struct PluginManifest {
    /// Unique plugin identifier, e.g. `"ext.my_plugin"`.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Short description.
    #[serde(default)]
    pub description: String,
    /// Semver version string.
    #[serde(default = "default_version")]
    pub version: String,
    /// Author name or email.
    #[serde(default)]
    pub author: String,
    /// Plugin type: "wasm" or "native" (legacy, use `runtime` instead).
    #[serde(default = "default_kind")]
    pub kind: String,
    /// Relative path to the plugin binary (e.g. `"plugin.wasm"`).
    #[serde(default)]
    pub binary: String,
    /// Plugin runtime: "native", "crepuscularity", or "web".
    /// Determines how the plugin's UI is rendered and where logic executes.
    #[serde(default)]
    pub runtime: Option<RuntimeConfig>,
    /// Whether the plugin is enabled by default.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Match mode configuration.
    #[serde(default)]
    pub match_mode: Option<MatchModeConfig>,
    /// Priority for sorting results.
    #[serde(default)]
    pub priority: Option<i32>,
    /// Commands this plugin provides (declared in manifest for indexing).
    #[serde(default)]
    pub commands: Vec<ManifestCommand>,
}

/// Match mode declared in the manifest.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MatchModeConfig {
    FuzzyCommand,
    AlwaysEvaluate,
    Prefix { prefix: String },
    Fallback { min_query_len: usize },
}

/// A command declared in the plugin manifest.
#[derive(Debug, Clone, Deserialize)]
pub struct ManifestCommand {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub has_submenu: bool,
}

/// Runtime configuration in the manifest.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RuntimeConfig {
    /// Compiled Rust plugin (default).
    Native,
    /// Crepuscularity template plugin with hot-reload.
    Crepuscularity {
        /// Directory containing .crepus template files, relative to plugin dir.
        #[serde(default = "default_templates")]
        template_dir: String,
    },
    /// Web plugin running JS/TS in the embedded browser.
    Web {
        /// Entry point: `index.html` or `main.js`, relative to plugin dir.
        entry: String,
    },
}

fn default_templates() -> String {
    "templates".to_string()
}

fn default_version() -> String {
    "0.1.0".to_string()
}

fn default_kind() -> String {
    "wasm".to_string()
}

fn default_true() -> bool {
    true
}

/// Scan a directory for plugin manifests.
///
/// Looks for `<dir>/<plugin_id>/plugin.toml` files and parses them.
/// Returns a list of `(manifest, plugin_dir)` tuples.
pub fn scan_plugin_manifests(dir: &Path) -> Vec<(PluginManifest, PathBuf)> {
    let mut results = Vec::new();

    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => {
            log::debug!("Could not read plugin directory {}: {}", dir.display(), e);
            return results;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let manifest_path = path.join("plugin.toml");
        if !manifest_path.exists() {
            continue;
        }

        match load_manifest(&manifest_path) {
            Ok(manifest) => {
                log::info!(
                    "Loaded plugin manifest: {} ({}) from {}",
                    manifest.name,
                    manifest.id,
                    manifest_path.display()
                );
                results.push((manifest, path));
            }
            Err(e) => {
                log::warn!(
                    "Failed to load plugin manifest {}: {}",
                    manifest_path.display(),
                    e
                );
            }
        }
    }

    results
}

/// Load and parse a single `plugin.toml` file.
pub fn load_manifest(path: &Path) -> Result<PluginManifest, String> {
    let contents = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

    toml::from_str(&contents).map_err(|e| format!("Failed to parse {}: {}", path.display(), e))
}

/// Default plugin directory: `~/.config/rover/plugins/`.
pub fn default_plugin_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("rover")
        .join("plugins")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_manifest() {
        let toml_str = r#"
            id = "ext.hello"
            name = "Hello World"
        "#;
        let manifest: PluginManifest = toml::from_str(toml_str).unwrap();
        assert_eq!(manifest.id, "ext.hello");
        assert_eq!(manifest.name, "Hello World");
        assert_eq!(manifest.version, "0.1.0");
        assert!(manifest.enabled);
        assert_eq!(manifest.kind, "wasm");
    }

    #[test]
    fn parse_full_manifest() {
        let toml_str = r#"
            id = "ext.weather"
            name = "Weather"
            description = "Show current weather"
            version = "1.2.0"
            author = "someone"
            kind = "native"
            binary = "weather.dylib"
            enabled = true
            priority = 15

            [match_mode]
            type = "prefix"
            prefix = "w:"

            [[commands]]
            id = "current_weather"
            name = "Current Weather"
            description = "Show weather for your location"
            keywords = ["weather", "forecast", "temperature"]
        "#;
        let manifest: PluginManifest = toml::from_str(toml_str).unwrap();
        assert_eq!(manifest.id, "ext.weather");
        assert_eq!(manifest.version, "1.2.0");
        assert_eq!(manifest.kind, "native");
        assert!(matches!(
            manifest.match_mode,
            Some(MatchModeConfig::Prefix { ref prefix }) if prefix == "w:"
        ));
        assert_eq!(manifest.commands.len(), 1);
        assert_eq!(manifest.commands[0].id, "current_weather");
    }

    #[test]
    fn scan_empty_dir() {
        let dir = std::env::temp_dir().join("rover_test_empty_plugins");
        let _ = std::fs::create_dir_all(&dir);
        let manifests = scan_plugin_manifests(&dir);
        // May or may not be empty depending on what's in temp, but shouldn't panic
        let _ = manifests;
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn parse_crepuscularity_runtime() {
        let toml_str = r#"
            id = "ext.theme_picker"
            name = "Theme Picker"

            [runtime]
            type = "crepuscularity"
            template_dir = "ui"
        "#;
        let manifest: PluginManifest = toml::from_str(toml_str).unwrap();
        match manifest.runtime {
            Some(RuntimeConfig::Crepuscularity { template_dir }) => {
                assert_eq!(template_dir, "ui");
            }
            other => panic!("Expected Crepuscularity runtime, got {:?}", other),
        }
    }

    #[test]
    fn parse_web_runtime() {
        let toml_str = r#"
            id = "ext.web_notes"
            name = "Web Notes"

            [runtime]
            type = "web"
            entry = "index.html"
        "#;
        let manifest: PluginManifest = toml::from_str(toml_str).unwrap();
        match manifest.runtime {
            Some(RuntimeConfig::Web { entry }) => {
                assert_eq!(entry, "index.html");
            }
            other => panic!("Expected Web runtime, got {:?}", other),
        }
    }

    #[test]
    fn parse_native_runtime() {
        let toml_str = r#"
            id = "ext.native"
            name = "Native Plugin"

            [runtime]
            type = "native"
        "#;
        let manifest: PluginManifest = toml::from_str(toml_str).unwrap();
        assert!(matches!(manifest.runtime, Some(RuntimeConfig::Native)));
    }

    #[test]
    fn parse_no_runtime_defaults_to_none() {
        let toml_str = r#"
            id = "ext.legacy"
            name = "Legacy Plugin"
        "#;
        let manifest: PluginManifest = toml::from_str(toml_str).unwrap();
        assert!(manifest.runtime.is_none());
    }

    #[test]
    fn scan_with_manifest() {
        let dir = std::env::temp_dir().join("rover_test_scan_plugins");
        let plugin_dir = dir.join("ext.test_plugin");
        let _ = std::fs::create_dir_all(&plugin_dir);

        let manifest_content = r#"
            id = "ext.test_plugin"
            name = "Test Plugin"
            description = "A test plugin"
        "#;
        std::fs::write(plugin_dir.join("plugin.toml"), manifest_content).unwrap();

        let manifests = scan_plugin_manifests(&dir);
        assert_eq!(manifests.len(), 1);
        assert_eq!(manifests[0].0.id, "ext.test_plugin");
        assert_eq!(manifests[0].0.name, "Test Plugin");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
