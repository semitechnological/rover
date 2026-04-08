//! # Rover Plugin API
//!
//! This crate defines the `Plugin` trait and all supporting types that Rover
//! plugins implement. Both built-in (compiled-in) plugins and future external
//! (WASM/dylib) plugins use this API.

use std::any::Any;
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

// ─── Plugin Trait ────────────────────────────────────────────────────────────

/// A plugin provides commands, match results, and lifecycle hooks to Rover.
///
/// Every feature in Rover (calculator, app launcher, clipboard, AI, browser,
/// terminal, file search, etc.) implements this trait.
pub trait Plugin: Send + Sync {
    // ── Metadata ─────────────────────────────────────────────────────────

    /// Unique identifier, e.g. `"builtin.calculator"` or `"ext.my_plugin"`.
    fn id(&self) -> &str;

    /// Human-readable name shown in settings/about.
    fn name(&self) -> &str;

    /// Short description of what this plugin does.
    fn description(&self) -> &str;

    /// Semver version string.
    fn version(&self) -> &str { "0.1.0" }

    // ── Lifecycle ────────────────────────────────────────────────────────

    /// Called once when the plugin is loaded. Use `ctx` to access storage
    /// and configuration.
    fn on_load(&mut self, ctx: &mut PluginContext) -> Result<(), PluginError>;

    /// Called when the plugin is about to be unloaded (app quit or disable).
    fn on_unload(&mut self) {}

    // ── Commands ─────────────────────────────────────────────────────────

    /// Commands this plugin contributes to the command palette.
    fn commands(&self) -> Vec<CommandDescriptor> { vec![] }

    // ── Query & matching ─────────────────────────────────────────────────

    /// Called on every keystroke. Return match results for the current query.
    fn on_query(&mut self, query: &str, ctx: &PluginContext) -> Vec<MatchResult>;

    /// Optional hook to intercept query updates before normal matching.
    /// Returning `Some` allows a plugin to modify the search bar behavior.
    fn on_query_input(&mut self, _query: &str, _ctx: &PluginContext) -> Option<SearchBarUpdate> {
        None
    }

    /// Called when the user confirms (presses Enter on) a match from this
    /// plugin.
    fn on_confirm(
        &mut self,
        match_id: &str,
        query: &str,
        secondary: bool,
        ctx: &mut PluginContext,
    ) -> ConfirmResult;

    /// Sorting priority. Higher = results shown first. Default 0.
    fn priority(&self) -> i32 { 0 }

    /// How this plugin's queries are matched against user input.
    fn match_mode(&self) -> MatchMode { MatchMode::FuzzyCommand }

    // ── Rendering hints ──────────────────────────────────────────────────

    /// Provide a rendering hint for a specific match row.
    fn render_match(&self, _match_id: &str, _selected: bool) -> RenderHint {
        RenderHint::Default
    }

    // ── Submenu support ──────────────────────────────────────────────────

    /// Whether this plugin provides a submenu (e.g. clipboard history list).
    fn provides_submenu(&self) -> bool { false }

    /// Items to show in the submenu. Called when the submenu is active and
    /// the user types a filter query.
    fn submenu_items(&mut self, _query: &str, _ctx: &PluginContext) -> Vec<MatchResult> {
        vec![]
    }

    /// Called when a submenu item is confirmed.
    fn on_submenu_confirm(
        &mut self,
        _match_id: &str,
        _query: &str,
        _ctx: &mut PluginContext,
    ) -> ConfirmResult {
        ConfirmResult::Dismiss
    }

    // ── Context items (pills) ────────────────────────────────────────────

    /// Context items rendered as pills at the top of the picker (e.g.
    /// browser tabs).
    fn context_items(&self, _ctx: &PluginContext) -> Vec<ContextItem> { vec![] }

    // ── Inline content ───────────────────────────────────────────────────

    /// Inline content shown as a card (e.g. AI response, terminal output).
    fn inline_content(&self, _ctx: &PluginContext) -> Option<InlineContent> { None }

    // ── Window support ───────────────────────────────────────────────────

