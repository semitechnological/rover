# Rover — Agent Rules

## Reference Codebase

Rover is modeled after Zed's patterns. When implementing or modifying any of the following, you MUST read the corresponding Zed source in `~/zed` first:

| Pattern | Zed Reference |
|---------|---------------|
| Picker UI + delegate | `~/zed/crates/picker/src/picker.rs` |
| Command palette | `~/zed/crates/command_palette/src/command_palette.rs` |
| Fuzzy matching | `~/zed/crates/fuzzy/src/strings.rs` |
| Theme system | `~/zed/crates/theme/src/theme.rs` |
| Theme JSON schema | `~/zed/assets/themes/one/one.json` |
| PopUp window (macOS) | `~/zed/crates/gpui_macOS/src/window.rs` (NSPanel) |
| WindowOptions | `~/zed/crates/gpui/src/platform.rs` |
| Extension manifest | `~/zed/crates/extension/src/extension_manifest.rs` |
| Extension API | `~/zed/crates/extension_api/` |
| Extension host/loading | `~/zed/crates/extension_host/` |

Do not guess how Zed does something — read the source.

## GPUI

Rover uses `gpui = "0.2"` from crates.io. This is Zed's GPUI but the public API differs slightly from Zed's internal usage:

- Use `div().flex().flex_col()` not `v_flex()` (not exported)
- Use `div().flex()` not `h_flex()` (not exported)
- Use `cx.processor()` for `uniform_list` callbacks, `cx.listener()` for action callbacks
- There is no `menu::SelectNext` etc. — define your own actions with `actions!()`
- Bind keys with `cx.bind_keys([KeyBinding::new(...)])` using key context strings
- Text input requires implementing `EntityInputHandler` + `Focusable` + a custom `Element` for rendering (see `crates/rover_picker/src/text_input.rs`, adapted from gpui's `examples/input.rs`)
- `uniform_list` needs a parent with flex layout and `flex_grow()` to get height
- `FluentBuilder` trait (`.when()`, `.map()`) is in `gpui::prelude::*`

## Architecture

```
rover (binary)           — main app, window, hotkey, RoverDelegate
crates/rover_picker      — generic Picker<D: PickerDelegate>, TextInput
crates/rover_theme       — Zed-style JSON themes, ThemeRegistry, ActiveTheme
crates/rover_fuzzy       — fuzzy matching via fuzzy-matcher/skim
crates/rover_commands    — CommandRegistry, app launcher, calculator
crates/rover_extensions  — extension.toml manifest, loader, theme registration
crates/rover_extension_api — traits for WASM extensions (future)
```

## Key Patterns

- **Picker**: Generic over `PickerDelegate`. Delegate owns the data, picker owns the UI. Escape clears input first, then dismisses. Confirm returns `bool` — `true` means dismiss.
- **Theme**: JSON files in `~/.config/rover/themes/` + bundled. Flat dot-separated tokens (`surface.background`, `text.muted`, etc.). `rover_theme::theme(cx)` returns active theme.
- **Extensions**: `~/.config/rover/extensions/`, each with `extension.toml`. Launcher-relevant only: themes, icon themes, commands. No languages/grammars/LSP.
- **Window**: `WindowKind::PopUp` for non-activating panel, `titlebar: None`, `WindowBackgroundAppearance::Transparent`.
- **Commands**: `CommandRegistry` holds all commands. `discover_apps()` scans `/Applications`, `/System/Applications`, `~/Applications`.

## Platform

macOS only. Global hotkey via Carbon `RegisterEventHotKey`. `LSUIElement = YES` in Info.plist (no dock icon).
