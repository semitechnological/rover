use gpui::prelude::*;
use gpui::*;
use rover_browser;
use rover_commands::{
    calculator, file_search, BuiltInCommand, Command, CommandKind, CommandRegistry,
};
use rover_db::{ClipboardEntry, RoverDb};
use rover_fuzzy::{match_strings, normalize_query, StringMatch, StringMatchCandidate};
use rover_picker::{ItemAction, Picker, PickerDelegate};
use rover_theme::theme;
use std::collections::VecDeque;
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq)]
pub enum ViewMode {
    Main,
    ClipboardHistory,
    FileSearch,
}

/// Browser tab context shown as pills in the picker.
#[derive(Clone, Default)]
pub struct BrowserContext {
    pub tabs: Vec<(String, String, rover_browser::TabState, u64)>, // (title, url, state, last_active_ms_ago)
    pub active_tab: usize,
}

pub struct RoverDelegate {
    all_commands: Vec<Command>,
    candidates: Vec<StringMatchCandidate>,
    matches: Vec<StringMatch>,
    selected_ix: usize,
    query: String,
    calculator_result: Option<String>,
    db: Arc<RoverDb>,
    view_mode: ViewMode,
    clipboard_entries: Vec<ClipboardEntry>,
    clipboard_filtered: Vec<usize>,
    file_results: Vec<file_search::FileResult>,
    query_history: QueryHistory,
    ai_session: Option<Arc<rusty_foundationmodels::Session>>,
    ai_enabled: bool,
    ai_result: Option<String>,
    ai_loading: bool,
    ai_debounce: Option<Task<()>>,
    inline_terminal: Option<String>,
    url_result: Option<String>,
    semantic: Option<Arc<std::sync::Mutex<rover_semantic::SemanticIndex>>>,
    help_query: Option<String>,
    help_result: Option<String>,
    help_loading: bool,
    /// Browser context — tabs open in the Rover browser
    browser_ctx: BrowserContext,
    /// Filtered browser tab indices (matches current query)
    browser_filtered: Vec<usize>,
    /// Shared live browser tabs ref for passing to new browser windows
    live_tabs: Option<rover_browser::LiveBrowserTabsRef>,
    /// Extension runtimes for executing extension commands
    ext_runtimes: std::collections::HashMap<String, rover_extensions::runtime::ExtensionRuntimeRef>,
    /// Recent clipboard entries shown in compact mode when query is empty
    recent_clips: Vec<ClipboardEntry>,
    /// "quit" or "quit AppName" — target app to close/quit
    quit_target: Option<String>,
    /// "close tab" / "tab close" — close the active browser tab
    close_tab_requested: bool,
    /// Whether clipboard section is expanded in compact mode
    clipboard_expanded: bool,
    /// Terminal output from inline command execution
    terminal_output: Option<String>,
    /// Whether an inline terminal command is currently running
    terminal_running: bool,
}

struct QueryHistory {
    history: Option<VecDeque<String>>,
    cursor: Option<usize>,
    prefix: Option<String>,
}

impl Default for QueryHistory {
    fn default() -> Self {
        Self {
            history: None,
            cursor: None,
            prefix: None,
        }
    }
}

impl QueryHistory {
    fn history(&mut self, db: &RoverDb) -> &mut VecDeque<String> {
        self.history.get_or_insert_with(|| {
            db.list_query_history()
                .unwrap_or_default()
                .into_iter()
                .collect()
        })
    }

    fn add(&mut self, query: String, db: &RoverDb) {
        if let Some(pos) = self.history(db).iter().position(|h| h == &query) {
            self.history(db).remove(pos);
        }
        self.history(db).push_back(query);
        self.cursor = None;
        self.prefix = None;
    }

    fn validate_cursor(&mut self, current_query: &str, db: &RoverDb) -> Option<usize> {
        if let Some(pos) = self.cursor {
            if self.history(db).get(pos).map(|s| s.as_str()) != Some(current_query) {
                self.cursor = None;
                self.prefix = None;
            }
        }
        self.cursor
    }

    fn previous(&mut self, current_query: &str, db: &RoverDb) -> Option<String> {
        if self.validate_cursor(current_query, db).is_none() {
            self.prefix = Some(current_query.to_string());
        }

        let prefix = self.prefix.clone().unwrap_or_default();
        let start_index = self.cursor.unwrap_or(self.history(db).len());

        for i in (0..start_index).rev() {
            if self
                .history(db)
                .get(i)
                .is_some_and(|e| e.starts_with(&prefix))
            {
                self.cursor = Some(i);
                return self.history(db).get(i).cloned();
            }
        }
        None
    }

    fn next(&mut self, current_query: &str, db: &RoverDb) -> Option<String> {
        let selected = self.validate_cursor(current_query, db)?;
        let prefix = self.prefix.clone().unwrap_or_default();

        for i in (selected + 1)..self.history(db).len() {
            if self
                .history(db)
                .get(i)
                .is_some_and(|e| e.starts_with(&prefix))
            {
                self.cursor = Some(i);
                return self.history(db).get(i).cloned();
            }
        }
        None
    }

    fn reset_cursor(&mut self) {
        self.cursor = None;
        self.prefix = None;
    }

    fn is_navigating(&self) -> bool {
        self.cursor.is_some()
    }
}

struct RowOffsets {
    has_browser: bool,
    browser_count: usize,
    has_recent_clips: bool,
    recent_clips_count: usize,
    has_close_tab: bool,
    has_quit: bool,
    has_terminal: bool,
    has_url: bool,
    has_calc: bool,
    has_ai: bool,
    has_help: bool,
    browser_start: usize,
    recent_clips_start: usize,
    close_tab_ix: usize,
    quit_ix: usize,
    terminal_ix: usize,
    url_ix: usize,
    calc_ix: usize,
    ai_ix: usize,
    help_ix: usize,
    command_offset: usize,
    /// Number of items that are "context" (browser + clips) — rendered as pills
    context_count: usize,
}

impl RoverDelegate {
    pub fn new(
        commands: Vec<Command>,
        db: Arc<RoverDb>,
        ai_session: Option<Arc<rusty_foundationmodels::Session>>,
        ai_enabled: bool,
        semantic: Option<Arc<std::sync::Mutex<rover_semantic::SemanticIndex>>>,
        live_browser_ctx: Option<BrowserContext>,
        live_tabs: Option<rover_browser::LiveBrowserTabsRef>,
        ext_runtimes: std::collections::HashMap<String, rover_extensions::runtime::ExtensionRuntimeRef>,
    ) -> Self {
        let candidates: Vec<StringMatchCandidate> = commands
            .iter()
            .enumerate()
            .map(|(i, c)| StringMatchCandidate::new(i, &c.name))
            .collect();
        let matches = match_strings(&candidates, "", 50);
        // Prefer live browser context from BrowserContextGlobal, fall back to saved session
        let browser_ctx = match live_browser_ctx {
            Some(ctx) if !ctx.tabs.is_empty() => ctx,
            _ => match db.load_session() {
                Ok(session) if !session.is_empty() => {
                    let active = session.iter().position(|(_, _, a)| *a).unwrap_or(0);
                    let tabs = session.into_iter().enumerate().map(|(i, (url, title, _))| {
                        let state = if i == active { rover_browser::TabState::Active } else { rover_browser::TabState::Frozen };
                        (title, url, state, 0u64)
                    }).collect();
                    BrowserContext { tabs, active_tab: active }
                }
                _ => BrowserContext::default(),
            },
        };
        Self {
            all_commands: commands,
            candidates,
            matches,
            selected_ix: 0,
            query: String::new(),
            calculator_result: None,
            db,
            view_mode: ViewMode::Main,
            clipboard_entries: Vec::new(),
            clipboard_filtered: Vec::new(),
            file_results: Vec::new(),
            query_history: QueryHistory::default(),
            ai_session,
            ai_enabled,
            ai_result: None,
            ai_loading: false,
            ai_debounce: None,
            inline_terminal: None,
            url_result: None,
            semantic,
            help_query: None,
            help_result: None,
            help_loading: false,
            browser_filtered: (0..browser_ctx.tabs.len()).collect(),
            browser_ctx,
            live_tabs,
            ext_runtimes,
            recent_clips: Vec::new(),
            quit_target: None,
            close_tab_requested: false,
            clipboard_expanded: false,
            terminal_output: None,
            terminal_running: false,
        }
    }

