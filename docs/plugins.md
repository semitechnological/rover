# Rover Plugin Architecture

This document describes the Rover plugin system: how plugins work, how to create built-in and external plugins, and the manifest format.

## Overview

Every feature in Rover (calculator, app launcher, clipboard history, AI, browser, terminal, file search, emoji, web search, Google integrations, macOS tools, window tiling, editor) is implemented as a **plugin**. Plugins implement the `rover_plugin_api::Plugin` trait and are managed by the `PluginRegistry` in the `rover_plugins` crate.

The architecture has three layers:

1. **`rover_plugin_api`** -- Defines the `Plugin` trait and all supporting types. No dependencies on GPUI or Rover internals.
2. **`rover_plugins`** -- Contains the `PluginRegistry` and all built-in plugin implementations. Each built-in plugin is gated behind a Cargo feature flag.
3. **`delegate_plugin.rs`** (in the `rover` binary) -- A thin adapter between the `PickerDelegate` trait (GPUI UI) and the plugin system.

## The Plugin Trait

```rust
pub trait Plugin: Send + Sync {
    // -- Metadata --
    fn id(&self) -> &str;           // e.g. "builtin.calculator"
    fn name(&self) -> &str;         // "Calculator"
    fn description(&self) -> &str;  // "Evaluate math expressions"
    fn version(&self) -> &str;      // "0.1.0" (default)

    // -- Lifecycle --
    fn on_load(&mut self, ctx: &mut PluginContext) -> Result<(), PluginError>;
    fn on_unload(&mut self) {}

    // -- Commands --
    fn commands(&self) -> Vec<CommandDescriptor> { vec![] }

    // -- Query & Matching --
    fn on_query(&mut self, query: &str, ctx: &PluginContext) -> Vec<MatchResult>;
    fn on_confirm(&mut self, match_id: &str, query: &str, secondary: bool, ctx: &mut PluginContext) -> ConfirmResult;
    fn priority(&self) -> i32 { 0 }
    fn match_mode(&self) -> MatchMode { MatchMode::FuzzyCommand }

    // -- Rendering --
    fn render_match(&self, match_id: &str, selected: bool) -> RenderHint { RenderHint::Default }

    // -- Submenu --
    fn provides_submenu(&self) -> bool { false }
    fn submenu_items(&mut self, query: &str, ctx: &PluginContext) -> Vec<MatchResult> { vec![] }
    fn on_submenu_confirm(&mut self, match_id: &str, query: &str, ctx: &mut PluginContext) -> ConfirmResult { ConfirmResult::Dismiss }

    // -- Context items (pills) --
    fn context_items(&self, ctx: &PluginContext) -> Vec<ContextItem> { vec![] }

    // -- Inline content (cards) --
    fn inline_content(&self, ctx: &PluginContext) -> Option<InlineContent> { None }

    // -- Window support --
    fn open_window(&mut self, ctx: &mut PluginContext) -> Option<WindowRequest> { None }

    // -- Sub-actions (Cmd+K overlay) --
    fn actions_for_match(&self, match_id: &str) -> Vec<ActionDescriptor> { vec![] }
    fn execute_action(&mut self, match_id: &str, action_id: &str, ctx: &mut PluginContext) -> ConfirmResult { ConfirmResult::Stay }
}
```

## Match Modes

Each plugin declares how it should be queried:

| Mode | Behaviour |
|------|-----------|
| `FuzzyCommand` | Commands are registered via `commands()` and matched by the central fuzzy matcher. `on_query` is NOT called by the registry directly. |
| `AlwaysEvaluate` | `on_query` is called on every keystroke regardless of other matches. Used by the calculator. |
| `Prefix(String)` | `on_query` is called only when the query starts with the given prefix. Used by terminal (`">"`) and explicit AI (`"?"`). |
| `Fallback { min_query_len }` | `on_query` is called only when the query is at least N chars long AND no other plugin produced good matches. Used by AI. |

## Plugin Lifecycle

