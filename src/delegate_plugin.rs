//! Plugin-based `RoverPluginDelegate` — dispatches all logic to the PluginRegistry.
//!
//! This replaces the monolithic `delegate.rs` with a slim adapter
//! between the `rover_picker::PickerDelegate` trait and the plugin system.

use gpui::prelude::*;
use gpui::*;
use rover_db::RoverDb;
use rover_fuzzy::{match_strings, StringMatchCandidate};
use rover_picker::{ItemAction, Picker, PickerDelegate, PickerEvent};
use rover_plugin_api::*;
use rover_plugins::PluginRegistry;
use rover_theme::theme;
use std::collections::HashMap;
use std::sync::Arc;

// ─── View modes ──────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
enum ViewMode {
    Main,
    Submenu {
        plugin_id: String,
        title: String,
    },
}

// ─── Unified match entry ─────────────────────────────────────────────────────

/// A match shown in the picker — either from plugin query results or from
/// fuzzy-matched plugin commands.
#[derive(Clone, Debug)]
enum PickerMatch {
    /// A result from a plugin's `on_query` (calculator, terminal, etc.)
    PluginResult(MatchResult),
    /// A command descriptor matched by fuzzy search
    FuzzyCommand {
        plugin_id: String,
        descriptor: CommandDescriptor,
        score: f64,
    },
    /// A submenu item
    SubmenuItem(MatchResult),
}

impl PickerMatch {
    fn id(&self) -> &str {
        match self {
            PickerMatch::PluginResult(r) => &r.id,
            PickerMatch::FuzzyCommand { descriptor, .. } => &descriptor.id,
            PickerMatch::SubmenuItem(r) => &r.id,
        }
    }

    fn plugin_id(&self) -> &str {
        match self {
            PickerMatch::PluginResult(r) => &r.plugin_id,
            PickerMatch::FuzzyCommand { plugin_id, .. } => plugin_id,
            PickerMatch::SubmenuItem(r) => &r.plugin_id,
        }
    }

    fn title(&self) -> &str {
        match self {
            PickerMatch::PluginResult(r) => &r.title,
            PickerMatch::FuzzyCommand { descriptor, .. } => &descriptor.name,
            PickerMatch::SubmenuItem(r) => &r.title,
        }
    }

    fn subtitle(&self) -> Option<&str> {
        match self {
            PickerMatch::PluginResult(r) => r.subtitle.as_deref(),
            PickerMatch::FuzzyCommand { descriptor, .. } => Some(&descriptor.description),
            PickerMatch::SubmenuItem(r) => r.subtitle.as_deref(),
        }
    }

    fn has_submenu(&self) -> bool {
        match self {
            PickerMatch::FuzzyCommand { descriptor, .. } => descriptor.has_submenu,
            _ => false,
        }
    }

    fn score(&self) -> f64 {
        match self {
            PickerMatch::PluginResult(r) => r.score,
            PickerMatch::FuzzyCommand { score, .. } => *score,
            PickerMatch::SubmenuItem(r) => r.score,
        }
    }

    fn render_hint(&self) -> Option<&RenderHint> {
        match self {
            PickerMatch::PluginResult(r) => r.render_hint.as_ref(),
            _ => None,
        }
    }

    fn icon_hint(&self) -> Option<&IconHint> {
        match self {
            PickerMatch::PluginResult(r) => r.icon.as_ref(),
            PickerMatch::FuzzyCommand { descriptor, .. } => descriptor.icon.as_ref(),
            PickerMatch::SubmenuItem(r) => r.icon.as_ref(),
        }
    }
}

// ─── The delegate ────────────────────────────────────────────────────────────

pub struct RoverPluginDelegate {
    registry: PluginRegistry,
    plugin_ctx: PluginContext,
    matches: Vec<PickerMatch>,
    selected_ix: usize,
    query: String,
    view_mode: ViewMode,
    /// Fuzzy candidates built from all plugin commands
    fuzzy_candidates: Vec<StringMatchCandidate>,
    /// Maps candidate index → (plugin_id, CommandDescriptor)
    fuzzy_command_map: Vec<(String, CommandDescriptor)>,
    /// Number of context items (browser tabs, etc.) at the start of matches
    context_count: usize,

