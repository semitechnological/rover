//! # Rover Plugin API
//!
//! This crate defines the `Plugin` trait and all supporting types that Rover
//! plugins implement. Built-in plugins, external TOML-manifest plugins,
//! crepuscularity template plugins, and web/JS/TS plugins all use this API.
//!
//! ## Plugin Runtimes
//!
//! Plugins declare a runtime that determines how their UI is rendered and
//! where their logic executes:
//!
//! - **Native** — Rust code compiled into the binary. UI via raw GPUI.
//! - **Crepuscularity** — `.crepus` template files (Tailwind-like DSL) rendered
//!   via the crepuscularity runtime. Logic in Rust. Supports hot-reload.
//! - **Web** — HTML/CSS/JS running inside the embedded WKWebView browser.
//!   Logic in JavaScript or TypeScript. Communicates via `window.rover.*` bridge.

use std::any::Any;
use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

// ─── Plugin Runtime ──────────────────────────────────────────────────────────

/// The runtime environment a plugin uses for rendering and execution.
#[derive(Debug, Clone, PartialEq)]
pub enum PluginRuntime {
    /// Compiled Rust plugin. UI rendered via raw GPUI builder API.
    Native,

    /// Crepuscularity template plugin. UI defined in `.crepus` files and
    /// rendered via the crepuscularity-runtime crate. Supports hot-reload.
    Crepuscularity {
        /// Path to the plugin's `.crepus` template directory.
        /// Relative paths are resolved from the plugin's data_dir.
        template_dir: PathBuf,
    },

    /// Web plugin. UI is HTML/CSS/JS loaded in the embedded WKWebView.
    /// Plugin logic runs in JavaScript and communicates via `window.rover.*`.
    Web {
        /// Entry point: path to `index.html` or `main.js`.
        entry: PathBuf,
    },
}

impl Default for PluginRuntime {
    fn default() -> Self {
        PluginRuntime::Native
    }
}

// ─── Render Output ───────────────────────────────────────────────────────────

/// How a plugin wants to render a match row or inline content.
/// The picker dispatcher uses this to select the appropriate renderer.
#[derive(Debug, Clone)]
pub enum RenderOutput {
    /// Use the default picker rendering (title + subtitle + icon).
    Default,

    /// Render via a crepuscularity template with context variables.
    CrepuscularityTemplate {
        /// Path to the `.crepus` component file.
        template: PathBuf,
        /// Context variables passed to the template.
        vars: HashMap<String, String>,
    },

    /// Render HTML content inside a web view panel.
    Html {
        /// HTML string to render.
        html: String,
    },
}

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

    /// The runtime this plugin uses for rendering and execution.
    fn runtime(&self) -> PluginRuntime { PluginRuntime::Native }

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

    /// Return a rich render output for a match row. Used by crepuscularity
    /// and web plugins to provide template-based or HTML rendering.
    /// Falls back to `render_match()` when this returns `RenderOutput::Default`.
    fn render_output(&self, _match_id: &str, _selected: bool) -> RenderOutput {
        RenderOutput::Default
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

    // ── Downcasting ─────────────────────────────────────────────────────

    /// Downcast to concrete type for plugin-specific API access.
    fn as_any(&self) -> &dyn Any;

    /// Downcast to concrete type (mutable) for plugin-specific API access.
    fn as_any_mut(&mut self) -> &mut dyn Any;
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
    /// Optional rendering hint for this specific result.
    pub render_hint: Option<RenderHint>,
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
    /// Quit/force-quit a macOS application by name.
    QuitApp(String),
    /// Custom result for internal use.
    Custom(Box<dyn Any + Send>),
}

// ─── Context Item ────────────────────────────────────────────────────────────

/// A context item rendered as a pill at the top of the picker.
#[derive(Debug, Clone)]
pub struct ContextItem {
    pub id: String,
    pub label: String,
    pub icon: Option<IconHint>,
    pub is_active: bool,
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
    /// Show a colored state pill next to the title (e.g. "URL", "SHELL", "AI").
    StatePill { label: String, color: String },
    /// Show inline action buttons alongside the match row.
    WithActions { actions: Vec<ActionDescriptor> },
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
    /// Name of the frontmost macOS app (before Rover activated).
    pub frontmost_app: Option<String>,
    /// Whether a terminal window was focused before Rover activated.
    pub terminal_was_focused: bool,
    /// Opaque handles for heavyweight services (AI session, live tabs, etc.)
    /// keyed by convention: `"ai_session"`, `"live_tabs"`, `"semantic_index"`.
    pub extras: HashMap<String, Arc<dyn Any + Send + Sync>>,
}

impl PluginContext {
    pub fn new(config: PluginConfig) -> Self {
        Self {
            db: None,
            config,
            frontmost_app: None,
            terminal_was_focused: false,
            extras: HashMap::new(),
        }
    }

    pub fn with_storage(mut self, db: Arc<dyn PluginStorage>) -> Self {
        self.db = Some(db);
        self
    }

    /// Get a typed extra by key, returning None if missing or wrong type.
    pub fn extra<T: 'static + Send + Sync>(&self, key: &str) -> Option<Arc<T>> {
        self.extras
            .get(key)
            .and_then(|v| v.clone().downcast::<T>().ok())
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

// ─── Web Plugin Message Protocol ─────────────────────────────────────────────

/// Messages sent from the web plugin (JS) to Rover via WKScriptMessageHandler.
#[derive(Debug, Clone)]
pub enum WebPluginMessage {
    /// Plugin is ready and has loaded.
    Ready { plugin_id: String },
    /// Plugin wants to add match results to the picker.
    AddResults { results: Vec<MatchResult> },
    /// Plugin wants to clear its previous results.
    ClearResults,
    /// Plugin wants to copy text to clipboard.
    CopyToClipboard(String),
    /// Plugin wants to open a URL.
    OpenUrl(String),
    /// Plugin wants to run a shell command.
    RunShell(String),
    /// Plugin wants to dismiss the picker.
    Dismiss,
    /// Plugin wants to store a value.
    StorageSet { key: String, value: String },
    /// Plugin wants to retrieve a value.
    StorageGet { key: String },
}

/// Messages sent from Rover to the web plugin (JS) via evaluateJavaScript.
#[derive(Debug, Clone)]
pub enum WebPluginEvent {
    /// User typed a query.
    Query { text: String },
    /// User confirmed a match from this plugin.
    Confirm { match_id: String, secondary: bool },
    /// Response to a StorageGet request.
    StorageValue { key: String, value: Option<String> },
    /// Plugin configuration/settings.
    Config { data: HashMap<String, String> },
}

// ─── Manifest Runtime Config ─────────────────────────────────────────────────

/// Runtime-specific configuration from the plugin manifest (plugin.toml).
#[derive(Debug, Clone)]
pub enum ManifestRuntime {
    /// `runtime = "native"` — compiled Rust plugin.
    Native,
    /// `runtime = "crepuscularity"` with template_dir.
    Crepuscularity { template_dir: String },
    /// `runtime = "web"` with entry point.
    Web { entry: String },
}