    pub fn enter_submenu(&mut self, mode: ViewMode, cx: &mut Context<Picker<Self>>) {
        self.view_mode = mode.clone();
        self.selected_ix = 0;
        self.query.clear();
        match mode {
            ViewMode::ClipboardHistory => {
                self.clipboard_entries = self.db.list_clipboard_history().unwrap_or_default();
                self.clipboard_filtered = (0..self.clipboard_entries.len()).collect();
            }
            ViewMode::FileSearch => {
                self.file_results.clear();
            }
            ViewMode::Main => {}
        }
        cx.notify();
    }

    pub fn exit_submenu(&mut self, cx: &mut Context<Picker<Self>>) {
        self.view_mode = ViewMode::Main;
        self.selected_ix = 0;
        self.query.clear();
        self.clipboard_entries.clear();
        self.clipboard_filtered.clear();
        self.file_results.clear();
        cx.notify();
    }

    fn row_offsets(&self) -> RowOffsets {
        let has_browser = !self.browser_filtered.is_empty();
        let browser_count = if has_browser { self.browser_filtered.len() } else { 0 };
        let has_recent_clips = self.clipboard_expanded;
        let recent_clips_count = if has_recent_clips { self.recent_clips.len() } else { 0 };
        let has_close_tab = self.close_tab_requested;
        let has_quit = self.quit_target.is_some();
        let has_terminal = self.inline_terminal.is_some();
        let has_url = self.url_result.is_some();
        let has_calc = self.calculator_result.is_some();
        let has_ai = self.ai_loading || self.ai_result.is_some();
        let has_help = self.help_query.is_some();

        let mut next = 0usize;
        let browser_start = next;
        next += browser_count;
        let recent_clips_start = next;
        next += recent_clips_count;
        let context_count = next; // everything up to here is "context" (pills)
        let close_tab_ix = next;
        if has_close_tab { next += 1; }
        let quit_ix = next;
        if has_quit { next += 1; }
        let terminal_ix = next;
        if has_terminal { next += 1; }
        let url_ix = next;
        if has_url { next += 1; }
        let calc_ix = next;
        if has_calc { next += 1; }
        let ai_ix = next;
        if has_ai { next += 1; }
        let help_ix = next;
        if has_help { next += 1; }
        let command_offset = next;
        RowOffsets {
            has_browser, browser_count, has_recent_clips, recent_clips_count,
            has_close_tab, has_quit, has_terminal, has_url, has_calc, has_ai, has_help,
            browser_start, recent_clips_start, close_tab_ix,
            quit_ix, terminal_ix, url_ix, calc_ix, ai_ix, help_ix,
            command_offset, context_count,
        }
    }

    // ── Pill rendering helpers ──────────────────────────────────────────────

    fn pill(selected: bool, colors: &rover_theme::ThemeColors) -> Div {
        let mut p = div()
            .px_2()
            .py(px(4.0))
            .rounded_full()
            .border_1()
            .flex()
            .items_center()
            .gap_1()
            .cursor_pointer();
        if selected {
            p = p
                .bg(colors.element_selected)
                .border_color(colors.text_accent);
        } else {
            p = p
                .bg(colors.surface_background)
                .border_color(colors.border_variant);
        }
        p
    }

    fn state_pill(label: &str, bg_color: Rgba, text_color: Hsla, selected: bool, colors: &rover_theme::ThemeColors) -> Div {
        let mut p = div()
            .px_2()
            .py(px(4.0))
            .rounded_full()
            .border_1()
            .flex()
            .items_center()
            .gap_1()
            .cursor_pointer();
        if selected {
            p = p.bg(colors.element_selected).border_color(text_color);
        } else {
            p = p.bg(bg_color).border_color(colors.border_variant);
        }
        p.child(
            div()
                .text_color(text_color)
                .text_size(px(10.0))
                .font_weight(FontWeight::SEMIBOLD)
                .child(SharedString::from(label.to_string())),
        )
    }