1. The `PluginRegistry` is created.
2. Built-in plugins are registered with `registry.register(Box::new(MyPlugin::new()))`.
3. External plugins are scanned from `~/.config/rover/plugins/` via `registry.load_external(dir)`.
4. `registry.init_all(&mut ctx)` calls `on_load()` on each enabled plugin. If `on_load` returns an error, the plugin is automatically disabled.
5. On each query, the registry evaluates plugins based on their `match_mode`.
6. On app quit, `registry.unload_all()` calls `on_unload()` on every plugin.

## ConfirmResult

When a user confirms a match, the plugin returns a `ConfirmResult`:

| Variant | Effect |
|---------|--------|
| `Dismiss` | Close the picker |
| `Stay` | Keep the picker open |
| `EnterSubmenu { title }` | Enter a submenu provided by the plugin |
| `OpenUrl(String)` | Open a URL in the browser |
| `RunShell(String)` | Run a shell command via `sh -c` |
| `LaunchApp(String)` | Activate a macOS app via AppleScript |
| `OpenWindow(WindowRequest)` | Open a separate window (browser, terminal, editor, settings) |
| `CopyToClipboard(String)` | Copy text to clipboard and dismiss |
| `OpenTerminal(Option<String>)` | Open a terminal window with optional command |
| `OpenSettings` | Open the settings window |
| `Custom(Box<dyn Any + Send>)` | Custom result for internal use |

## Built-in Plugins

| Plugin | ID | Match Mode | Feature Flag |
|--------|----|-----------|--------------|
| Calculator | `builtin.calculator` | AlwaysEvaluate | `calculator` |
| App Launcher | `builtin.app_launcher` | FuzzyCommand | `app-launcher` |
| Clipboard History | `builtin.clipboard` | FuzzyCommand | `clipboard` |
| File Search | `builtin.file_search` | FuzzyCommand | `file-search` |
| AI Assistant | `builtin.ai` | Fallback(6) | `ai` |
| Terminal | `builtin.terminal` | Prefix(">") | `terminal` |
| Browser | `builtin.browser` | FuzzyCommand | `browser` |
| Emoji | `builtin.emoji` | FuzzyCommand / `:` prefix in SwiftUI | `emoji` |
| Web Search | `builtin.web_search` | Prefix search shortcuts | `web-search` |
| Google Integrations | `builtin.google` | Prefix("?") | `google` |
| macOS Tools | `builtin.macos_tools` | Prefix("/") | `macos-tools` |
| Window Tiling | `builtin.tile` | FuzzyCommand | `tile` |
| Editor | `builtin.editor` | FuzzyCommand | `editor` |

### Feature Flags

Each built-in plugin is gated on a Cargo feature flag. The rover binary's `Cargo.toml` declares:

```toml
[features]
default = ["calculator", "app-launcher", "clipboard", "file-search", "ai", "terminal", "browser", "emoji", "macos-tools", "web-search", "google", "tile", "editor"]
calculator = ["rover_plugins/calculator"]
app-launcher = ["rover_plugins/app-launcher"]
clipboard = ["rover_plugins/clipboard"]
file-search = ["rover_plugins/file-search"]
ai = ["rover_plugins/ai", "dep:rusty_foundationmodels"]
terminal = ["rover_plugins/terminal"]
browser = ["rover_plugins/browser", "dep:rover_browser"]
emoji = ["rover_plugins/emoji"]
macos-tools = ["rover_plugins/macos-tools"]
web-search = ["rover_plugins/web-search"]
google = ["rover_plugins/google"]
tile = ["rover_plugins/tile"]
editor = ["rover_plugins/editor"]
```

To build without AI:

```sh
cargo build -p rover --no-default-features --features calculator,app-launcher,clipboard,file-search,terminal,browser,tile,editor
```

## Creating a Built-in Plugin

1. Create a new file in `crates/rover_plugins/src/builtin/`, e.g. `my_feature.rs`.

2. Implement the `Plugin` trait:

