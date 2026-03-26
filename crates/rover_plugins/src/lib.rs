//! # Rover Plugins
//!
//! This crate contains the `PluginRegistry` and all built-in plugin
//! implementations. Each built-in feature (calculator, app launcher,
//! clipboard, file search, AI, terminal, browser, tile, editor) is a plugin
//! that implements the `rover_plugin_api::Plugin` trait.
//!
//! External plugins are loaded from `~/.config/rover/plugins/*/plugin.toml`
//! via the `manifest` module.

pub mod builtin;
pub mod manifest;
mod registry;

pub use manifest::{default_plugin_dir, scan_plugin_manifests, PluginManifest};
pub use registry::PluginRegistry;
