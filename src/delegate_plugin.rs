//! Thin `RoverDelegate` that dispatches all logic to the PluginRegistry.
//!
//! This replaces the monolithic `delegate_legacy.rs` with a slim adapter
//! between the `rover_picker::PickerDelegate` trait and the plugin system.

use gpui::prelude::*;
use gpui::*;
use rover_fuzzy::{match_strings, StringMatchCandidate};
use rover_picker::{ItemAction, Picker, PickerDelegate, PickerEvent};
use rover_plugin_api::*;
use rover_plugins::PluginRegistry;
use rover_theme::theme;
use std::sync::Arc;

// ─── View modes ──────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
enum ViewMode {
    Main,
    Submenu { plugin_id: String, title: String },
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
}

// ─── The delegate ────────────────────────────────────────────────────────────

pub struct RoverPluginDelegate {
    registry: PluginRegistry,
    plugin_ctx: PluginContext,
    context_items: Vec<ContextItem>,
    matches: Vec<PickerMatch>,
    selected_ix: usize,
    query: String,
    view_mode: ViewMode,
    /// Cached AI inline content (if any)
    ai_inline_content: Option<String>,
    /// Fuzzy candidates built from all plugin commands
    fuzzy_candidates: Vec<StringMatchCandidate>,
    /// Maps candidate index → (plugin_id, CommandDescriptor)
    fuzzy_command_map: Vec<(String, CommandDescriptor)>,
}

impl RoverPluginDelegate {
    pub fn detect_frontmost_app() -> Option<String> {
        let output = std::process::Command::new("osascript")
            .arg("-e")
            .arg(
                r#"tell application "System Events" to get name of first application process whose frontmost is true"#,
            )
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

    pub fn new(registry: PluginRegistry, plugin_ctx: PluginContext) -> Self {
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
            context_items: Vec::new(),
            matches: Vec::new(),
            selected_ix: 0,
            query: String::new(),
            view_mode: ViewMode::Main,
            ai_inline_content: None,
            fuzzy_candidates,
            fuzzy_command_map,
        };

        // Initial empty query
        delegate.do_query("");
        delegate
    }