    fn render_calculator_pill(
        &self,
        selected: bool,
        colors: &rover_theme::ThemeColors,
    ) -> AnyElement {
        let result = self.calculator_result.as_ref().unwrap();
        Self::pill(selected, colors)
            .child(
                div()
                    .text_color(colors.text_muted)
                    .text_size(px(11.0))
                    .child(SharedString::from("=")),
            )
            .child(
                div()
                    .text_color(colors.text_accent)
                    .text_size(px(12.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(SharedString::from(result.clone())),
            )
            .into_any_element()
    }

    fn render_command_pill(
        &self,
        command: &Command,
        selected: bool,
        colors: &rover_theme::ThemeColors,
    ) -> AnyElement {
        let text_color = if selected { colors.text_accent } else { colors.text };
        Self::pill(selected, colors)
            .child(
                div()
                    .text_color(text_color)
                    .text_size(px(12.0))
                    .child(SharedString::from(command.name.clone())),
            )
            .when(command.has_submenu, |el| {
                el.child(
                    div()
                        .text_color(colors.text_muted)
                        .text_size(px(10.0))
                        .child(SharedString::from(">")),
                )
            })
            .into_any_element()
    }

    fn render_clipboard_pill(
        &self,
        entry: &ClipboardEntry,
        selected: bool,
        colors: &rover_theme::ThemeColors,
    ) -> AnyElement {
        let preview: String = entry
            .content
            .chars()
            .take(30)
            .collect::<String>()
            .replace('\n', " ");
        Self::pill(selected, colors)
            .child(
                div()
                    .text_color(colors.text)
                    .text_size(px(11.0))
                    .overflow_hidden()
                    .child(SharedString::from(preview)),
            )
            .into_any_element()
    }

    fn render_file_pill(
        &self,
        file: &file_search::FileResult,
        selected: bool,
        colors: &rover_theme::ThemeColors,
    ) -> AnyElement {
        Self::pill(selected, colors)
            .child(
                div()
                    .text_color(colors.text)
                    .text_size(px(11.0))
                    .child(SharedString::from(file.name.clone())),
            )
            .into_any_element()
    }

    fn render_terminal_pill(
        &self,
        command: &str,
        selected: bool,
        colors: &rover_theme::ThemeColors,
    ) -> AnyElement {
        Self::state_pill("$", rgba(0x88888826u32), colors.text_muted, selected, colors)
            .child(
                div()
                    .text_color(colors.text)
                    .text_size(px(11.0))
                    .child(SharedString::from(command.to_string())),
            )
            .into_any_element()
    }

    fn render_url_pill(
        &self,
        url: &str,
        selected: bool,
        colors: &rover_theme::ThemeColors,
    ) -> AnyElement {
        Self::state_pill("URL", rgba(0x5588cc26u32), colors.text_accent, selected, colors)
            .child(
                div()
                    .text_color(colors.text)
                    .text_size(px(11.0))
                    .child(SharedString::from(url.to_string())),
            )
            .into_any_element()
    }

    fn render_ai_pill(&self, selected: bool, colors: &rover_theme::ThemeColors) -> AnyElement {
        let text = if self.ai_loading {
            "...".to_string()
        } else {
            self.ai_result.clone().unwrap_or_default()
        };
        Self::state_pill("AI", rgba(0x5588cc26u32), colors.text_accent, selected, colors)
            .child(
                div()
                    .text_color(colors.text)
                    .text_size(px(11.0))
                    .child(SharedString::from(text)),
            )
            .into_any_element()
    }

    fn render_help_pill(&self, selected: bool, colors: &rover_theme::ThemeColors) -> AnyElement {
        let text = if self.help_loading {
            "Searching...".to_string()
        } else if let Some(result) = &self.help_result {
            if result.len() > 60 {
                format!("{}...", &result[..57])
            } else {
                result.clone()
            }
        } else {
            format!("Help: {}", self.help_query.as_deref().unwrap_or(""))
        };
        Self::state_pill("?", rgba(0x34c75926u32), colors.text_accent, selected, colors)
            .child(
                div()
                    .text_color(colors.text)
                    .text_size(px(11.0))
                    .child(SharedString::from(text)),
            )
            .into_any_element()
    }

    fn render_browser_pill(
        &self,
        title: &str,
        url: &str,
        _is_active: bool,
        is_frozen: bool,
        selected: bool,
        colors: &rover_theme::ThemeColors,
    ) -> AnyElement {
        let text_color = if selected { colors.text_accent } else { colors.text };
        let short_title = if title.chars().count() > 20 {
            let end = title.char_indices().nth(18).map(|(b, _)| b).unwrap_or(title.len());
            format!("{}…", &title[..end])
        } else {
            title.to_string()
        };
        let mut pill = Self::pill(selected, colors)
            .when(is_frozen && !selected, |el| el.opacity(0.5))
            .child(
                div()
                    .text_color(text_color)
                    .text_size(px(11.0))
                    .child(SharedString::from(short_title)),
            );
        // Show URL domain when selected
        if selected {
            let domain = rover_browser::domain_from_url(url).to_string();
            pill = pill.child(
                div()
                    .text_color(colors.text_muted)
                    .text_size(px(9.0))
                    .child(SharedString::from(domain)),
            );
        }
        pill.into_any_element()
    }

    fn render_recent_clip_pill(
        &self,
        entry: &ClipboardEntry,
        selected: bool,
        colors: &rover_theme::ThemeColors,
    ) -> AnyElement {
        let preview: String = entry.content.chars().take(20).collect::<String>().replace('\n', " ");
        Self::state_pill("📋", rgba(0x88888826u32), colors.text_muted, selected, colors)
            .child(
                div()
                    .text_color(colors.text)
                    .text_size(px(11.0))
                    .overflow_hidden()
                    .child(SharedString::from(preview)),
            )
            .into_any_element()
    }

    /// Render a command as a full-width list row (non-compact mode).
    fn render_command_row(
        &self,
        command: &Command,
        selected: bool,
        colors: &rover_theme::ThemeColors,
    ) -> AnyElement {
        let bg = if selected { colors.element_selected } else { gpui::transparent_black() };
        let text_color = if selected { colors.text_accent } else { colors.text };
        div()
            .px_2()
            .py(px(5.0))
            .rounded_md()
            .bg(bg)
            .flex()
            .items_center()
            .justify_between()
            .cursor_pointer()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_color(text_color)
                            .text_size(px(13.0))
                            .child(SharedString::from(command.name.clone())),
                    )
                    .child(
                        div()
                            .text_color(colors.text_muted)
                            .text_size(px(11.0))
                            .child(SharedString::from(command.description.clone())),
                    ),
            )
            .when(command.has_submenu, |el| {
                el.child(
                    div()
                        .text_color(colors.text_muted)
                        .text_size(px(10.0))
                        .child(SharedString::from("›")),
                )
            })
            .into_any_element()
    }

    fn render_close_tab_pill(
        &self,
        selected: bool,
        colors: &rover_theme::ThemeColors,
    ) -> AnyElement {
        Self::state_pill("✕", rgba(0xff444426u32), colors.text_accent, selected, colors)
            .child(
                div()
                    .text_color(colors.text)
                    .text_size(px(11.0))
                    .child(SharedString::from("Close browser tab")),
            )
            .into_any_element()
    }

    fn render_quit_pill(
        &self,
        target: &str,
        selected: bool,
        colors: &rover_theme::ThemeColors,
    ) -> AnyElement {
        let label = if target.is_empty() {
            "Close frontmost window".to_string()
        } else {
            format!("Quit {target}")
        };
        Self::state_pill("✕", rgba(0xff444426u32), colors.text_accent, selected, colors)
            .child(
                div()
                    .text_color(colors.text)
                    .text_size(px(11.0))
                    .child(SharedString::from(label)),
            )
            .into_any_element()
    }

    pub fn select_history(&mut self, up: bool, query: &str) -> Option<String> {
        if self.view_mode != ViewMode::Main {
            return None;
        }
        if up {
            let should_use = self.selected_ix == 0 || self.query_history.is_navigating();
            if should_use {
                return self.query_history.previous(query, &self.db);
            }
        } else if self.query_history.is_navigating() {
            if let Some(q) = self.query_history.next(query, &self.db) {
                return Some(q);
            } else {
                let prefix = self.query_history.prefix.take().unwrap_or_default();
                self.query_history.reset_cursor();
                return Some(prefix);
            }
        }
        None
    }
}

impl PickerDelegate for RoverDelegate {
    type ListItem = AnyElement;

    fn match_count(&self) -> usize {
        match self.view_mode {
            ViewMode::Main => {
                let o = self.row_offsets();
                o.command_offset + self.matches.len()
            }
            ViewMode::ClipboardHistory => self.clipboard_filtered.len(),
            ViewMode::FileSearch => self.file_results.len(),
        }
    }

    fn selected_index(&self) -> usize {
        self.selected_ix
    }

    fn set_selected_index(&mut self, ix: usize, _cx: &mut Context<Picker<Self>>) {
        self.selected_ix = ix;
    }

    fn placeholder_text(&self) -> Arc<str> {
        match self.view_mode {
            ViewMode::Main => Arc::from("Do anything..."),
            ViewMode::ClipboardHistory => Arc::from("Search clipboard history..."),
            ViewMode::FileSearch => Arc::from("Search for files..."),
        }
    }