```rust
use rover_plugin_api::*;

pub struct MyPlugin {
    // internal state
}

impl MyPlugin {
    pub fn new() -> Self {
        Self { /* ... */ }
    }
}

impl Plugin for MyPlugin {
    fn id(&self) -> &str { "builtin.my_feature" }
    fn name(&self) -> &str { "My Feature" }
    fn description(&self) -> &str { "Does something useful" }

    fn on_load(&mut self, _ctx: &mut PluginContext) -> Result<(), PluginError> {
        // Initialize resources, scan files, etc.
        Ok(())
    }

    fn match_mode(&self) -> MatchMode {
        MatchMode::FuzzyCommand
    }

    fn commands(&self) -> Vec<CommandDescriptor> {
        vec![CommandDescriptor {
            id: "my_command".to_string(),
            name: "My Command".to_string(),
            description: "Does something".to_string(),
            icon: Some(IconHint::Emoji("star".to_string())),
            keywords: vec!["my".to_string(), "feature".to_string()],
            has_submenu: false,
        }]
    }

    fn on_query(&mut self, _query: &str, _ctx: &PluginContext) -> Vec<MatchResult> {
        // FuzzyCommand plugins typically return empty here.
        vec![]
    }

    fn on_confirm(
        &mut self,
        match_id: &str,
        _query: &str,
        _secondary: bool,
        _ctx: &mut PluginContext,
    ) -> ConfirmResult {
        if match_id == "my_command" {
            // Do something
            ConfirmResult::Dismiss
        } else {
            ConfirmResult::Stay
        }
    }
}
```

3. Add it to `crates/rover_plugins/src/builtin/mod.rs`:

```rust
#[cfg(feature = "my-feature")]
pub mod my_feature;
```

4. Add the feature flag to both `crates/rover_plugins/Cargo.toml` and the root `Cargo.toml`.

5. Register the plugin in the delegate or main initialization code.

## External Plugins

External plugins live in `~/.config/rover/plugins/<plugin_id>/`. Each plugin directory contains a `plugin.toml` manifest.

### Plugin Manifest Format

```toml
# Required
id = "ext.my_plugin"
name = "My Plugin"

# Optional
description = "What this plugin does"
version = "1.0.0"
author = "Your Name"
kind = "wasm"          # "wasm" or "native" (future)
binary = "plugin.wasm" # relative path to the plugin binary
enabled = true
priority = 10

# Match mode (optional, defaults to FuzzyCommand)
[match_mode]
type = "fuzzy_command"
# OR
# type = "always_evaluate"
# OR
# type = "prefix"
# prefix = "w:"
# OR
# type = "fallback"
# min_query_len = 4

# Commands declared in the manifest (for indexing)
[[commands]]
id = "weather_now"
name = "Current Weather"
description = "Show weather for your location"
keywords = ["weather", "forecast"]
has_submenu = false
```

### Loading External Plugins

The `PluginRegistry::load_external(dir)` method scans the plugin directory for manifests. Currently it indexes manifests and registers their enabled state. Actual WASM/dylib loading is planned for a future release.

To manually scan:

```rust
use rover_plugins::{PluginRegistry, default_plugin_dir, scan_plugin_manifests};

let mut registry = PluginRegistry::new();
let _ = registry.load_external(&default_plugin_dir());
```

## Plugin Runtimes

Plugins declare a **runtime** that determines how their UI is rendered and where their logic executes. There are three runtimes:

### Native (Rust/GPUI)

Default runtime. Plugin code is compiled Rust. UI rendered via raw GPUI builder API or the `view!` macro. This is what all built-in plugins use.

```toml
[runtime]
type = "native"
```

### Crepuscularity

UI defined in `.crepus` template files (Tailwind-like DSL) and rendered via the crepuscularity-runtime crate. Logic in Rust. Supports **hot-reload** — edit a `.crepus` file and the UI updates without recompiling.

```toml
[runtime]
type = "crepuscularity"
template_dir = "templates"   # relative to plugin directory
```

Template files use the crepuscularity DSL:

```
# templates/row.crepus
$: default selected = false
div flex items-center gap-2 px-3 py-2
    if {selected}
        div bg-blue-500 rounded-full w-2 h-2
    span text-sm text-white
        "{title}"
    span text-xs text-zinc-400
        "{description}"
```

Plugins return `RenderOutput::CrepuscularityTemplate` from `render_output()` to provide template path and context variables.

