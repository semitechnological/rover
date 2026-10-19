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
    has_terminal: bool,
    has_url: bool,
    has_calc: bool,
    has_ai: bool,
    has_help: bool,
    terminal_ix: usize,
    url_ix: usize,
    calc_ix: usize,
    ai_ix: usize,
    help_ix: usize,
    command_offset: usize,
}

impl RoverDelegate {
    pub fn new(
        commands: Vec<Command>,
        db: Arc<RoverDb>,
        ai_session: Option<Arc<rusty_foundationmodels::Session>>,
        ai_enabled: bool,
        semantic: Option<Arc<std::sync::Mutex<rover_semantic::SemanticIndex>>>,
    ) -> Self {
        let candidates: Vec<StringMatchCandidate> = commands
            .iter()
            .enumerate()
            .map(|(i, c)| StringMatchCandidate::new(i, &c.name))
            .collect();
        let matches = match_strings(&candidates, "", 50);
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

    /// Returns `(has_terminal, has_url, has_calc, has_ai, terminal_ix, url_ix, calc_ix, ai_ix, command_offset)`.
    fn row_offsets(&self) -> RowOffsets {
        let has_terminal = self.inline_terminal.is_some();
        let has_url = self.url_result.is_some();
        let has_calc = self.calculator_result.is_some();
        let has_ai = self.ai_loading || self.ai_result.is_some();
        let mut next = 0usize;
        let terminal_ix = next;
        if has_terminal { next += 1; }
        let url_ix = next;
        if has_url { next += 1; }
        let calc_ix = next;
        if has_calc { next += 1; }
        let ai_ix = next;
        if has_ai { next += 1; }
        let has_help = self.help_query.is_some();
        let help_ix = next;
        if has_help { next += 1; }
        let command_offset = next;
        RowOffsets { has_terminal, has_url, has_calc, has_ai, has_help, terminal_ix, url_ix, calc_ix, ai_ix, help_ix, command_offset }
    }

    // ── Tinycast row rendering helpers ──────────────────────────────────────

    fn icon_tile(glyph: &str, bg_hex: u32, running: bool) -> Div {
        div()
            .w(px(26.0))
            .h(px(26.0))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .child(
                div()
                    .w(px(24.0))
                    .h(px(24.0))
                    .rounded(px(6.0))
                    .bg(rgb(bg_hex))
                    .border_1()
                    .border_color(rgb(0x3a3a42))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(12.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(0xffffff))
                    .child(SharedString::from(glyph.to_string())),
            )
            .when(running, |tile| {
                tile.child(
                    div()
                        .mt(px(1.0))
                        .w(px(4.0))
                        .h(px(4.0))
                        .rounded(px(2.0))
                        .bg(rgb(0xd4d4d8)),
                )
            })
    }

    fn tinycast_row(
        title: impl Into<String>,
        glyph: &str,
        bg_hex: u32,
        running: bool,
        keycaps: &[&str],
        kind_label: &str,
        selected: bool,
    ) -> AnyElement {
        div()
            .w_full()
            .h(px(36.0))
            .px(px(10.0))
            .rounded(px(10.0))
            .bg(rgb(if selected {
                rover_picker::TC_SELECTION
            } else {
                rover_picker::TC_SCRIM
            }))
            .flex()
            .items_center()
            .justify_between()
            .gap(px(10.0))
            .cursor_pointer()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .overflow_hidden()
                    .child(Self::icon_tile(glyph, bg_hex, running))
                    .child(
                        div()
                            .text_size(px(13.5))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(rover_picker::TC_TEXT_PRIMARY))
                            .child(SharedString::from(title.into())),
                    )
                    .when(!keycaps.is_empty(), |left| {
                        left.child(rover_picker::render_keycap_sequence(keycaps))
                    }),
            )
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(rgb(rover_picker::TC_TEXT_SECONDARY))
                    .child(SharedString::from(kind_label.to_string())),
            )
            .into_any_element()
    }

    fn classify_command(
        command: &Command,
    ) -> (&'static str, &'static str, &'static str, u32, bool, Vec<&'static str>) {
        let lower = command.name.to_ascii_lowercase();
        match &command.kind {
            CommandKind::LaunchApp { .. } => {
                let is_fav = lower.contains("safari")
                    || lower.contains("ghostty")
                    || lower.contains("terminal")
                    || lower.contains("zed")
                    || lower.contains("finder");
                let glyph = if lower.contains("safari") {
                    "🧭"
                } else if lower.contains("terminal") || lower.contains("ghostty") {
                    ">_"
                } else if lower.contains("finder") {
                    "📁"
                } else {
                    "⌘"
                };
                (
                    if is_fav { "Favorites" } else { "Applications" },
                    "Application",
                    glyph,
                    0x2563eb,
                    is_fav,
                    Vec::new(),
                )
            }
            CommandKind::BuiltIn { id } => match id {
                BuiltInCommand::Browser => (
                    "Favorites",
                    "Browser",
                    "🌐",
                    0x0ea5e9,
                    true,
                    vec!["⌘", "B"],
                ),
                BuiltInCommand::OpenTerminal => (
                    "Applications",
                    "Application",
                    ">_",
                    0x10b981,
                    true,
                    vec!["⌘", "J"],
                ),
                BuiltInCommand::ClipboardHistory => (
                    "Applications",
                    "Application",
                    "📋",
                    0x0d9488,
                    false,
                    vec!["⌘", "⇧", "V"],
                ),
                BuiltInCommand::FileSearch => (
                    "Applications",
                    "Application",
                    "🔎",
                    0x3b82f6,
                    false,
                    vec!["⌘", "F"],
                ),
                BuiltInCommand::Settings => (
                    "Applications",
                    "Settings",
                    "⚙",
                    0x6366f1,
                    false,
                    vec!["⌘", ","],
                ),
                BuiltInCommand::Help => (
                    "Applications",
                    "Guide",
                    "?",
                    0x8b5cf6,
                    false,
                    vec!["⌘", "?"],
                ),
                BuiltInCommand::Snippets => (
                    "Applications",
                    "Command",
                    "✂",
                    0x64748b,
                    false,
                    Vec::new(),
                ),
            },
            CommandKind::Action(_) => {
                if lower.starts_with("left half") {
                    (
                        "Favorites",
                        "Window Management",
                        "◧",
                        0x0284c7,
                        false,
                        vec!["⌃", "⌥", "←"],
                    )
                } else if lower.starts_with("right half") {
                    (
                        "Favorites",
                        "Window Management",
                        "◨",
                        0x0284c7,
                        false,
                        vec!["⌃", "⌥", "→"],
                    )
                } else if lower == "maximize" {
                    (
                        "Window Management",
                        "Window Management",
                        "⬜",
                        0x0284c7,
                        false,
                        vec!["⌃", "⌥", "↵"],
                    )
                } else if lower == "center" {
                    (
                        "Window Management",
                        "Window Management",
                        "⊡",
                        0x0284c7,
                        false,
                        vec!["⌃", "⌥", "C"],
                    )
                } else if lower.contains("room") {
                    (
                        "Favorites",
                        "Room",
                        "⊞",
                        0x7c5cff,
                        true,
                        vec!["⌘", "Space"],
                    )
                } else if lower.contains("niri") {
                    (
                        "Favorites",
                        "Niri Strip",
                        "↔",
                        0x2563eb,
                        false,
                        vec!["⌘", "R"],
                    )
                } else {
                    (
                        "Window Management",
                        "Command",
                        "⌘",
                        0x4f46e5,
                        false,
                        Vec::new(),
                    )
                }
            }
            CommandKind::OpenUrl { .. } => ("Favorites", "Browser", "🌐", 0x0ea5e9, false, Vec::new()),
            CommandKind::Calculator { .. } => (
                "Results",
                "Calculator",
                "=",
                0xf59e0b,
                false,
                vec!["↵"],
            ),
        }
    }

    fn section_for_index(&self, ix: usize) -> &'static str {
        if !self.query.trim().is_empty() {
            return "Results";
        }
        match self.view_mode {
            ViewMode::Main => {
                let o = self.row_offsets();
                if ix < o.command_offset {
                    return "Results";
                }
                let match_ix = ix.saturating_sub(o.command_offset);
                if let Some(m) = self.matches.get(match_ix) {
                    if let Some(cmd) = self.all_commands.get(m.candidate_id) {
                        return Self::classify_command(cmd).0;
                    }
                }
                "Applications"
            }
            ViewMode::ClipboardHistory => "Clipboard History",
            ViewMode::FileSearch => "Files",
        }
    }

    fn render_calculator_pill(
        &self,
        selected: bool,
        _colors: &rover_theme::ThemeColors,
    ) -> AnyElement {
        let result = self.calculator_result.as_ref().unwrap();
        Self::tinycast_row(
            format!("= {result}"),
            "=",
            0xf59e0b,
            false,
            &["↵"],
            "Calculator",
            selected,
        )
    }

    fn render_command_pill(
        &self,
        command: &Command,
        selected: bool,
        _colors: &rover_theme::ThemeColors,
    ) -> AnyElement {
        let (_sec, kind_label, glyph, bg_hex, running, keycaps) = Self::classify_command(command);
        Self::tinycast_row(
            command.name.clone(),
            glyph,
            bg_hex,
            running,
            &keycaps,
            kind_label,
            selected,
        )
    }

    fn render_clipboard_pill(
        &self,
        entry: &ClipboardEntry,
        selected: bool,
        _colors: &rover_theme::ThemeColors,
    ) -> AnyElement {
        let preview: String = entry
            .content
            .chars()
            .take(56)
            .collect::<String>()
            .replace('\n', " ");
        Self::tinycast_row(
            preview,
            "📋",
            0x0d9488,
            false,
            &["↵"],
            "Clipboard",
            selected,
        )
    }

    fn render_file_pill(
        &self,
        file: &file_search::FileResult,
        selected: bool,
        _colors: &rover_theme::ThemeColors,
    ) -> AnyElement {
        Self::tinycast_row(
            file.name.clone(),
            "📄",
            0x3b82f6,
            false,
            &["↵"],
            "File",
            selected,
        )
    }

    fn render_terminal_pill(
        &self,
        command: &str,
        selected: bool,
        _colors: &rover_theme::ThemeColors,
    ) -> AnyElement {
        Self::tinycast_row(
            format!("Run in Shell: {command}"),
            ">_",
            0x10b981,
            true,
            &["↵"],
            "Inline Shell",
            selected,
        )
    }

    fn render_url_pill(
        &self,
        url: &str,
        selected: bool,
        _colors: &rover_theme::ThemeColors,
    ) -> AnyElement {
        Self::tinycast_row(
            format!("Open {url}"),
            "🌐",
            0x0ea5e9,
            true,
            &["↵"],
            "Browser",
            selected,
        )
    }

    fn render_ai_pill(&self, selected: bool, _colors: &rover_theme::ThemeColors) -> AnyElement {
        let text = if self.ai_loading {
            "...".to_string()
        } else {
            self.ai_result.clone().unwrap_or_default()
        };
        Self::tinycast_row(text, "✦", 0x7c5cff, false, &["↵"], "AI", selected)
    }

    fn render_help_pill(&self, selected: bool, _colors: &rover_theme::ThemeColors) -> AnyElement {
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
        Self::tinycast_row(text, "?", 0x8b5cf6, false, &["↵"], "Guide", selected)
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
            ViewMode::Main => Arc::from("Search for apps and commands..."),
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
                    self.matches = vec![];
                    self.selected_ix = 0;
                    return Task::ready(());
                }
                self.inline_terminal = None;

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

                if o.has_terminal && self.selected_ix == o.terminal_ix {
                    if let Some(cmd) = &self.inline_terminal {
                        let _ = std::process::Command::new("sh").arg("-c").arg(cmd).spawn();
                    }
                    return true;
                }

                if o.has_url && self.selected_ix == o.url_ix {
                    if let Some(url) = self.url_result.clone() {
                        let browser_db = self.db.clone();
                        let sem = self.semantic.clone();
                        cx.spawn(async move |_this, cx: &mut AsyncApp| {
                            cx.update(|cx| {
                                rover_browser::open_browser_window_with_semantic(cx, &url, Some(browser_db), sem);
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
                                    cx.spawn(async move |_this, cx: &mut AsyncApp| {
                                        cx.update(|cx| {
                                            rover_browser::open_browser_window_with_semantic(
                                                cx,
                                                rover_browser::DEFAULT_URL,
                                                Some(browser_db),
                                                sem,
                                            );
                                        })
                                        .ok();
                                    })
                                    .detach();
                                    return true;
                                }
                            }
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

    fn actions_for_selected(&self) -> Vec<ItemAction> {
        match self.view_mode {
            ViewMode::Main => {
                let o = self.row_offsets();

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

    fn section_header(&self, ix: usize) -> Option<SharedString> {
        let current = self.section_for_index(ix);
        if ix == 0 {
            return Some(SharedString::from(current));
        }
        let prev = self.section_for_index(ix - 1);
        if current != prev {
            Some(SharedString::from(current))
        } else {
            None
        }
    }

    fn primary_action_label(&self) -> SharedString {
        match self.view_mode {
            ViewMode::Main => {
                let o = self.row_offsets();
                if o.has_terminal && self.selected_ix == o.terminal_ix {
                    return "Run Command".into();
                }
                if o.has_url && self.selected_ix == o.url_ix {
                    return "Open in Browser".into();
                }
                if o.has_calc && self.selected_ix == o.calc_ix {
                    return "Copy Result".into();
                }
                let match_ix = self.selected_ix.saturating_sub(o.command_offset);
                if let Some(m) = self.matches.get(match_ix) {
                    if let Some(cmd) = self.all_commands.get(m.candidate_id) {
                        let (_sec, kind_label, _, _, _, _) = Self::classify_command(cmd);
                        return match kind_label {
                            "Window Management" | "Room" | "Niri Strip" => "Manage Window".into(),
                            "Browser" => "Open Browser".into(),
                            "Application" => "Open Application".into(),
                            _ => "Run Command".into(),
                        };
                    }
                }
                "Open".into()
            }
            ViewMode::ClipboardHistory => "Paste".into(),
            ViewMode::FileSearch => "Open File".into(),
        }
    }
}