    fn update_matches(&mut self, query: String, _cx: &mut Context<Picker<Self>>) -> Task<()> {
        self.query = query.clone();
        self.query_history.reset_cursor();

        match self.view_mode {
            ViewMode::Main => {
                let normalized = normalize_query(&query);

                // Priority 1: explicit terminal prefix (> cmd or ! cmd) — exclusive mode
                let terminal_prefix = query.starts_with("> ") || query.starts_with("! ");
                if terminal_prefix {
                    let cmd = query[2..].trim().to_string();
                    self.inline_terminal = if cmd.is_empty() { None } else { Some(cmd) };
                    self.calculator_result = None;
                    self.url_result = None;
                    self.ai_result = None;
                    self.ai_loading = false;
                    self.ai_debounce = None;
                    self.browser_filtered.clear();
                    self.matches = vec![];
                    self.selected_ix = 0;
                    return Task::ready(());
                }
                self.inline_terminal = None;

                // Filter browser tabs by query (title or URL), sort by recency
                if self.browser_ctx.tabs.is_empty() {
                    self.browser_filtered.clear();
                } else if query.is_empty() {
                    self.browser_filtered = (0..self.browser_ctx.tabs.len()).collect();
                } else {
                    let q = query.to_lowercase();
                    self.browser_filtered = self.browser_ctx.tabs.iter().enumerate()
                        .filter(|(_, (title, url, _, _))| {
                            title.to_lowercase().contains(&q) || url.to_lowercase().contains(&q)
                        })
                        .map(|(i, _)| i)
                        .collect();
                }
                // Sort: active tab first, then by most recently used (smallest ms_ago first)
                if !self.browser_filtered.is_empty() {
                    let active = self.browser_ctx.active_tab;
                    self.browser_filtered.sort_by(|&a, &b| {
                        let a_active = a == active;
                        let b_active = b == active;
                        if a_active != b_active {
                            return b_active.cmp(&a_active);
                        }
                        let a_ms = self.browser_ctx.tabs[a].3;
                        let b_ms = self.browser_ctx.tabs[b].3;
                        a_ms.cmp(&b_ms) // smaller ms_ago = more recent
                    });
                }

                // Load recent clips if expanded (WU-10)
                if self.clipboard_expanded && query.is_empty() {
                    self.recent_clips = self.db.list_clipboard_history()
                        .unwrap_or_default()
                        .into_iter()
                        .take(5)
                        .collect();
                } else if !self.clipboard_expanded {
                    self.recent_clips.clear();
                }

                // Close tab detection — before quit/close so "close tab" doesn't trigger quit
                let lower = query.to_lowercase();
                let tab_close_patterns = [
                    "close tab", "tab close", "close current tab", "close this tab",
                ];
                self.close_tab_requested = tab_close_patterns.iter().any(|p| lower == *p)
                    && !self.browser_ctx.tabs.is_empty();

                // Quit / close detection — skip when target word is "tab"/"tabs"
                if !self.close_tab_requested {
                    if lower == "quit" || lower == "close" || lower == "exit" {
                        self.quit_target = Some(String::new()); // close frontmost window
                    } else if lower.starts_with("quit ") || lower.starts_with("close ") || lower.starts_with("exit ") {
                        let app = query.split_once(' ').map(|(_, a)| a.trim()).unwrap_or("");
                        let app_lower = app.to_lowercase();
                        if app_lower == "tab" || app_lower == "tabs" {
                            self.quit_target = None;
                        } else if !app.is_empty() {
                            self.quit_target = Some(app.to_string());
                        } else {
                            self.quit_target = Some(String::new());
                        }
                    } else {
                        self.quit_target = None;
                    }
                } else {
                    self.quit_target = None;
                }

                // Priority 1a: help prefix (help <query>) — vector search docs
                let help_prefix = query.starts_with("help ")
                    || query.starts_with("Help ")
                    || query == "help";
                if help_prefix {
                    let topic = query.split_once(' ').map(|(_, t)| t.trim()).unwrap_or("");
                    self.help_query = if topic.is_empty() {
                        Some("How to use Rover".to_string())
                    } else {
                        Some(topic.to_string())
                    };
                    self.help_result = None;
                    self.help_loading = false;
                } else {
                    self.help_query = None;
                    self.help_result = None;
                    self.help_loading = false;
                }

                // Priority 1b: URL detection → open in Rover browser
                if rover_browser::looks_like_url(&query) && !query.is_empty() {
                    let url = if query.starts_with("http://") || query.starts_with("https://") {
                        query.clone()
                    } else {
                        format!("https://{query}")
                    };
                    self.url_result = Some(url);
                } else {
                    self.url_result = None;
                }

                // Priority 2: calculator (always evaluate)
                self.calculator_result = calculator::evaluate(&self.query).map(|v| {
                    if v.fract() == 0.0 {
                        format!("{}", v as i64)
                    } else {
                        format!("{:.6}", v)
                            .trim_end_matches('0')
                            .trim_end_matches('.')
                            .to_string()
                    }
                });

                // Priority 3: fuzzy match against commands (always run)
                self.matches = match_strings(&self.candidates, &normalized, 50);
                self.selected_ix = 0;

                // Priority 4: AI — show "Ask AI" hint row (don't fire until Enter)
                let is_ai_explicit = self.ai_enabled
                    && self.ai_session.is_some()
                    && (query.starts_with("? ") || query.starts_with("ai "));

                let has_good_matches = self.matches.first().map(|m| m.score > 0).unwrap_or(false);
                let is_ai_fallback = self.ai_enabled
                    && self.ai_session.is_some()
                    && !query.is_empty()
                    && query.len() >= 6
                    && !has_good_matches
                    && self.calculator_result.is_none();

                if is_ai_explicit || is_ai_fallback {
                    // Show "Ask AI" row but don't fire the request yet
                    self.ai_result = Some("Press Enter to ask AI".to_string());
                    self.ai_loading = false;
                    self.ai_debounce = None;
                } else {
                    self.ai_debounce = None;
                    self.ai_result = None;
                    self.ai_loading = false;
                }
            }
            ViewMode::ClipboardHistory => {
                if query.is_empty() {
                    self.clipboard_filtered = (0..self.clipboard_entries.len()).collect();
                } else {
                    let q = query.to_lowercase();
                    self.clipboard_filtered = self
                        .clipboard_entries
                        .iter()
                        .enumerate()
                        .filter(|(_, e)| e.content.to_lowercase().contains(&q))
                        .map(|(i, _)| i)
                        .collect();
                }
                self.selected_ix = 0;
            }
            ViewMode::FileSearch => {
                self.file_results = file_search::search_files(&query, 30);
                self.selected_ix = 0;
            }
        }
        Task::ready(())
    }

