# Rover

A macOS launcher and AI assistant bar. Sits in the menu bar and pops up system-wide with a hotkey.

## Features

- **App launcher** — fuzzy search across all installed applications
- **Calculator** — type any math expression to evaluate it inline
- **Clipboard history** — search and paste from your clipboard history
- **File search** — find files on your Mac
- **AI assistant** — on-device Apple Intelligence (macOS 26+, Apple Silicon)
  - Asks calendar, reminders, weather, music, and more
  - Writes and runs AppleScript autonomously
  - Wikipedia summaries
  - Falls back to AI when no app matches your query
- **Browser** — keyboard-driven WebKit browser window (Cmd+\ for address bar)
- **Terminal** — built-in terminal suggestions; type `> command` to run shell commands

## Input

| Prefix | Behaviour |
|--------|-----------|
| _(none)_ | Fuzzy match apps/commands; AI fires automatically when nothing matches |
| `> cmd` or `! cmd` | Run shell command directly |
| `? query` or `ai query` | Force AI response |

Math expressions are always evaluated alongside matches.

## Requirements

- macOS 14+
- Apple Silicon (for AI features — macOS 26+ required)
- Xcode with macOS 26 SDK (for AI builds)

## Building

```sh
cargo build -p rover
```

For AI features, the local `rusty_foundationmodels` crate must be present at `../rusty_foundationmodels`.

## Settings

Open via the menu bar (Rover → Settings) or press Cmd+, after activation.

- **General** — hotkey, bar position (Top / Center / Bottom)
- **AI** — enable/disable Apple Intelligence, AI in terminal
- **Data** — clear clipboard history, query history

## Architecture

Built with [`gpui`](https://crates.io/crates/gpui) 0.2 (Zed's UI framework, public crates.io release).

```
rover (binary)
├── crates/rover_picker      — generic picker UI
├── crates/rover_theme       — JSON theme system
├── crates/rover_fuzzy       — fuzzy matching
├── crates/rover_commands    — app launcher, calculator, file search
├── crates/rover_db          — SQLite storage
├── crates/rover_browser     — WebKit browser window
└── crates/rover_extensions  — extension manifest loader
```
