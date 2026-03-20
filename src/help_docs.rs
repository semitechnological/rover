//! Help documentation chunks for semantic search.
//!
//! Each entry is a small, self-contained paragraph that covers one topic.
//! These are indexed into the `"help"` collection of `SemanticIndex` on startup
//! so that `help <query>` can retrieve the most relevant chunks and pass them
//! to AI for a natural-language answer.

use rover_semantic::SemanticIndex;

/// A documentation chunk with a stable id and body text.
struct DocChunk {
    id: &'static str,
    text: &'static str,
}

const HELP_DOCS: &[DocChunk] = &[
    // ── Launcher ─────────────────────────────────────────────────────────────
    DocChunk {
        id: "launcher-basics",
        text: "Rover is a macOS launcher activated by a global hotkey (Cmd+Space by default). \
               It opens a floating search bar where you can type to find apps, run commands, \
               do math, search clipboard history, search files, or ask AI questions. \
               Press Escape or click outside to dismiss.",
    },
    DocChunk {
        id: "launcher-hotkey",
        text: "The global hotkey to open Rover defaults to Cmd+Space. You can change it in \
               Settings. You can also set a custom hotkey by recording a key combination in the \
               Settings window. If Hyperkey mode is enabled, Caps Lock acts as a modifier key.",
    },
    DocChunk {
        id: "launcher-calculator",
        text: "Rover has a built-in calculator. Type any math expression (e.g. '2+2', \
               'sqrt(144)', '3*pi') and the result appears inline. Press Enter to copy the \
               result to your clipboard.",
    },
    DocChunk {
        id: "launcher-terminal",
        text: "Prefix your query with '>' or '!' to run a shell command. For example, \
               '> ls -la' runs that command in a shell. The Terminal built-in command \
               opens a floating terminal window.",
    },
    DocChunk {
        id: "launcher-url",
        text: "Type a URL (like 'github.com' or 'https://example.com') in the launcher and \
               press Enter to open it in the Rover browser. Rover auto-detects URLs.",
    },

    // ── AI ────────────────────────────────────────────────────────────────────
    DocChunk {
        id: "ai-basics",
        text: "Rover has an AI assistant powered by Apple Intelligence (FoundationModels). \
               It requires macOS 26+, Apple Silicon, and the AI feature enabled in Settings. \
               Type a question and when no good matches are found, an 'Ask AI' row appears. \
               Press Enter to run the AI query.",
    },
    DocChunk {
        id: "ai-prefix",
        text: "To explicitly ask the AI, prefix your query with '?' or 'ai '. For example, \
               '? what is the capital of France' or 'ai remind me to buy milk'. The AI has \
               access to macOS tools: AppleScript, shell commands, calendar, reminders, \
               music control, weather, Wikipedia, and notes.",
    },
    DocChunk {
        id: "ai-tools",
        text: "The AI can: run AppleScript to control any app, open URLs, launch apps, \
               run shell commands, get system info, check calendar events, manage reminders, \
               get weather, control Music.app playback, create notes, and search Wikipedia. \
               It writes its own AppleScript autonomously.",
    },

    // ── Browser ──────────────────────────────────────────────────────────────
    DocChunk {
        id: "browser-basics",
        text: "Rover includes a keyboard-driven WebKit browser. Open it via the 'Open Browser' \
               command in the launcher, or from the CLI with 'rover browser'. The browser \
               window is a fullscreen WebView with no embedded chrome — all controls are in \
               the transport overlay.",
    },
    DocChunk {
        id: "browser-transport",
        text: "Press Cmd+\\ to open the transport overlay in the browser. The transport is a \
               small floating popup that appears near your cursor. It shows tab pills, a URL \
               input, and action hints. Type a URL or command and press Enter. Press Escape \
               or click outside to close it.",
    },
    DocChunk {
        id: "browser-commands",
        text: "In the browser transport, you can type commands: 'back' or '<' to go back, \
               'forward' or '>' to go forward, 'reload' or 'r' to reload, 'close' to close \
               the current tab, 'new' to open a new tab, 'find <text>' to search the page, \
               'bookmark' to bookmark the page, 'history' to show history, 'share' to copy \
               the URL to clipboard.",
    },
    DocChunk {
        id: "browser-tabs",
        text: "The browser supports multiple tabs. Use Cmd+T to open a new tab, Cmd+W to \
               close the current tab, Cmd+[ or Cmd+] to navigate back/forward. Tabs are \
               shown as pills in the transport overlay — click a pill to switch tabs. \
               Tab states are indicated by colored dots: green (active), amber (cached), \
               grey (frozen).",
    },
    DocChunk {
        id: "browser-search",
        text: "In the browser transport, type a search query (not a URL) and press Enter to \
               search the web. The default search engine is Perplexity AI. You can also \
               prefix with a search engine name like 'g: query' for Google or 'ddg: query' \
               for DuckDuckGo.",
    },
    DocChunk {
        id: "browser-content-blocking",
        text: "The Rover browser has built-in content blocking based on uBlock Origin Lite. \
               It blocks ads and trackers using three layers: network blocking via \
               WKContentRuleList, cosmetic filtering to hide ad elements, and scriptlet \
               injection to stub tracking SDKs. This is always on.",
    },

    // ── Clipboard ────────────────────────────────────────────────────────────
    DocChunk {
        id: "clipboard-history",
        text: "Rover monitors your clipboard and keeps a history of up to 500 entries. \
               Open clipboard history from the launcher by selecting 'Clipboard History' \
               or searching for it. In the submenu, search entries by content. Press Enter \
               to paste the selected entry (copies it to clipboard).",
    },

    // ── File Search ──────────────────────────────────────────────────────────
    DocChunk {
        id: "file-search",
        text: "Rover can search for files on your system using Spotlight (mdfind). Select \
               'File Search' from the launcher to enter the file search submenu. Type a \
               filename or keyword and results appear in real time. Press Enter to open \
               a file, or use actions to show in Finder or copy the path.",
    },

    // ── Settings ─────────────────────────────────────────────────────────────
    DocChunk {
        id: "settings-basics",
        text: "Open Settings from the launcher ('Settings' command) or via Cmd+, from the \
               menu bar. Settings include: hotkey configuration, theme selection, AI toggle, \
               Hyperkey mode, and more. Settings are stored in a local SQLite database.",
    },
    DocChunk {
        id: "settings-themes",
        text: "Rover supports custom themes. Themes are JSON files with dot-separated color \
               tokens (e.g. 'text', 'text.muted', 'background', 'element.selected'). \
               Extensions can provide additional themes. Select a theme in the Settings window.",
    },
    DocChunk {
        id: "settings-hyperkey",
        text: "Hyperkey mode remaps Caps Lock to act as a modifier key (Ctrl+Option+Cmd+Shift). \
               Enable it in Settings. This lets you use Caps Lock + another key as a hotkey \
               without conflicting with other shortcuts.",
    },

    // ── CLI ──────────────────────────────────────────────────────────────────
    DocChunk {
        id: "cli-usage",
        text: "Rover can be used from the command line. Available CLI commands: \
               'rover' (open launcher), 'rover open <url>' (open URL in browser), \
               'rover search <query>' (search the web), 'rover browser' (open browser), \
               'rover launch <app>' (activate a macOS app), 'rover run <cmd>' (run shell command), \
               'rover screenshot [name]' (take a screenshot), 'rover help', 'rover version'.",
    },

    // ── Extensions ───────────────────────────────────────────────────────────
    DocChunk {
        id: "extensions",
        text: "Rover supports extensions defined by extension.toml manifest files. Extensions \
               can provide custom themes and (in future) WASM-based commands. The extension \
               system uses the rover_extension_api traits. Place extensions in the extensions \
               directory and they are loaded on startup.",
    },

    // ── Keyboard Shortcuts ───────────────────────────────────────────────────
    DocChunk {
        id: "shortcuts-launcher",
        text: "Launcher keyboard shortcuts: Cmd+Space (open/toggle Rover), Escape (dismiss), \
               Enter (confirm selection), Up/Down arrows (navigate results), Tab (show actions), \
               Cmd+, (open settings), Cmd+Q (quit Rover). Use Up/Down to browse query history \
               when the input is focused.",
    },
    DocChunk {
        id: "shortcuts-browser",
        text: "Browser keyboard shortcuts: Cmd+\\ (open transport/command bar), \
               Cmd+T (new tab), Cmd+W (close tab), Cmd+[ (go back), Cmd+] (go forward), \
               Cmd+R (reload), Cmd+Shift+C (copy URL to clipboard). \
               In the transport: Enter (submit), Escape (close transport).",
    },

    // ── Help system ──────────────────────────────────────────────────────────
    DocChunk {
        id: "help-system",
        text: "Type 'help' followed by a topic to get contextual help. For example, \
               'help browser', 'help shortcuts', 'help ai', 'help clipboard'. Rover uses \
               semantic vector search to find the most relevant documentation and then \
               passes it to AI for a natural-language answer.",
    },
];

/// Index all help documentation chunks into the `"help"` collection.
/// Call this from a background thread — embedding is CPU-intensive on first run.
pub fn index_help_docs(index: &mut SemanticIndex) {
    for chunk in HELP_DOCS {
        if let Err(e) = index.upsert(
            chunk.id,
            "help",
            chunk.text,
            serde_json::json!({}),
        ) {
            log::warn!("Failed to index help doc '{}': {e}", chunk.id);
        }
    }
    log::info!("help_docs: indexed {} documentation chunks", HELP_DOCS.len());
}
