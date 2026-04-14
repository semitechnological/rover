//! Tool implementations for AI-powered actions.
//!
//! This module provides tools that the AI can use to perform actions on the system.
//! Each tool is wrapped with permission prompts (unless Yolo Mode is enabled).

pub mod applescript;

pub use applescript::{escape_string, run as run_applescript, validate_date, AppleScriptBuilder};

// Re-export from tools.rs (these are still used by other parts of the codebase)
mod tools_defs;
pub use tools_defs::{build_os_tools, set_yolo_mode};