    // ── DB-backed state ──
    db: Arc<RoverDb>,
    /// Query history for up/down arrow cycling
    query_history: Vec<String>,
    /// Current position in query history
    history_cursor: Option<usize>,
    /// The query that was active before the user started browsing history
    history_stash: String,

    // ── AI async state ──
    ai_session: Option<Arc<rusty_foundationmodels::Session>>,
    ai_debounce: Option<Task<()>>,

    // ── Semantic / help async state ──
    semantic: Option<Arc<std::sync::Mutex<rover_semantic::SemanticIndex>>>,

    // ── Terminal state ──
    terminal_output: Option<String>,
    terminal_running: bool,

    // ── App tracking state ──
    /// The frontmost app before Rover activated
    pub previous_frontmost_app: Option<String>,
    /// Whether the browser was focused before Rover activated
    pub browser_was_focused: bool,
    /// Whether a terminal (Ghostty, etc.) was focused before Rover activated
    pub terminal_was_focused: bool,

    // ── Browser state ──
    live_tabs: Option<rover_browser::LiveBrowserTabsRef>,

    // ── Extension runtimes ──
    ext_runtimes: HashMap<String, rover_extensions::runtime::ExtensionRuntimeRef>,

    // ── Cached inline content ──
    cached_ai_text: Option<String>,
    cached_terminal_output: Option<(String, bool)>,
}

impl RoverPluginDelegate {
    pub fn new(
        registry: PluginRegistry,
        db: Arc<RoverDb>,
        ai_session: Option<Arc<rusty_foundationmodels::Session>>,
        ai_enabled: bool,
        semantic: Option<Arc<std::sync::Mutex<rover_semantic::SemanticIndex>>>,
        live_tabs: Option<rover_browser::LiveBrowserTabsRef>,
        ext_runtimes: HashMap<String, rover_extensions::runtime::ExtensionRuntimeRef>,
        previous_app: Option<String>,
    ) -> Self {
        // Build plugin context with extras
        let mut extras: HashMap<String, Arc<dyn std::any::Any + Send + Sync>> = HashMap::new();
        if let Some(ref session) = ai_session {
            extras.insert("ai_session".to_string(), session.clone());
        }
        if let Some(ref tabs) = live_tabs {
            extras.insert("live_tabs".to_string(), tabs.clone());
        }
        if let Some(ref sem) = semantic {
            extras.insert("semantic_index".to_string(), sem.clone());
        }

        // Use the pre-computed frontmost app from the background poller (avoids a blocking osascript call)
        let previous_frontmost_app = previous_app;
        let terminal_was_focused = previous_frontmost_app
            .as_ref()
            .map(|n| n == "Ghostty" || n == "Terminal" || n == "iTerm2" || n == "Alacritty" || n == "kitty" || n == "WezTerm")
            .unwrap_or(false);
        let browser_was_focused = previous_frontmost_app
            .as_ref()
            .map(|n| n == "rover" || n == "Rover")
            .unwrap_or(false);

        let plugin_ctx = PluginContext {
            db: None,
            config: PluginConfig::default(),
            frontmost_app: previous_frontmost_app.clone(),
            terminal_was_focused,
            extras,
        };

        // Load query history from DB
        let query_history = db
            .list_query_history()
            .unwrap_or_default();

        // Build fuzzy candidates from all plugin commands
        let all_commands = registry.all_commands();
        let fuzzy_candidates: Vec<StringMatchCandidate> = all_commands
            .iter()
            .enumerate()
            .map(|(i, (_, cmd))| StringMatchCandidate::new(i, &cmd.name))
            .collect();
        let fuzzy_command_map: Vec<(String, CommandDescriptor)> = all_commands;

        let mut delegate = Self {
            registry,
            plugin_ctx,
            matches: Vec::new(),
            selected_ix: 0,
            query: String::new(),
            view_mode: ViewMode::Main,
            fuzzy_candidates,
            fuzzy_command_map,
            context_count: 0,
            db,
            query_history,
            history_cursor: None,
            history_stash: String::new(),
            ai_session,
            ai_debounce: None,
            semantic,
            terminal_output: None,
            terminal_running: false,
            previous_frontmost_app,
            browser_was_focused,
            terminal_was_focused,
            live_tabs,
            ext_runtimes,
            cached_ai_text: None,
            cached_terminal_output: None,
        };

        // Initial empty query
        delegate.do_query("");
        delegate
    }

