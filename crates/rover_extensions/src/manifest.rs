use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Extension manifest parsed from extension.toml.
/// Modeled after Zed's ExtensionManifest, but scoped to launcher-relevant fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionManifest {
    pub id: String,
    pub name: String,
    pub version: String,

    #[serde(default)]
    pub description: Option<String>,

    #[serde(default)]
    pub authors: Vec<String>,

    #[serde(default)]
    pub themes: Vec<PathBuf>,

    #[serde(default)]
    pub icon_themes: Vec<PathBuf>,

    #[serde(default)]
    pub slash_commands: BTreeMap<String, SlashCommandEntry>,

    #[serde(default)]
    pub capabilities: Vec<ExtensionCapability>,

    /// Path to the compiled WASM library, relative to extension root.
    #[serde(default)]
    pub lib: Option<LibEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlashCommandEntry {
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionCapability {
    /// Permission to execute processes.
    ProcessExec,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibEntry {
    pub path: Option<PathBuf>,
}
