use serde::{Deserialize, Serialize};

/// Result of running an extension command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandOutput {
    pub text: String,
}

/// Trait that Rover WASM extensions implement.
pub trait Extension: Send + Sync {
    fn new() -> Self
    where
        Self: Sized;

    /// Run a slash command registered by this extension.
    fn run_command(&mut self, command: &str, args: Vec<String>) -> Result<CommandOutput, String>;
}

/// Metadata about a command provided by an extension.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionCommandInfo {
    pub name: String,
    pub description: String,
}
