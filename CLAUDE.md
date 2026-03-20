# Rover — Agent Instructions

## What is Rover

Rover is a macOS launcher/assistant bar built with `gpui = "0.2"` (crates.io, not Zed's internal fork). It sits in the menu bar as a system-wide pop-up panel activated by a global hotkey (Cmd+Space by default), and provides:

- **Fuzzy app/command launcher** — scans `/Applications`, `/System/Applications`, `~/Applications`
- **Calculator** — evaluates math expressions inline
- **Clipboard history** — stores and searches past clipboard entries
- **File search** — spotlight-style file finder
- **AI assistant** — Apple Intelligence via FoundationModels (macOS 26+, Apple Silicon)
- **WebKit browser** — keyboard-driven browser window (Cmd+\ to open address bar)
- **Terminal** — built-in terminal suggestion and shell command execution
- **Settings** — macOS menubar Settings/About with AI toggle, bar position, and more

## Architecture

```
rover (binary, src/)
├── main.rs            — app entry, menubar, global hotkey, window creation
├── delegate.rs        — RoverDelegate: picker logic, input priority, AI fallback
├── ai.rs              — creates Apple Intelligence session with OS tools
├── tools.rs           — all ToolDefinitions (AppleScript, shell, calendar, etc.)
├── settings_window.rs — Settings + About windows
├── hotkey.rs          — Carbon RegisterEventHotKey wrapper
└── clipboard_monitor.rs — background clipboard polling

crates/
├── rover_picker       — generic Picker<D: PickerDelegate> UI + TextInput element
├── rover_theme        — JSON theme system (flat dot-separated tokens)
├── rover_fuzzy        — fuzzy matching (fuzzy-matcher/skim algorithm)
├── rover_commands     — CommandRegistry, app launcher, calculator, file search, builtins
├── rover_db           — SQLite via rusqlite: clipboard history, query history, settings
├── rover_browser      — WKWebView-based browser window with command bar
├── rover_extensions   — extension.toml manifest loader
└── rover_extension_api — traits for future WASM extensions
```

## GPUI Notes

Rover uses `gpui = "0.2"` from crates.io. The public API differs from Zed's internal usage:

- `div().flex().flex_col()` not `v_flex()` — not exported
- `div().flex()` not `h_flex()` — not exported
- `cx.processor()` for `uniform_list` callbacks, `cx.listener()` for action callbacks
- No `menu::SelectNext` etc. — define actions with `actions!()`
- Key bindings: `cx.bind_keys([KeyBinding::new(...)])` with key context strings
- Text input: implement `EntityInputHandler` + `Focusable` + custom `Element` (see `crates/rover_picker/src/text_input.rs`)
- `uniform_list` needs a parent with `flex_grow()` to get height
- `FluentBuilder` trait (`.when()`, `.map()`) is in `gpui::prelude::*`
- No `when_some` or `hide_window` — use if-let and `minimize_window()` respectively

## Input Priority (delegate.rs)

Queries are processed in this order:

1. **Explicit terminal prefix** (`> cmd` or `! cmd`) — runs command via `sh -c`, shows shell row
2. **Calculator** — always evaluated; shown if a result exists
3. **Fuzzy match** — always run against all registered commands
4. **AI fallback** — fires automatically when query ≥ 6 chars, no good fuzzy matches, no calc result
5. **Explicit AI prefix** (`? query` or `ai query`) — forces AI regardless of fuzzy matches

## AI Integration

`src/ai.rs` creates an `Option<rusty_foundationmodels::Session>` with all OS tools. Only available on macOS 26+ with Apple Silicon and FoundationModels entitlement.

`src/tools.rs` defines all tools passed to the session:

| Tool | Description |
|------|-------------|
| `run_applescript` | Execute arbitrary AppleScript via `osascript` |
| `open_url` | Open URL in Safari via AppleScript |
| `launch_app` | Activate any macOS app via `tell application X to activate` |
| `run_shell` | Run shell command via `sh -c` |
| `get_system_info` | macOS version + hostname |
| `get_frontmost_app` | Name of the focused app via System Events |
| `get_calendar_events` | Events from Calendar.app for today/tomorrow/a date |
| `get_reminders` | Incomplete reminders from Reminders.app |
| `add_reminder` | Create a new reminder |
| `get_weather` | Current weather via wttr.in |
| `control_music` | Play/pause/next/previous/stop/get_current/play_track in Music.app |
| `create_note` | Create a note in Notes.app |
| `search_wikipedia` | Wikipedia REST API summary (free, no auth) |

The AI writes its own AppleScript — never ask it to. It synthesises Wikipedia results rather than quoting verbatim.

## Key Patterns

### GPUI Global
```rust
struct DbGlobal(Arc<RoverDb>);
impl Global for DbGlobal {}
cx.set_global(DbGlobal(db.clone()));
let db = cx.global::<DbGlobal>().0.clone();
```

### Picker Events
```rust
// rover_picker/src/lib.rs
pub enum PickerEvent {
    Dismiss,
    OpenSettings,
    ToggleActions,
    OpenTerminal,
}
```

### BuiltIn Commands
`BuiltInCommand` enum in `rover_commands/src/registry.rs`:
- `ClipboardHistory` — enter clipboard submenu
- `FileSearch` — enter file search submenu
- `Settings` — open settings window
- `Snippets` — (reserved)
- `OpenTerminal` — open terminal window
- `Browser` — open WebKit browser window

### Window Setup
```rust
WindowOptions {
    kind: WindowKind::PopUp,     // non-activating NSPanel
    titlebar: None,
    window_background: WindowBackgroundAppearance::Transparent,
    ..
}
```

### Carbon Hotkey
`src/hotkey.rs` wraps `RegisterEventHotKey` from Carbon. Hotkey is `Cmd+Space` by default, configurable in settings.

## Platform & Requirements

- macOS only. `LSUIElement = YES` (no dock icon, menu bar only).
- AI features require macOS 26+, Apple Silicon, Xcode with macOS 26 SDK.
- Build: `cargo build -p rover` (requires `cmake` for wasmtime if extensions are enabled).
- `rusty_foundationmodels` lives at `crates/rusty_foundationmodels` — Swift FFI bridge to Apple FoundationModels framework. The `build.rs` compiles `bridge.swift` via `xcrun swiftc` at build time (gracefully degrades if Xcode/macOS 26 SDK is absent).

## Do Not

- Use `open -a AppName` — use AppleScript `tell application X to activate`
- Guess Zed internal API surface — check gpui 0.2 crates.io docs instead
- Truncate Wikipedia results before passing to AI — let AI summarise
- Ask the AI to write AppleScript — it does this autonomously via `run_applescript`
- Use `when_some` or `hide_window` — not in gpui 0.2 public API
