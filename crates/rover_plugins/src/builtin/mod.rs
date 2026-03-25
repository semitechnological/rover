//! Built-in plugins that ship with Rover.
//!
//! Each plugin is gated on a cargo feature flag so it can be compiled out.

#[cfg(feature = "calculator")]
pub mod calculator;

#[cfg(feature = "app-launcher")]
pub mod app_launcher;

#[cfg(feature = "clipboard")]
pub mod clipboard;

#[cfg(feature = "file-search")]
pub mod file_search;

#[cfg(feature = "ai")]
pub mod ai;

#[cfg(feature = "terminal")]
pub mod terminal;

#[cfg(feature = "browser")]
pub mod browser;