    fn confirm(&mut self, cx: &mut Context<Picker<Self>>) -> bool {
        match self.view_mode {
            ViewMode::Main => {
                let o = self.row_offsets();

                // Browser context — navigate to tab URL in existing browser
                if o.has_browser && self.selected_ix >= o.browser_start && self.selected_ix < o.browser_start + o.browser_count {
                    let filtered_ix = self.selected_ix - o.browser_start;
                    let tab_ix = self.browser_filtered[filtered_ix];
                    let (_, url, _, _) = &self.browser_ctx.tabs[tab_ix];
                    let url = url.clone();
                    let browser_db = self.db.clone();
                    let sem = self.semantic.clone();
                    let lt = self.live_tabs.clone();
                    cx.spawn(async move |_this, cx: &mut AsyncApp| {
                        cx.update(|cx| {
                            rover_browser::navigate_or_open(cx, &url, Some(browser_db), sem, lt);
                        })
                        .ok();
                    })
                    .detach();
                    return true;
                }

                // Recent clipboard — paste to clipboard
                if o.has_recent_clips && self.selected_ix >= o.recent_clips_start && self.selected_ix < o.recent_clips_start + o.recent_clips_count {
                    let clip_ix = self.selected_ix - o.recent_clips_start;
                    if let Some(entry) = self.recent_clips.get(clip_ix) {
                        cx.write_to_clipboard(ClipboardItem::new_string(entry.content.clone()));
                    }
                    return true;
                }

                // Close browser tab
                if o.has_close_tab && self.selected_ix == o.close_tab_ix {
                    let active = self.browser_ctx.active_tab;
                    if let Some(lt) = &self.live_tabs {
                        if let Ok(mut live) = lt.lock() {
                            live.close_inbox.push(active);
                        }
                    }
                    // Remove from local state
                    if active < self.browser_ctx.tabs.len() {
                        self.browser_ctx.tabs.remove(active);
                        self.browser_filtered.retain(|&i| i != active);
                        for i in self.browser_filtered.iter_mut() {
                            if *i > active { *i -= 1; }
                        }
                        if self.browser_ctx.active_tab >= self.browser_ctx.tabs.len() && self.browser_ctx.active_tab > 0 {
                            self.browser_ctx.active_tab -= 1;
                        }
                    }
                    return true;
                }

                // Quit command
                if o.has_quit && self.selected_ix == o.quit_ix {
                    if let Some(target) = &self.quit_target {
                        if target.is_empty() {
                            // Close frontmost window via Cmd+W keystroke (no assistive access needed)
                            std::thread::spawn(move || {
                                std::thread::sleep(std::time::Duration::from_millis(400));
                                let _ = std::process::Command::new("osascript")
                                    .arg("-e")
                                    .arg(r#"tell application "System Events" to keystroke "w" using command down"#)
                                    .status();
                            });
                        } else {
                            // Quit the named app
                            let target = target.clone();
                            std::thread::spawn(move || {
                                std::thread::sleep(std::time::Duration::from_millis(400));
                                let _ = std::process::Command::new("osascript")
                                    .arg("-e")
                                    .arg(format!(r#"tell application "{target}" to quit"#))
                                    .status();
                            });
                        }
                    }
                    return true;
                }

                if o.has_terminal && self.selected_ix == o.terminal_ix {
                    if let Some(cmd) = &self.inline_terminal {
                        let cmd = cmd.clone();
                        self.terminal_running = true;
                        self.terminal_output = Some(String::new());
                        cx.spawn({
                            let cmd = cmd.clone();
                            async move |this, cx: &mut AsyncApp| {
                                let output = std::process::Command::new("sh")
                                    .arg("-c")
                                    .arg(&cmd)
                                    .stdout(std::process::Stdio::piped())
                                    .stderr(std::process::Stdio::piped())
                                    .output();
                                let _ = cx.update(|cx| {
                                    let _ = this.update(cx, |picker, cx| {
                                        picker.delegate.terminal_running = false;
                                        match output {
                                            Ok(out) => {
                                                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                                                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                                                let combined = if stderr.is_empty() {
                                                    stdout
                                                } else {
                                                    format!("{stdout}\n{stderr}")
                                                };
                                                picker.delegate.terminal_output = Some(combined);
                                            }
                                            Err(e) => {
                                                picker.delegate.terminal_output = Some(format!("Error: {e}"));
                                            }
                                        }
                                        cx.notify();
                                    });
                                });
                            }
                        }).detach();
                    }
                    return false; // Don't dismiss — show output inline
                }

                if o.has_url && self.selected_ix == o.url_ix {
                    if let Some(url) = self.url_result.clone() {
                        let browser_db = self.db.clone();
                        let sem = self.semantic.clone();
                        let lt = self.live_tabs.clone();
                        cx.spawn(async move |_this, cx: &mut AsyncApp| {
                            cx.update(|cx| {
                                rover_browser::navigate_or_open(cx, &url, Some(browser_db), sem, lt);
                            })
                            .ok();
                        })
                        .detach();
                    }
                    return true;
                }

                if o.has_calc && self.selected_ix == o.calc_ix {
                    if let Some(result) = &self.calculator_result {
                        cx.write_to_clipboard(ClipboardItem::new_string(result.clone()));
                    }
                    return false;
                }

                if o.has_ai && self.selected_ix == o.ai_ix {
                    // If AI hasn't run yet, fire the request now
                    if !self.ai_loading && self.ai_result.as_deref() == Some("Press Enter to ask AI") {
                        if let Some(session) = self.ai_session.clone() {
                            let prompt = if self.query.starts_with("? ") {
                                self.query[2..].to_string()
                            } else if self.query.starts_with("ai ") {
                                self.query[3..].to_string()
                            } else {
                                self.query.clone()
                            };
                            self.ai_loading = true;
                            self.ai_result = None;
                            self.ai_debounce = Some(cx.spawn(async move |this, cx: &mut AsyncApp| {
                                let result = session.respond(&prompt).await;
                                let _ = cx.update(|cx| {
                                    let _ = this.update(cx, |picker, cx| {
                                        picker.delegate.ai_loading = false;
                                        picker.delegate.ai_debounce = None;
                                        picker.delegate.ai_result = Some(match result {
                                            Ok(text) => text,
                                            Err(e) => format!("Error: {e}"),
                                        });
                                        cx.notify();
                                    });
                                });
                            }));
                        }
                        return false;
                    }
                    // If AI has a result, copy it
                    if let Some(result) = &self.ai_result {
                        cx.write_to_clipboard(ClipboardItem::new_string(result.clone()));
                    }
                    return false;
                }

                if o.has_help && self.selected_ix == o.help_ix {
                    if !self.help_loading && self.help_result.is_none() {
                        // Do vector search then pass context to AI
                        let topic = self.help_query.clone().unwrap_or_default();
                        let semantic = self.semantic.clone();
                        let session = self.ai_session.clone();
                        self.help_loading = true;
                        cx.spawn(async move |this, cx: &mut AsyncApp| {
                            // Vector search for relevant docs (help docs + visited pages)
                            let context = if let Some(sem) = &semantic {
                                let idx = sem.lock().unwrap();
                                // Search all collections — help docs and indexed pages
                                idx.search(&topic, None, 6)
                                    .unwrap_or_default()
                                    .into_iter()
                                    .map(|r| {
                                        if r.collection == "pages" {
                                            let url = r.metadata.get("url")
                                                .and_then(|v| v.as_str())
                                                .unwrap_or("unknown");
                                            format!("[From page: {url}]\n{}", r.text)
                                        } else {
                                            r.text
                                        }
                                    })
                                    .collect::<Vec<_>>()
                                    .join("\n\n")
                            } else {
                                String::new()
                            };

                            let answer = if let Some(session) = session {
                                let prompt = format!(
                                    "The user asked for help about: {topic}\n\n\
                                     Here is relevant documentation:\n{context}\n\n\
                                     Give a concise, helpful answer based on the documentation above. \
                                     Be brief (2-4 sentences max)."
                                );
                                match session.respond(&prompt).await {
                                    Ok(text) => text,
                                    Err(e) => format!("Error: {e}"),
                                }
                            } else if !context.is_empty() {
                                // No AI, just show raw docs
                                context
                            } else {
                                "No help available for this topic.".to_string()
                            };

                            let _ = cx.update(|cx| {
                                let _ = this.update(cx, |picker, cx| {
                                    picker.delegate.help_loading = false;
                                    picker.delegate.help_result = Some(answer);
                                    cx.notify();
                                });
                            });
                        }).detach();
                        return false;
                    }
                    // If we already have a result, copy it
                    if let Some(result) = &self.help_result {
                        cx.write_to_clipboard(ClipboardItem::new_string(result.clone()));
                    }
                    return false;
                }

                let match_ix = self.selected_ix.saturating_sub(o.command_offset);

                if let Some(string_match) = self.matches.get(match_ix) {
                    if let Some(command) = self.all_commands.get(string_match.candidate_id) {
                        if let CommandKind::BuiltIn { id } = &command.kind {
                            match id {
                                BuiltInCommand::ClipboardHistory => {
                                    self.enter_submenu(ViewMode::ClipboardHistory, cx);
                                    return false;
                                }
                                BuiltInCommand::FileSearch => {
                                    self.enter_submenu(ViewMode::FileSearch, cx);
                                    return false;
                                }
                                BuiltInCommand::Settings => {
                                    cx.emit(rover_picker::PickerEvent::OpenSettings);
                                    return false;
                                }
                                BuiltInCommand::OpenTerminal => {
                                    cx.emit(rover_picker::PickerEvent::OpenTerminal);
                                    return true;
                                }
                                BuiltInCommand::Snippets => return false,
                                BuiltInCommand::Help => {
                                    // Pre-populate help query so user sees help results
                                    self.update_matches("help ".to_string(), cx).detach();
                                    return false;
                                }
                                BuiltInCommand::Browser => {
                                    let browser_db = self.db.clone();
                                    let sem = self.semantic.clone();
                                    let lt = self.live_tabs.clone();
                                    cx.spawn(async move |_this, cx: &mut AsyncApp| {
                                        cx.update(|cx| {
                                            rover_browser::navigate_or_open(
                                                cx,
                                                rover_browser::DEFAULT_URL,
                                                Some(browser_db),
                                                sem,
                                                lt,
                                            );
                                        })
                                        .ok();
                                    })
                                    .detach();
                                    return true;
                                }
                                BuiltInCommand::CloseWindow => {
                                    // Dismiss picker first, then close the frontmost window
                                    // via Cmd+W keystroke (no assistive access needed).
                                    std::thread::spawn(|| {
                                        std::thread::sleep(std::time::Duration::from_millis(400));
                                        let _ = std::process::Command::new("osascript")
                                            .arg("-e")
                                            .arg(r#"tell application "System Events" to keystroke "w" using command down"#)
                                            .status();
                                    });
                                    return true;
                                }
                            }
                        }

                        // Handle extension commands
                        if let CommandKind::Extension { ext_id, command: cmd_name } = &command.kind {
                            if let Some(runtime) = self.ext_runtimes.get(ext_id) {
                                let rt = runtime.clone();
                                let cmd = cmd_name.clone();
                                let query = self.query.clone();
                                std::thread::spawn(move || {
                                    match rt.run_command(&cmd, &query) {
                                        Ok(output) => log::info!("[ext] {}: {}", cmd, output.text),
                                        Err(e) => log::warn!("[ext] {} failed: {}", cmd, e),
                                    }
                                });
                            }
                            return true;
                        }

                        if !self.query.is_empty() {
                            let _ = self.db.add_query(&self.query);
                            self.query_history.add(self.query.clone(), &self.db);
                        }
                        let _ = self.db.record_command_usage(&command.name);

                        CommandRegistry::execute(command);
                        return true;
                    }
                }
                false
            }
            ViewMode::ClipboardHistory => {
                if let Some(&idx) = self.clipboard_filtered.get(self.selected_ix) {
                    if let Some(entry) = self.clipboard_entries.get(idx) {
                        cx.write_to_clipboard(ClipboardItem::new_string(entry.content.clone()));
                        self.exit_submenu(cx);
                        return true;
                    }
                }
                false
            }
            ViewMode::FileSearch => {
                if let Some(file) = self.file_results.get(self.selected_ix) {
                    let _ = std::process::Command::new("open").arg(&file.path).spawn();
                    return true;
                }
                false
            }
        }
    }

    fn shift_confirm(&mut self, cx: &mut Context<Picker<Self>>) -> bool {
        match self.view_mode {
            ViewMode::Main => {
                let o = self.row_offsets();

                // Shift+Enter on browser tab → open URL in new browser window
                if o.has_browser && self.selected_ix >= o.browser_start && self.selected_ix < o.browser_start + o.browser_count {
                    let filtered_ix = self.selected_ix - o.browser_start;
                    let tab_ix = self.browser_filtered[filtered_ix];
                    let (_, url, _, _) = &self.browser_ctx.tabs[tab_ix];
                    let url = url.clone();
                    let browser_db = self.db.clone();
                    let sem = self.semantic.clone();
                    let lt = self.live_tabs.clone();
                    cx.spawn(async move |_this, cx: &mut AsyncApp| {
                        cx.update(|cx| {
                            rover_browser::open_browser_window_with_live_tabs(cx, &url, Some(browser_db), sem, lt);
                        })
                        .ok();
                    })
                    .detach();
                    return true;
                }

                // Shift+Enter on URL → open in new browser window
                if o.has_url && self.selected_ix == o.url_ix {
                    if let Some(url) = self.url_result.clone() {
                        let browser_db = self.db.clone();
                        let sem = self.semantic.clone();
                        let lt = self.live_tabs.clone();
                        cx.spawn(async move |_this, cx: &mut AsyncApp| {
                            cx.update(|cx| {
                                rover_browser::open_browser_window_with_live_tabs(cx, &url, Some(browser_db), sem, lt);
                            })
                            .ok();
                        })
                        .detach();
                    }
                    return true;
                }

                // Everything else: same as regular confirm
                self.confirm(cx)
            }
            _ => self.confirm(cx),
        }
    }

    fn dismissed(&mut self, _cx: &mut Context<Picker<Self>>) {}

    fn select_history(&mut self, up: bool, query: &str) -> Option<String> {
        RoverDelegate::select_history(self, up, query)
    }

    fn in_submenu(&self) -> bool {
        self.view_mode != ViewMode::Main
    }

    fn exit_submenu(&mut self, cx: &mut Context<Picker<Self>>) {
        RoverDelegate::exit_submenu(self, cx);
    }

    fn ai_widget_content(&self) -> Option<&str> {
        if self.ai_loading {
            return None;
        }
        match &self.ai_result {
            Some(text) if text != "Press Enter to ask AI" => Some(text.as_str()),
            _ => None,
        }
    }

    fn terminal_output(&self) -> Option<&str> {
        self.terminal_output.as_deref()
    }

    fn terminal_running(&self) -> bool {
        self.terminal_running
    }

    fn actions_for_selected(&self) -> Vec<ItemAction> {
        match self.view_mode {
            ViewMode::Main => {
                let o = self.row_offsets();

                // Browser context pills
                if o.has_browser && self.selected_ix >= o.browser_start && self.selected_ix < o.browser_start + o.browser_count {
                    return vec![
                        ItemAction {
                            name: "Switch to Tab".into(),
                            shortcut: Some("\u{21A9}".into()),
                        },
                        ItemAction {
                            name: "Close Tab".into(),
                            shortcut: None,
                        },
                    ];
                }

                // Recent clips
                if o.has_recent_clips && self.selected_ix >= o.recent_clips_start && self.selected_ix < o.recent_clips_start + o.recent_clips_count {
                    return vec![ItemAction {
                        name: "Copy to Clipboard".into(),
                        shortcut: Some("\u{21A9}".into()),
                    }];
                }

                // Quit
                if o.has_quit && self.selected_ix == o.quit_ix {
                    let name = if self.quit_target.as_ref().map(|t| t.is_empty()).unwrap_or(true) {
                        "Close Frontmost Window"
                    } else {
                        "Quit Application"
                    };
                    return vec![ItemAction {
                        name: name.into(),
                        shortcut: Some("\u{21A9}".into()),
                    }];
                }

                if o.has_terminal && self.selected_ix == o.terminal_ix {
                    return vec![ItemAction {
                        name: "Run".into(),
                        shortcut: Some("\u{21A9}".into()),
                    }];
                }

                if o.has_url && self.selected_ix == o.url_ix {
                    return vec![ItemAction {
                        name: "Open in Rover Browser".into(),
                        shortcut: Some("\u{21A9}".into()),
                    }];
                }

                if o.has_calc && self.selected_ix == o.calc_ix {
                    return vec![ItemAction {
                        name: "Copy Result".into(),
                        shortcut: Some("\u{21A9}".into()),
                    }];
                }

                if o.has_ai && self.selected_ix == o.ai_ix {
                    return vec![ItemAction {
                        name: "Copy Answer".into(),
                        shortcut: Some("\u{21A9}".into()),
                    }];
                }

                if o.has_help && self.selected_ix == o.help_ix {
                    let name = if self.help_result.is_some() { "Copy Answer" } else { "Search Docs" };
                    return vec![ItemAction {
                        name: name.into(),
                        shortcut: Some("\u{21A9}".into()),
                    }];
                }

                let match_ix = self.selected_ix.saturating_sub(o.command_offset);

                if let Some(string_match) = self.matches.get(match_ix) {
                    if let Some(command) = self.all_commands.get(string_match.candidate_id) {
                        let mut actions = vec![];
                        match &command.kind {
                            CommandKind::LaunchApp { .. } => {
                                actions.push(ItemAction {
                                    name: "Open".into(),
                                    shortcut: Some("\u{21A9}".into()),
                                });
                                actions.push(ItemAction {
                                    name: "Show in Finder".into(),
                                    shortcut: None,
                                });
                                actions.push(ItemAction {
                                    name: "Copy Name".into(),
                                    shortcut: None,
                                });
                            }
                            CommandKind::OpenUrl { .. } => {
                                actions.push(ItemAction {
                                    name: "Open URL".into(),
                                    shortcut: Some("\u{21A9}".into()),
                                });
                                actions.push(ItemAction {
                                    name: "Copy URL".into(),
                                    shortcut: None,
                                });
                            }
                            CommandKind::BuiltIn { .. } => {
                                actions.push(ItemAction {
                                    name: "Open".into(),
                                    shortcut: Some("\u{21A9}".into()),
                                });
                            }
                            _ => {
                                actions.push(ItemAction {
                                    name: "Execute".into(),
                                    shortcut: Some("\u{21A9}".into()),
                                });
                            }
                        }
                        return actions;
                    }
                }
                vec![]
            }
            ViewMode::ClipboardHistory => {
                vec![
                    ItemAction {
                        name: "Paste".into(),
                        shortcut: Some("\u{21A9}".into()),
                    },
                    ItemAction {
                        name: "Copy to Clipboard".into(),
                        shortcut: None,
                    },
                    ItemAction {
                        name: "Delete Entry".into(),
                        shortcut: None,
                    },
                ]
            }
            ViewMode::FileSearch => {
                vec![
                    ItemAction {
                        name: "Open".into(),
                        shortcut: Some("\u{21A9}".into()),
                    },
                    ItemAction {
                        name: "Show in Finder".into(),
                        shortcut: None,
                    },
                    ItemAction {
                        name: "Copy Path".into(),
                        shortcut: None,
                    },
                ]
            }
        }
    }

    fn execute_action(&mut self, action_ix: usize, cx: &mut Context<Picker<Self>>) -> bool {
        match self.view_mode {
            ViewMode::Main => {
                let o = self.row_offsets();

                if o.has_browser && self.selected_ix >= o.browser_start && self.selected_ix < o.browser_start + o.browser_count {
                    match action_ix {
                        0 => return self.confirm(cx), // Switch to tab
                        1 => {
                            // Close tab
                            let filtered_ix = self.selected_ix - o.browser_start;
                            let tab_ix = self.browser_filtered[filtered_ix];
                            if let Some(lt) = &self.live_tabs {
                                if let Ok(mut live) = lt.lock() {
                                    live.close_inbox.push(tab_ix);
                                }
                            }
                            // Remove from local state
                            self.browser_ctx.tabs.remove(tab_ix);
                            // Rebuild filtered indices
                            self.browser_filtered.retain(|&i| i != tab_ix);
                            for i in self.browser_filtered.iter_mut() {
                                if *i > tab_ix { *i -= 1; }
                            }
                            if self.browser_ctx.active_tab >= self.browser_ctx.tabs.len() && self.browser_ctx.active_tab > 0 {
                                self.browser_ctx.active_tab -= 1;
                            }
                            if self.selected_ix >= self.browser_filtered.len() && self.selected_ix > 0 {
                                self.selected_ix -= 1;
                            }
                            cx.notify();
                            return false;
                        }
                        _ => return false,
                    }
                }

                if o.has_recent_clips && self.selected_ix >= o.recent_clips_start && self.selected_ix < o.recent_clips_start + o.recent_clips_count {
                    return self.confirm(cx);
                }

                if o.has_close_tab && self.selected_ix == o.close_tab_ix {
                    return self.confirm(cx);
                }

                if o.has_quit && self.selected_ix == o.quit_ix {
                    return self.confirm(cx);
                }

                if o.has_terminal && self.selected_ix == o.terminal_ix {
                    if let Some(cmd) = &self.inline_terminal {
                        let _ = std::process::Command::new("sh").arg("-c").arg(cmd).spawn();
                    }
                    return true;
                }

                if o.has_url && self.selected_ix == o.url_ix {
                    return self.confirm(cx);
                }

                if o.has_calc && self.selected_ix == o.calc_ix {
                    if let Some(result) = &self.calculator_result {
                        cx.write_to_clipboard(ClipboardItem::new_string(result.clone()));
                    }
                    return false;
                }

                if o.has_ai && self.selected_ix == o.ai_ix {
                    if let Some(result) = &self.ai_result {
                        cx.write_to_clipboard(ClipboardItem::new_string(result.clone()));
                    }
                    return false;
                }

                if o.has_help && self.selected_ix == o.help_ix {
                    return self.confirm(cx);
                }

                let match_ix = self.selected_ix.saturating_sub(o.command_offset);

                if let Some(string_match) = self.matches.get(match_ix) {
                    if let Some(command) = self.all_commands.get(string_match.candidate_id) {
                        match (&command.kind, action_ix) {
                            (CommandKind::LaunchApp { path }, 0) => {
                                let _ = std::process::Command::new("open")
                                    .arg("-a")
                                    .arg(path)
                                    .spawn();
                                return true;
                            }
                            (CommandKind::LaunchApp { path }, 1) => {
                                let _ = std::process::Command::new("open")
                                    .arg("-R")
                                    .arg(path)
                                    .spawn();
                                return true;
                            }
                            (CommandKind::LaunchApp { .. }, 2) => {
                                cx.write_to_clipboard(ClipboardItem::new_string(
                                    command.name.clone(),
                                ));
                                return false;
                            }
                            (CommandKind::OpenUrl { url }, 0) => {
                                let _ = std::process::Command::new("open").arg(url).spawn();
                                return true;
                            }
                            (CommandKind::OpenUrl { url }, 1) => {
                                cx.write_to_clipboard(ClipboardItem::new_string(url.clone()));
                                return false;
                            }
                            _ => {
                                return self.confirm(cx);
                            }
                        }
                    }
                }
                false
            }
            ViewMode::ClipboardHistory => {
                if let Some(&idx) = self.clipboard_filtered.get(self.selected_ix) {
                    if let Some(entry) = self.clipboard_entries.get(idx) {
                        match action_ix {
                            0 | 1 => {
                                cx.write_to_clipboard(ClipboardItem::new_string(
                                    entry.content.clone(),
                                ));
                                if action_ix == 0 {
                                    self.exit_submenu(cx);
                                    return true;
                                }
                                return false;
                            }
                            2 => {
                                let id = entry.id;
                                let _ = self.db.delete_clipboard_entry(id);
                                self.clipboard_entries =
                                    self.db.list_clipboard_history().unwrap_or_default();
                                self.clipboard_filtered =
                                    (0..self.clipboard_entries.len()).collect();
                                if self.selected_ix >= self.clipboard_filtered.len()
                                    && self.selected_ix > 0
                                {
                                    self.selected_ix -= 1;
                                }
                                cx.notify();
                                return false;
                            }
                            _ => {}
                        }
                    }
                }
                false
            }
            ViewMode::FileSearch => {
                if let Some(file) = self.file_results.get(self.selected_ix) {
                    match action_ix {
                        0 => {
                            let _ = std::process::Command::new("open").arg(&file.path).spawn();
                            return true;
                        }
                        1 => {
                            let _ = std::process::Command::new("open")
                                .arg("-R")
                                .arg(&file.path)
                                .spawn();
                            return true;
                        }
                        2 => {
                            cx.write_to_clipboard(ClipboardItem::new_string(
                                file.path.to_string_lossy().to_string(),
                            ));
                            return false;
                        }
                        _ => {}
                    }
                }
                false
            }
        }
    }

    fn context_item_count(&self) -> usize {
        if self.view_mode != ViewMode::Main { return 0; }
        self.row_offsets().context_count
    }

    fn render_match(
        &self,
        ix: usize,
        selected: bool,
        cx: &mut Context<Picker<Self>>,
    ) -> Option<Self::ListItem> {
        let theme = theme(cx);
        let colors = &theme.colors;

        match self.view_mode {
            ViewMode::Main => {
                let o = self.row_offsets();

                // Browser context pills
                if o.has_browser && ix >= o.browser_start && ix < o.browser_start + o.browser_count {
                    let filtered_ix = ix - o.browser_start;
                    let tab_ix = self.browser_filtered[filtered_ix];
                    let (title, url, state, _) = &self.browser_ctx.tabs[tab_ix];
                    let is_active = tab_ix == self.browser_ctx.active_tab;
                    let is_frozen = *state == rover_browser::TabState::Frozen;
                    return Some(self.render_browser_pill(title, url, is_active, is_frozen, selected, colors));
                }

                // Recent clipboard pills
                if o.has_recent_clips && ix >= o.recent_clips_start && ix < o.recent_clips_start + o.recent_clips_count {
                    let clip_ix = ix - o.recent_clips_start;
                    let entry = &self.recent_clips[clip_ix];
                    return Some(self.render_recent_clip_pill(entry, selected, colors));
                }

                // Close tab pill
                if o.has_close_tab && ix == o.close_tab_ix {
                    return Some(self.render_close_tab_pill(selected, colors));
                }

                // Quit command
                if o.has_quit && ix == o.quit_ix {
                    let target = self.quit_target.as_deref().unwrap_or("");
                    return Some(self.render_quit_pill(target, selected, colors));
                }

                if o.has_terminal && ix == o.terminal_ix {
                    let cmd = self.inline_terminal.as_deref().unwrap_or("");
                    return Some(self.render_terminal_pill(cmd, selected, colors));
                }

                if o.has_url && ix == o.url_ix {
                    let url = self.url_result.as_deref().unwrap_or("");
                    return Some(self.render_url_pill(url, selected, colors));
                }

                if o.has_calc && ix == o.calc_ix {
                    return Some(self.render_calculator_pill(selected, colors));
                }

                if o.has_ai && ix == o.ai_ix {
                    return Some(self.render_ai_pill(selected, colors));
                }

                if o.has_help && ix == o.help_ix {
                    return Some(self.render_help_pill(selected, colors));
                }

                let match_ix = ix.saturating_sub(o.command_offset);
                let string_match = self.matches.get(match_ix)?;
                let command = self.all_commands.get(string_match.candidate_id)?;
                Some(self.render_command_pill(command, selected, colors))
            }
            ViewMode::ClipboardHistory => {
                let &entry_ix = self.clipboard_filtered.get(ix)?;
                let entry = self.clipboard_entries.get(entry_ix)?;
                Some(self.render_clipboard_pill(entry, selected, colors))
            }
            ViewMode::FileSearch => {
                let file = self.file_results.get(ix)?;
                Some(self.render_file_pill(file, selected, colors))
            }
        }
    }

    fn render_row(
        &self,
        ix: usize,
        selected: bool,
        cx: &mut Context<Picker<Self>>,
    ) -> Option<Self::ListItem> {
        let theme = theme(cx);
        let colors = &theme.colors;

        match self.view_mode {
            ViewMode::Main => {
                let o = self.row_offsets();

                // Non-context items rendered as list rows
                if o.has_close_tab && ix == o.close_tab_ix {
                    return Some(self.render_close_tab_pill(selected, colors));
                }
                if o.has_quit && ix == o.quit_ix {
                    let target = self.quit_target.as_deref().unwrap_or("");
                    return Some(self.render_quit_pill(target, selected, colors));
                }
                if o.has_terminal && ix == o.terminal_ix {
                    let cmd = self.inline_terminal.as_deref().unwrap_or("");
                    return Some(self.render_terminal_pill(cmd, selected, colors));
                }
                if o.has_url && ix == o.url_ix {
                    let url = self.url_result.as_deref().unwrap_or("");
                    return Some(self.render_url_pill(url, selected, colors));
                }
                if o.has_calc && ix == o.calc_ix {
                    return Some(self.render_calculator_pill(selected, colors));
                }
                if o.has_ai && ix == o.ai_ix {
                    return Some(self.render_ai_pill(selected, colors));
                }
                if o.has_help && ix == o.help_ix {
                    return Some(self.render_help_pill(selected, colors));
                }

                let match_ix = ix.saturating_sub(o.command_offset);
                let string_match = self.matches.get(match_ix)?;
                let command = self.all_commands.get(string_match.candidate_id)?;
                Some(self.render_command_row(command, selected, colors))
            }
            _ => self.render_match(ix, selected, cx),
        }
    }
}