    /// Request to open a separate window (browser, editor, settings, etc.).
    fn open_window(&mut self, _ctx: &mut PluginContext) -> Option<WindowRequest> { None }

    // ── Actions ──────────────────────────────────────────────────────────

    /// Sub-actions available for a given match (shown on Cmd+K).
    fn actions_for_match(&self, _match_id: &str) -> Vec<ActionDescriptor> { vec![] }

    /// Execute a sub-action.
    fn execute_action(
        &mut self,
        _match_id: &str,
        _action_id: &str,
        _ctx: &mut PluginContext,
    ) -> ConfirmResult {
        ConfirmResult::Stay
    }
}

// ─── Match Mode ──────────────────────────────────────────────────────────────

/// Determines when a plugin's `on_query` is called.
#[derive(Debug, Clone, PartialEq)]
pub enum MatchMode {
    /// Matched via fuzzy search against command names. `on_query` is called
    /// only when there is a fuzzy match.
    FuzzyCommand,

    /// Always evaluated on every query (e.g. calculator).
    AlwaysEvaluate,

    /// Only when the query starts with a specific prefix (e.g. `">"` for
    /// terminal, `"?"` for AI).
    Prefix(String),

    /// Fallback: evaluated when no other plugin produced good matches.
    Fallback { min_query_len: usize },
}

// ─── Command Descriptor ──────────────────────────────────────────────────────

/// Describes a command that a plugin contributes to the palette.
#[derive(Debug, Clone)]
pub struct CommandDescriptor {
    /// Unique ID within the plugin, e.g. `"clipboard_history"`.
    pub id: String,
    /// Display name shown in the picker.
    pub name: String,
    /// Short description.
    pub description: String,
    /// Optional icon hint.
    pub icon: Option<IconHint>,
    /// Additional keywords for fuzzy matching.
    pub keywords: Vec<String>,
    /// Whether confirming this command enters a submenu.
    pub has_submenu: bool,
}

// ─── Match Result ────────────────────────────────────────────────────────────

/// A single match result returned by a plugin.
#[derive(Debug, Clone)]
pub struct MatchResult {
    /// Unique ID for this match within the plugin.
    pub id: String,
    /// Display title.
    pub title: String,
    /// Optional subtitle (shown below or next to title).
    pub subtitle: Option<String>,
    /// Optional icon hint.
    pub icon: Option<IconHint>,
    /// Score for sorting (higher = better match). Typically 0.0 .. 1.0.
    pub score: f64,
    /// The plugin that produced this result.
    pub plugin_id: String,
}

// ─── Confirm Result ──────────────────────────────────────────────────────────

/// What happens after the user confirms a match.
pub enum ConfirmResult {
    /// Close the picker.
    Dismiss,
    /// Keep the picker open (e.g. submenu navigation).
    Stay,
    /// Enter a submenu provided by the plugin.
    EnterSubmenu { title: String },
    /// Open a URL in the browser.
    OpenUrl(String),
    /// Run a shell command.
    RunShell(String),
    /// Launch a macOS application by name.
    LaunchApp(String),
    /// Open a separate window.
    OpenWindow(WindowRequest),
    /// Copy text to clipboard and dismiss.
    CopyToClipboard(String),
    /// Open a terminal with optional initial command.
    OpenTerminal(Option<String>),
    /// Open the settings window.
    OpenSettings,
    /// Custom result for internal use.
    Custom(Box<dyn Any + Send>),
}

// ─── Context Item ────────────────────────────────────────────────────────────

/// A context item rendered as a pill at the top of the picker.
#[derive(Debug, Clone)]
pub struct ContextItem {
    /// Plugin that owns this context item.
    pub plugin_id: String,
    pub id: String,
    pub label: String,
    pub icon: Option<IconHint>,
    pub is_active: bool,
}

/// Search bar behavior update requested by a plugin.
#[derive(Debug, Clone)]
pub enum SearchBarUpdate {
    /// Replace current query text.
    ReplaceQuery(String),
    /// Set a dynamic placeholder for current mode.
    SetPlaceholder(String),
}

// ─── Inline Content ──────────────────────────────────────────────────────────

