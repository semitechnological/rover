//! # Rover Plugins
//!
//! This crate contains the `PluginRegistry` and all built-in plugin
//! implementations. Each built-in feature (calculator, app launcher,
//! clipboard, file search, AI, terminal, browser) is a plugin that
//! implements the `rover_plugin_api::Plugin` trait.

pub mod builtin;
mod registry;

pub use registry::PluginRegistry;