### Web (JS/TS/HTML)

Plugin UI is HTML/CSS/JS loaded in the embedded WKWebView browser. Logic runs in JavaScript or TypeScript. Communicates with Rover via a `window.rover.*` bridge API.

```toml
[runtime]
type = "web"
entry = "index.html"   # or "main.js", relative to plugin directory
```

Web plugins receive events and send results through the bridge:

```javascript
// Listen for queries from the picker
window.rover.onQuery((query) => {
    const results = search(query);
    window.rover.addResults(results.map(r => ({
        id: r.id,
        title: r.title,
        subtitle: r.description,
        score: r.relevance,
    })));
});

// Handle user confirming a result
window.rover.onConfirm((matchId, secondary) => {
    if (secondary) {
        window.rover.copyToClipboard(getData(matchId));
    } else {
        window.rover.openUrl(getUrl(matchId));
    }
});

// Access plugin storage
const saved = await window.rover.storage.get("last_query");
await window.rover.storage.set("last_query", query);
```

### Future: WASM Plugins

External WASM plugins will implement a guest-side API that mirrors the `Plugin` trait. The host will load the `.wasm` binary via wasmtime and bridge calls through the `rover_extension_api` interface. This is tracked but not yet implemented.

## PluginContext

Plugins receive a `PluginContext` for interacting with Rover's services:

```rust
pub struct PluginContext {
    pub db: Option<Arc<dyn PluginStorage>>,
    pub config: PluginConfig,
}

pub struct PluginConfig {
    pub data_dir: PathBuf,  // ~/.config/rover/plugins/<plugin_id>/
    pub enabled: bool,
}
```

The `PluginStorage` trait provides simple key-value storage:

```rust
pub trait PluginStorage: Send + Sync {
    fn get(&self, key: &str) -> Option<String>;
    fn set(&self, key: &str, value: &str);
    fn delete(&self, key: &str);
    fn list_keys(&self, prefix: &str) -> Vec<String>;
}
```

## Testing

Run plugin tests:

```sh
# Plugin API crate (no system dependencies)
cargo test -p rover_plugin_api

# Full plugin tests (requires macOS with Xcode command line tools)
cargo test -p rover_plugins
```

The registry has comprehensive tests covering:
- Plugin registration and counting
- Enable/disable toggling
- Failed plugin initialization (auto-disable)
- Query merging from multiple plugins
- Priority ordering
- Confirm dispatch to the correct plugin
- Disabled plugins excluded from queries
- Prefix mode filtering
- FuzzyCommand mode isolation
- Feature flag gating simulation

## Search Bar + Pill Hooks (Current API)

Rover plugins can now drive top pills and search-bar behavior directly:

- `on_query_input(&mut self, query: &str, ctx: &PluginContext) -> Option<SearchBarUpdate>`
  - `SearchBarUpdate::ReplaceQuery(String)`
  - `SearchBarUpdate::SetPlaceholder(String)`
- `context_items(&self, ctx: &PluginContext) -> Vec<ContextItem>`
  - `ContextItem` includes `plugin_id`, `id`, `label`, `icon`, and `is_active`
- `crepus_surfaces(&self, ctx: &PluginContext) -> Vec<CrepusSurface>`
  - `CrepusSurfaceTarget` supports `Pills`, `Widget`, and `Screen`
  - each surface declares `template`, optional `component` (`#Name`), `props`, and `priority`

Pill confirms are dispatched back to the owning plugin by `plugin_id + id` through `on_confirm`, so plugin state and behavior stay modular.

## Crepuscularity Conventions

For Crepuscularity-based Rover plugins (components/widgets/screens):

- Use runtime templates when you need hot reload, `include`, slots, or multi-component files.
- Use single-file components for small widgets, and multi-component files with `#Name` for grouped UI sets.
- Use `$: default` (single-file) or TOML defaults (multi-component) for optional props.
- Keep plugin UI split by role (`components/`, `widgets/`, `screens/`) so modules can behave like touchbar-style surfaces.
- For browser extension plugins, use `crepuscularity-webext` capability declarations and capability scanning conventions.