/// Content displayed inline in the picker (cards, widgets).
#[derive(Debug, Clone)]
pub enum InlineContent {
    /// Simple text block.
    Text(String),
    /// Loading indicator with a message.
    Loading(String),
    /// Terminal output (monospace, dark bg).
    TerminalOutput { output: String, running: bool },
}

// ─── Icon Hint ───────────────────────────────────────────────────────────────

/// Describes how to render an icon. The picker resolves these to actual icons.
#[derive(Debug, Clone)]
pub enum IconHint {
    /// macOS app name — resolve via NSWorkspace.
    AppIcon(String),
    /// SF Symbol name.
    SystemSymbol(String),
    /// Emoji character.
    Emoji(String),
    /// Path to an icon file.
    Path(String),
}

// ─── Window Request ──────────────────────────────────────────────────────────

/// Request to open a window.
#[derive(Debug, Clone)]
pub enum WindowRequest {
    Browser { url: Option<String> },
    Terminal { cmd: Option<String> },
    Editor { path: Option<String> },
    Settings,
    Custom { title: String, width: u32, height: u32 },
}

// ─── Render Hint ─────────────────────────────────────────────────────────────

/// Hint for the picker on how to render a match row.
#[derive(Debug, Clone)]
pub enum RenderHint {
    /// Use default rendering (title + subtitle).
    Default,
    /// Show a subtitle.
    WithSubtitle { subtitle: String },
    /// Show a preview text.
    WithPreview { preview: String },
}

// ─── Action Descriptor ───────────────────────────────────────────────────────

/// A sub-action for a match (shown in the Cmd+K overlay).
#[derive(Debug, Clone)]
pub struct ActionDescriptor {
    pub id: String,
    pub name: String,
    pub shortcut: Option<String>,
}

// ─── Plugin Context ──────────────────────────────────────────────────────────

/// Context passed to plugins for interacting with Rover's services.
pub struct PluginContext {
    /// Key-value storage for plugin state.
    pub db: Option<Arc<dyn PluginStorage>>,
    /// Plugin-specific configuration.
    pub config: PluginConfig,
}

impl PluginContext {
    pub fn new(config: PluginConfig) -> Self {
        Self { db: None, config }
    }

    pub fn with_storage(mut self, db: Arc<dyn PluginStorage>) -> Self {
        self.db = Some(db);
        self
    }
}

// ─── Plugin Storage ──────────────────────────────────────────────────────────

/// Simple key-value storage backend for plugins.
pub trait PluginStorage: Send + Sync {
    fn get(&self, key: &str) -> Option<String>;
    fn set(&self, key: &str, value: &str);
    fn delete(&self, key: &str);
    fn list_keys(&self, prefix: &str) -> Vec<String>;
}

// ─── Plugin Config ───────────────────────────────────────────────────────────

/// Configuration for a single plugin instance.
#[derive(Debug, Clone)]
pub struct PluginConfig {
    /// Per-plugin data directory, e.g. `~/.config/rover/plugins/<plugin_id>/`.
    pub data_dir: PathBuf,
    /// Whether this plugin is enabled.
    pub enabled: bool,
}

impl Default for PluginConfig {
    fn default() -> Self {
        Self {
            data_dir: dirs::config_dir()
                .unwrap_or_else(|| PathBuf::from("/tmp"))
                .join("rover")
                .join("plugins")
                .join("default"),
            enabled: true,
        }
    }
}

// ─── Plugin Error ────────────────────────────────────────────────────────────

/// Errors returned by plugin operations.
#[derive(Debug)]
pub enum PluginError {
    /// Plugin failed to initialize.
    InitFailed(String),
    /// Plugin is not available on this platform/configuration.
    NotAvailable(String),
    /// Generic error.
    Other(String),
}

impl fmt::Display for PluginError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PluginError::InitFailed(msg) => write!(f, "plugin init failed: {msg}"),
            PluginError::NotAvailable(msg) => write!(f, "plugin not available: {msg}"),
            PluginError::Other(msg) => write!(f, "plugin error: {msg}"),
        }
    }
}

impl std::error::Error for PluginError {}