    fn do_query(&mut self, query: &str) {
        self.query = query.to_string();
        self.matches.clear();
        self.context_items.clear();

        if let Some(update) = self
            .registry
            .query_input_hook(&self.query, &self.plugin_ctx)
        {
            match update {
                SearchBarUpdate::ReplaceQuery(new_q) => {
                    self.query = new_q;
                }
                SearchBarUpdate::SetPlaceholder(_) => {}
            }
        }

        // Cache AI inline content
        self.ai_inline_content = self
            .registry
            .get("builtin.ai")
            .and_then(|ai| ai.inline_content(&self.plugin_ctx))
            .and_then(|content| match content {
                rover_plugin_api::InlineContent::Text(text) => Some(text),
                _ => None,
            });

        match &self.view_mode {
            ViewMode::Submenu { plugin_id, .. } => {
                let pid = plugin_id.clone();
                let items = self.registry.submenu_items(&pid, query, &self.plugin_ctx);
                self.matches = items.into_iter().map(PickerMatch::SubmenuItem).collect();
            }
            ViewMode::Main => {
                self.context_items = self.registry.context_items(&self.plugin_ctx);
                // 1. Fuzzy match against all registered commands
                let fuzzy_matches = match_strings(&self.fuzzy_candidates, &self.query, 50);
                let has_good_fuzzy = !self.query.is_empty()
                    && fuzzy_matches.first().map(|m| m.score > 0).unwrap_or(false);

                // 2. Query non-FuzzyCommand plugins (AlwaysEvaluate, Prefix, Fallback)
                let plugin_results = self.registry.query(
                    &self.query,
                    has_good_fuzzy,
                    false, // will be updated after we get calc results
                    &self.plugin_ctx,
                );

                // Add plugin results (calc, terminal prefix, AI fallback, etc.)
                for result in plugin_results {
                    self.matches.push(PickerMatch::PluginResult(result));
                }

                // Add fuzzy-matched commands
                for m in fuzzy_matches {
                    if let Some((plugin_id, descriptor)) =
                        self.fuzzy_command_map.get(m.candidate_id)
                    {
                        self.matches.push(PickerMatch::FuzzyCommand {
                            plugin_id: plugin_id.clone(),
                            descriptor: descriptor.clone(),
                            score: m.score as f64,
                        });
                    }
                }

                // Sort: plugin results first (by priority), then fuzzy matches by score
                self.matches.sort_by(|a, b| {
                    // Plugin results before fuzzy commands
                    let a_is_plugin = matches!(a, PickerMatch::PluginResult(_));
                    let b_is_plugin = matches!(b, PickerMatch::PluginResult(_));
                    if a_is_plugin != b_is_plugin {
                        return if a_is_plugin {
                            std::cmp::Ordering::Less
                        } else {
                            std::cmp::Ordering::Greater
                        };
                    }
                    // Then by score descending
                    b.score()
                        .partial_cmp(&a.score())
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
            }
        }

        // Clamp selection
        if self.selected_ix >= self.matches.len() {
            self.selected_ix = 0;
        }
    }

    fn handle_confirm_result(
        &mut self,
        result: ConfirmResult,
        cx: &mut Context<Picker<Self>>,
    ) -> bool {
        match result {
            ConfirmResult::Dismiss => true,
            ConfirmResult::Stay => false,
            ConfirmResult::EnterSubmenu { title } => {
                let match_ix = self.selected_ix.saturating_sub(self.context_items.len());
                let plugin_id = self
                    .matches
                    .get(match_ix)
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
                // Run via sh -c
                let _ = std::process::Command::new("sh").arg("-c").arg(&cmd).spawn();
                true
            }
            ConfirmResult::LaunchApp(name) => {
                let script = format!(
                    r#"tell application "{}" to activate"#,
                    name.replace('"', "\\\"")
                );
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
            ConfirmResult::Custom(_) => false,
        }
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
            ViewMode::Submenu { title, .. } => Arc::from(format!("Search {}...", title).as_str()),
        }
    }

    fn update_matches(&mut self, query: String, cx: &mut Context<Picker<Self>>) -> Task<()> {
        self.do_query(&query);
        cx.notify();
        Task::ready(())
    }

    fn confirm(&mut self, cx: &mut Context<Picker<Self>>) -> bool {
        if matches!(self.view_mode, ViewMode::Main) && self.selected_ix < self.context_items.len() {
            let item = self.context_items[self.selected_ix].clone();
            let result = self.registry.confirm(
                &item.plugin_id,
                &item.id,
                &self.query,
                false,
                &mut self.plugin_ctx,
            );
            return self.handle_confirm_result(result, cx);
        }

        if self.matches.is_empty() {
            return false;
        }

        let match_ix = self.selected_ix.saturating_sub(self.context_items.len());
        let m = self.matches[match_ix].clone();
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
        if matches!(self.view_mode, ViewMode::Main) && self.selected_ix < self.context_items.len() {
            let item = self.context_items[self.selected_ix].clone();
            let result = self.registry.confirm(
                &item.plugin_id,
                &item.id,
                &self.query,
                true,
                &mut self.plugin_ctx,
            );
            return self.handle_confirm_result(result, cx);
        }

        if self.matches.is_empty() {
            return false;
        }

        let match_ix = self.selected_ix.saturating_sub(self.context_items.len());
        let m = self.matches[match_ix].clone();
        let pid = m.plugin_id().to_string();
        let mid = m.id().to_string();
        let result = self
            .registry
            .confirm(&pid, &mid, &self.query, true, &mut self.plugin_ctx);
        self.handle_confirm_result(result, cx)
    }

    fn dismissed(&mut self, _cx: &mut Context<Picker<Self>>) {}

    fn render_match(
        &self,
        ix: usize,
        selected: bool,
        cx: &mut Context<Picker<Self>>,
    ) -> Option<Self::ListItem> {
        if matches!(self.view_mode, ViewMode::Main) && ix < self.context_items.len() {
            let item = &self.context_items[ix];
            let colors = theme(cx).colors.clone();
            let text_color = if selected {
                colors.text_accent
            } else {
                colors.text
            };
            let mut row = div()
                .px_2()
                .py(px(4.0))
                .rounded_full()
                .border_1()
                .flex()
                .items_center()
                .gap_1()
                .cursor_pointer();
            if selected {
                row = row
                    .bg(colors.element_selected)
                    .border_color(colors.text_accent);
            } else if item.is_active {
                row = row
                    .bg(colors.elevated_surface_background)
                    .border_color(colors.border);
            } else {
                row = row
                    .bg(colors.surface_background)
                    .border_color(colors.border_variant);
            }
            return Some(
                row.child(
                    div()
                        .text_color(text_color)
                        .text_size(px(12.0))
                        .child(SharedString::from(item.label.clone())),
                )
                .into_any_element(),
            );
        }

        let match_ix = ix.saturating_sub(self.context_items.len());
        let m = self.matches.get(match_ix)?;
        let colors = theme(cx).colors.clone();

        let text_color = if selected {
            colors.text_accent
        } else {
            colors.text
        };

        let title = m.title().to_string();
        let _subtitle = m.subtitle().map(|s| s.to_string());
        let has_sub = m.has_submenu();

        let mut row = div()
            .px_2()
            .py(px(4.0))
            .rounded_full()
            .border_1()
            .flex()
            .items_center()
            .gap_1()
            .cursor_pointer();

        if selected {
            row = row
                .bg(colors.element_selected)
                .border_color(colors.text_accent);
        } else {
            row = row
                .bg(colors.surface_background)
                .border_color(colors.border_variant);
        }

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
        let match_ix = ix.saturating_sub(self.context_items.len());
        let m = self.matches.get(match_ix)?;
        let colors = theme(cx).colors.clone();

        let row_bg = if selected {
            colors.element_selected
        } else {
            gpui::transparent_black()
        };
        let text_color = if selected {
            colors.text_accent
        } else {
            colors.text
        };

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
        self.ai_inline_content.as_deref()
    }

    fn terminal_output(&self) -> Option<&str> {
        None
    }

    fn terminal_running(&self) -> bool {
        false
    }

    fn actions_for_selected(&self) -> Vec<ItemAction> {
        if self.selected_ix < self.context_items.len() {
            return vec![ItemAction {
                name: SharedString::from("Open"),
                shortcut: Some(SharedString::from("↩")),
            }];
        }
        let match_ix = self.selected_ix.saturating_sub(self.context_items.len());
        if let Some(m) = self.matches.get(match_ix) {
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
        if self.selected_ix < self.context_items.len() && action_ix == 0 {
            let item = self.context_items[self.selected_ix].clone();
            let result = self.registry.confirm(
                &item.plugin_id,
                &item.id,
                &self.query,
                false,
                &mut self.plugin_ctx,
            );
            return self.handle_confirm_result(result, cx);
        }
        let match_ix = self.selected_ix.saturating_sub(self.context_items.len());
        if let Some(m) = self.matches.get(match_ix).cloned() {
            let pid = m.plugin_id().to_string();
            let mid = m.id().to_string();
            if let Some(plugin) = self.registry.get(&pid) {
                let actions = plugin.actions_for_match(&mid);
                if let Some(action) = actions.get(action_ix) {
                    let aid = action.id.clone();
                    if let Some(plugin_mut) = self.registry.get_mut(&pid) {
                        let result = plugin_mut.execute_action(&mid, &aid, &mut self.plugin_ctx);
                        return self.handle_confirm_result(result, cx);
                    }
                }
            }
        }
        false
    }

    fn context_item_count(&self) -> usize {
        if matches!(self.view_mode, ViewMode::Main) {
            self.context_items.len()
        } else {
            0
        }
    }
}
