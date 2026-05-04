# Rover SwiftUI Frontend Status

## Completed

- `CLAUDE.md` points to `AGENTS.md`; `AGENTS.md` is the source instruction file.
- SwiftUI frontend has launcher parity plugins for app launcher, calculator, clipboard, file search, terminal, browser, web search, Google integrations, macOS tools, emoji, help, AI, and semantic search.
- Cmd+Opt+Space opens the SwiftUI launcher in emoji search mode.
- Settings now has General, Hotkeys, AI, Data, and About sections.
- Settings-backed behavior is wired for built-in commands on empty query, bar position, launcher hotkey preset, and AI enable/disable.
- SwiftPM test target exists for SwiftUI plugin behavior.
- Stable `rustfmt.toml` works with stable rustfmt.
- SwiftUI Rust bridge outputs were regenerated so the runtime UniFFI checksum matches the dylib.

## Verified

- `cargo fmt --check`
- `cargo check`
- `cargo test`
- `cargo clippy --all-targets --all-features`
- `swift test --package-path RoverSwiftUI`
- `swift build --package-path RoverSwiftUI -Xswiftc -warnings-as-errors`
- `swift package --package-path RoverSwiftUI plugin --allow-writing-to-package-directory rust-build`
- `RoverSwiftUI/.build/arm64-apple-macosx/debug/RoverApp` starts without the UniFFI checksum crash.

## Remaining Manual QA

- Verify global launcher hotkey presets on a signed app bundle.
- Verify Cmd+Opt+Space does not conflict with the system emoji picker on the target machine.
- Verify Notes, Reminders, Calendar, and file search permissions on a fresh macOS account.
- Verify full Raycast-style keyboard flow in the running UI: selection movement, Enter, Cmd+Enter, Cmd+1 through Cmd+9, Escape, outside-click dismissal, settings window, browser window, and terminal window.
- Verify Apple Intelligence availability on macOS 26 hardware with the FoundationModels bridge available.

## External Dependency Note

The Rust gates currently depend on the local path dependency `/Users/undivisible/projects/rs_ai`. That checkout needed `futures_channel::mpsc` imported outside the `foundation_models_bridge` cfg so Rover can compile when the bridge is unavailable.