    /// Detect the frontmost app BEFORE Rover activates.
    pub fn detect_frontmost_app() -> Option<String> {
        let output = std::process::Command::new("osascript")
            .arg("-e")
            .arg(r#"tell application "System Events" to get name of first application process whose frontmost is true"#)
            .output()
            .ok()?;
        if output.status.success() {
            let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !name.is_empty() && name != "rover" && name != "Rover" {
                Some(name)
            } else {
                None
            }
        } else {
            None
        }
    }

    fn do_query(&mut self, query: &str) {
        self.query = query.to_string();
        self.matches.clear();
        self.context_count = 0;

        // Reset history browsing when the user types
        self.history_cursor = None;

        // Update plugin context with current state
        self.plugin_ctx.frontmost_app = self.previous_frontmost_app.clone();
        self.plugin_ctx.terminal_was_focused = self.terminal_was_focused;

        match &self.view_mode {
            ViewMode::Submenu { plugin_id, .. } => {
                let pid = plugin_id.clone();
                let items = self.registry.submenu_items(&pid, query, &self.plugin_ctx);
                self.matches = items.into_iter().map(PickerMatch::SubmenuItem).collect();
            }
            ViewMode::Main => {
                // 1. Fuzzy match against all registered commands
                let fuzzy_matches = match_strings(&self.fuzzy_candidates, query, 50);
                let has_good_fuzzy = !query.is_empty()
                    && fuzzy_matches
                        .first()
                        .map(|m| m.score > 0)
                        .unwrap_or(false);

                // 2. Query non-FuzzyCommand plugins (AlwaysEvaluate, Prefix, Fallback).
                // Pass has_calc_result=false first; if the calculator fires we filter
                // Fallback results out directly rather than re-running the entire query.
                let plugin_results = self.registry.query(
                    query,
                    has_good_fuzzy,
                    false,
                    &self.plugin_ctx,
                );
                let has_calc_result = plugin_results
                    .iter()
                    .any(|r| r.plugin_id == "builtin.calculator");

                // Filter Fallback results when calc has a result, avoiding a second query.
                let final_plugin_results: Vec<MatchResult> = if has_calc_result {
                    plugin_results.into_iter()
                        .filter(|r| !matches!(
                            self.registry.get(&r.plugin_id).map(|p| p.match_mode()),
                            Some(MatchMode::Fallback { .. })
                        ))
                        .collect()
                } else {
                    plugin_results
                };

                // Add plugin results (calc, terminal, search engine, AI, quit, etc.)
                for result in final_plugin_results {
                    self.matches.push(PickerMatch::PluginResult(result));
                }

                // Add fuzzy-matched commands
                for m in fuzzy_matches {
                    if let Some((plugin_id, descriptor)) = self.fuzzy_command_map.get(m.candidate_id) {
                        self.matches.push(PickerMatch::FuzzyCommand {
                            plugin_id: plugin_id.clone(),
                            descriptor: descriptor.clone(),
                            score: m.score as f64,
                        });
                    }
                }

                // Sort: plugin results first (by priority), then fuzzy matches by score
                self.matches.sort_by(|a, b| {
                    let a_is_plugin = matches!(a, PickerMatch::PluginResult(_));
                    let b_is_plugin = matches!(b, PickerMatch::PluginResult(_));
                    if a_is_plugin != b_is_plugin {
                        return if a_is_plugin {
                            std::cmp::Ordering::Less
                        } else {
                            std::cmp::Ordering::Greater
                        };
                    }
                    b.score().partial_cmp(&a.score()).unwrap_or(std::cmp::Ordering::Equal)
                });
            }
        }

        // Update cached inline content
        self.update_inline_caches();

        // Clamp selection
        if self.selected_ix >= self.matches.len() {
            self.selected_ix = 0;
        }
    }

    fn update_inline_caches(&mut self) {
        // Cache AI inline content
        self.cached_ai_text = None;
        if let Some(ai) = self.registry.get("builtin.ai") {
            if let Some(InlineContent::Text(text)) = ai.inline_content(&self.plugin_ctx) {
                self.cached_ai_text = Some(text);
            }
        }
        // Also check help plugin
        if self.cached_ai_text.is_none() {
            if let Some(help) = self.registry.get("builtin.help") {
                if let Some(InlineContent::Text(text)) = help.inline_content(&self.plugin_ctx) {
                    self.cached_ai_text = Some(text);
                }
            }
        }

        // Cache terminal inline content
        self.cached_terminal_output = None;
        if let Some(term) = self.registry.get("builtin.terminal") {
            match term.inline_content(&self.plugin_ctx) {
                Some(InlineContent::TerminalOutput { output, running }) => {
                    self.cached_terminal_output = Some((output, running));
                }
                _ => {}
            }
        }
    }

    fn handle_confirm_result(
        &mut self,
        result: ConfirmResult,
        cx: &mut Context<Picker<Self>>,
    ) -> bool {
        match result {
            ConfirmResult::Dismiss => true,
            ConfirmResult::Stay => {
                // Check if AI plugin started loading — spawn async task
                self.maybe_spawn_ai(cx);
                self.maybe_spawn_help(cx);
                cx.notify();
                false
            }
            ConfirmResult::EnterSubmenu { title } => {
                let plugin_id = self
                    .matches
                    .get(self.selected_ix)
                    .map(|m| m.plugin_id().to_string())
                    .unwrap_or_default();
                self.view_mode = ViewMode::Submenu { plugin_id, title };
                self.selected_ix = 0;
                self.query.clear();
                self.do_query("");
                cx.notify();
                false
            }
            ConfirmResult::OpenUrl(url) => {
                cx.emit(PickerEvent::OpenBrowser { url });
                true
            }
            ConfirmResult::RunShell(cmd) => {
                let _ = std::process::Command::new("sh").arg("-c").arg(&cmd).spawn();
                true
            }
            ConfirmResult::LaunchApp(name) => {
                let safe = crate::tools::escape_applescript(&name);
                let script = format!(r#"tell application "{safe}" to activate"#);
                let _ = std::process::Command::new("osascript")
                    .arg("-e")
                    .arg(&script)
                    .spawn();
                true
            }
            ConfirmResult::OpenWindow(req) => {
                match req {
                    WindowRequest::Browser { url } => {
                        cx.emit(PickerEvent::OpenBrowser {
                            url: url.unwrap_or_else(|| "https://start.duckduckgo.com".to_string()),
                        });
                    }
                    WindowRequest::Terminal { cmd } => {
                        cx.emit(PickerEvent::OpenTerminal { cmd });
                    }
                    WindowRequest::Settings => {
                        cx.emit(PickerEvent::OpenSettings);
                    }
                    _ => {}
                }
                true
            }
            ConfirmResult::CopyToClipboard(text) => {
                cx.write_to_clipboard(ClipboardItem::new_string(text));
                true
            }
            ConfirmResult::OpenTerminal(cmd) => {
                cx.emit(PickerEvent::OpenTerminal { cmd });
                true
            }
            ConfirmResult::OpenSettings => {
                cx.emit(PickerEvent::OpenSettings);
                true
            }
            ConfirmResult::QuitApp(name) => {
                let safe = crate::tools::escape_applescript(&name);
                let script = format!(r#"tell application "{safe}" to quit"#);
                let _ = std::process::Command::new("osascript")
                    .arg("-e")
                    .arg(&script)
                    .spawn();
                true
            }
            ConfirmResult::Custom(_) => false,
        }
    }

    /// If the AI plugin is loading, spawn an async task to get the response.
    fn maybe_spawn_ai(&mut self, cx: &mut Context<Picker<Self>>) {
        let ai_loading = self
            .registry
            .get("builtin.ai")
            .and_then(|p| {
                let ai = p.as_any().downcast_ref::<rover_plugins::builtin::ai::AiPlugin>()?;
                if ai.is_loading() { Some(ai.last_query().to_string()) } else { None }
            });

        if let Some(query) = ai_loading {
            if let Some(session) = self.ai_session.clone() {
                self.ai_debounce = Some(cx.spawn(async move |this, cx: &mut AsyncApp| {
                    let result = session.respond(&query).await;
                    let _ = cx.update(|cx| {
                        let _ = this.update(cx, |picker, cx| {
                            if let Some(ai) = picker.delegate.registry.get_mut("builtin.ai") {
                                if let Some(ai_plugin) = ai.as_any_mut().downcast_mut::<rover_plugins::builtin::ai::AiPlugin>() {
                                    ai_plugin.set_result(Some(match result {
                                        Ok(text) => text,
                                        Err(e) => format!("Error: {e}"),
                                    }));
                                }
                            }
                            picker.delegate.update_inline_caches();
                            cx.notify();
                        });
                    });
                }));
            }
        }
    }

    /// If the help plugin is loading, spawn async semantic search + AI.
    fn maybe_spawn_help(&mut self, cx: &mut Context<Picker<Self>>) {
        let help_loading = self
            .registry
            .get("builtin.help")
            .and_then(|p| {
                let help = p.as_any().downcast_ref::<rover_plugins::builtin::help::HelpPlugin>()?;
                if help.is_loading() { Some(help.last_query().to_string()) } else { None }
            });

        if let Some(topic) = help_loading {
            let semantic = self.semantic.clone();
            let session = self.ai_session.clone();
            cx.spawn(async move |this, cx: &mut AsyncApp| {
                // Vector search for relevant docs
                let context = if let Some(sem) = &semantic {
                    let idx = sem.lock().unwrap();
                    idx.search(&topic, None, 6)
                        .unwrap_or_default()
                        .into_iter()
                        .map(|r| {
                            if r.collection == "pages" {
                                let url = r
                                    .metadata
                                    .get("url")
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
                    context
                } else {
                    "No help available for this topic.".to_string()
                };

                let _ = cx.update(|cx| {
                    let _ = this.update(cx, |picker, cx| {
                        if let Some(help) = picker.delegate.registry.get_mut("builtin.help") {
                            if let Some(help_plugin) = help.as_any_mut().downcast_mut::<rover_plugins::builtin::help::HelpPlugin>() {
                                help_plugin.set_result(Some(answer));
                            }
                        }
                        picker.delegate.update_inline_caches();
                        cx.notify();
                    });
                });
            })
            .detach();
        }
    }

    // ── Rendering helpers ────────────────────────────────────────────────

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

    fn state_pill_color(color_name: &str) -> (Rgba, Hsla) {
        match color_name {
            "blue" => (
                rgba(0x1e3a5fcc),
                hsla(210.0 / 360.0, 0.7, 0.65, 1.0),
            ),
            "green" => (
                rgba(0x1a3d1acc),
                hsla(120.0 / 360.0, 0.6, 0.55, 1.0),
            ),
            "red" => (
                rgba(0x5f1e1ecc),
                hsla(0.0, 0.7, 0.6, 1.0),
            ),
            "purple" => (
                rgba(0x3d1a5fcc),
                hsla(270.0 / 360.0, 0.6, 0.65, 1.0),
            ),
            "gray" | _ => (
                rgba(0x333333cc),
                hsla(0.0, 0.0, 0.6, 1.0),
            ),
        }
    }

    fn render_state_pill(
        label: &str,
        color: &str,
        selected: bool,
        colors: &rover_theme::ThemeColors,
    ) -> Div {
        let (bg, text_color) = Self::state_pill_color(color);
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
            p = p.bg(bg).border_color(colors.border_variant);
        }
        p.child(
            div()
                .text_color(text_color)
                .text_size(px(10.0))
                .font_weight(FontWeight::SEMIBOLD)
                .child(SharedString::from(label.to_string())),
        )
    }
}

// ─── PickerDelegate impl ────────────────────────────────────────────────────

impl PickerDelegate for RoverPluginDelegate {
    type ListItem = AnyElement;

    fn match_count(&self) -> usize {
        self.matches.len()
    }

    fn selected_index(&self) -> usize {
        self.selected_ix
    }

    fn set_selected_index(&mut self, ix: usize, _cx: &mut Context<Picker<Self>>) {
        self.selected_ix = ix;
    }

    fn placeholder_text(&self) -> Arc<str> {
        match &self.view_mode {
            ViewMode::Main => Arc::from("Type to search..."),
            ViewMode::Submenu { title, .. } => {
                Arc::from(format!("Search {}...", title).as_str())
            }
        }
    }

    fn update_matches(&mut self, query: String, cx: &mut Context<Picker<Self>>) -> Task<()> {
        self.do_query(&query);
        cx.notify();
        Task::ready(())
    }

    fn confirm(&mut self, cx: &mut Context<Picker<Self>>) -> bool {
        if self.matches.is_empty() {
            return false;
        }

        // Save non-empty queries to DB-backed history
        let query = self.query.clone();
        if !query.trim().is_empty() {
            let _ = self.db.add_query(&query);
            self.query_history.retain(|q| q != &query);
            self.query_history.push(query);
            if self.query_history.len() > 100 {
                self.query_history.remove(0);
            }
        }

        let m = self.matches[self.selected_ix].clone();
        let result = match &self.view_mode {
            ViewMode::Submenu { plugin_id, .. } => {
                let pid = plugin_id.clone();
                self.registry
                    .submenu_confirm(&pid, m.id(), &self.query, &mut self.plugin_ctx)
            }
            ViewMode::Main => {
                let pid = m.plugin_id().to_string();
                let mid = m.id().to_string();
                self.registry
                    .confirm(&pid, &mid, &self.query, false, &mut self.plugin_ctx)
            }
        };

        self.handle_confirm_result(result, cx)
    }

    fn shift_confirm(&mut self, cx: &mut Context<Picker<Self>>) -> bool {
        if self.matches.is_empty() {
            return false;
        }

        let m = self.matches[self.selected_ix].clone();
        let pid = m.plugin_id().to_string();
        let mid = m.id().to_string();
        let result =
            self.registry
                .confirm(&pid, &mid, &self.query, true, &mut self.plugin_ctx);
        self.handle_confirm_result(result, cx)
    }

    fn dismissed(&mut self, _cx: &mut Context<Picker<Self>>) {}

    fn no_matches_text(&self) -> SharedString {
        match &self.view_mode {
            ViewMode::Main => "No matches".into(),
            ViewMode::Submenu { title, .. } => {
                SharedString::from(format!("No {} found", title.to_lowercase()))
            }
        }
    }

    fn context_item_count(&self) -> usize {
        self.context_count
    }

    fn select_history(&mut self, up: bool, query: &str) -> Option<String> {
        if self.query_history.is_empty() {
            return None;
        }

        match self.history_cursor {
            None => {
                if up {
                    self.history_stash = query.to_string();
                    let ix = self.query_history.len() - 1;
                    self.history_cursor = Some(ix);
                    Some(self.query_history[ix].clone())
                } else {
                    None
                }
            }
            Some(ix) => {
                if up {
                    if ix > 0 {
                        let new_ix = ix - 1;
                        self.history_cursor = Some(new_ix);
                        Some(self.query_history[new_ix].clone())
                    } else {
                        None
                    }
                } else if ix + 1 < self.query_history.len() {
                    let new_ix = ix + 1;
                    self.history_cursor = Some(new_ix);
                    Some(self.query_history[new_ix].clone())
                } else {
                    self.history_cursor = None;
                    Some(self.history_stash.clone())
                }
            }
        }
    }

    fn render_match(
        &self,
        ix: usize,
        selected: bool,
        cx: &mut Context<Picker<Self>>,
    ) -> Option<Self::ListItem> {
        let m = self.matches.get(ix)?;
        let colors = theme(cx).colors.clone();

        let title = m.title().to_string();

        // Check for StatePill render hint
        if let Some(RenderHint::StatePill { label, color }) = m.render_hint() {
            let mut pill = Self::render_state_pill(label, color, selected, &colors);
            pill = pill.child(
                div()
                    .text_color(if selected { colors.text_accent } else { colors.text })
                    .text_size(px(12.0))
                    .child(SharedString::from(title)),
            );
            return Some(pill.into_any_element());
        }

        // Default pill rendering
        let text_color = if selected { colors.text_accent } else { colors.text };
        let has_sub = m.has_submenu();

        let mut row = Self::pill(selected, &colors);
        row = row.child(
            div()
                .text_color(text_color)
                .text_size(px(12.0))
                .child(SharedString::from(title)),
        );

        if has_sub {
            row = row.child(
                div()
                    .text_color(colors.text_muted)
                    .text_size(px(10.0))
                    .child(SharedString::from(">")),
            );
        }

        Some(row.into_any_element())
    }

    fn render_row(
        &self,
        ix: usize,
        selected: bool,
        cx: &mut Context<Picker<Self>>,
    ) -> Option<Self::ListItem> {
        let m = self.matches.get(ix)?;
        let colors = theme(cx).colors.clone();

        let row_bg = if selected {
            colors.element_selected
        } else {
            gpui::transparent_black()
        };
        let text_color = if selected { colors.text_accent } else { colors.text };

        let title = m.title().to_string();
        let sub_text = m.subtitle().map(|s| s.to_string());
        let has_sub = m.has_submenu();

        let mut row = div()
            .px_2()
            .py(px(5.0))
            .rounded_md()
            .bg(row_bg)
            .flex()
            .items_center()
            .justify_between()
            .cursor_pointer();

        let mut left = div().flex().items_center().gap_2();

        // Render state pill badge if present
        if let Some(RenderHint::StatePill { label, color }) = m.render_hint() {
            let (_, pill_text_color) = Self::state_pill_color(color);
            left = left.child(
                div()
                    .text_color(pill_text_color)
                    .text_size(px(10.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(SharedString::from(label.clone())),
            );
        }

        left = left.child(
            div()
                .text_color(text_color)
                .text_size(px(13.0))
                .child(SharedString::from(title)),
        );
        if let Some(sub) = sub_text {
            left = left.child(
                div()
                    .text_color(colors.text_muted)
                    .text_size(px(11.0))
                    .child(SharedString::from(sub)),
            );
        }
        row = row.child(left);

        if has_sub {
            row = row.child(
                div()
                    .text_color(colors.text_muted)
                    .text_size(px(10.0))
                    .child(SharedString::from(">")),
            );
        }

        Some(row.into_any_element())
    }

    fn in_submenu(&self) -> bool {
        matches!(self.view_mode, ViewMode::Submenu { .. })
    }

    fn exit_submenu(&mut self, cx: &mut Context<Picker<Self>>) {
        self.view_mode = ViewMode::Main;
        self.selected_ix = 0;
        self.query.clear();
        self.do_query("");
        cx.notify();
    }

    fn ai_widget_content(&self) -> Option<&str> {
        self.cached_ai_text.as_deref()
    }

    fn terminal_output(&self) -> Option<&str> {
        self.cached_terminal_output
            .as_ref()
            .map(|(output, _)| output.as_str())
    }

    fn terminal_running(&self) -> bool {
        self.cached_terminal_output
            .as_ref()
            .map(|(_, running)| *running)
            .unwrap_or(false)
    }

    fn actions_for_selected(&self) -> Vec<ItemAction> {
        if let Some(m) = self.matches.get(self.selected_ix) {
            let pid = m.plugin_id();
            let mid = m.id();
            if let Some(plugin) = self.registry.get(pid) {
                return plugin
                    .actions_for_match(mid)
                    .into_iter()
                    .map(|a| ItemAction {
                        name: SharedString::from(a.name),
                        shortcut: a.shortcut.map(SharedString::from),
                    })
                    .collect();
            }
        }
        vec![]
    }

    fn execute_action(&mut self, action_ix: usize, cx: &mut Context<Picker<Self>>) -> bool {
        if let Some(m) = self.matches.get(self.selected_ix).cloned() {
            let pid = m.plugin_id().to_string();
            let mid = m.id().to_string();
            if let Some(plugin) = self.registry.get(&pid) {
                let actions = plugin.actions_for_match(&mid);
                if let Some(action) = actions.get(action_ix) {
                    let aid = action.id.clone();
                    if let Some(plugin_mut) = self.registry.get_mut(&pid) {
                        let result =
                            plugin_mut.execute_action(&mid, &aid, &mut self.plugin_ctx);
                        return self.handle_confirm_result(result, cx);
                    }
                }
            }
        }
        false
    }
}
