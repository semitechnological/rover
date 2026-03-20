use gpui::prelude::*;
use gpui::*;
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

impl RoverDelegate {
    pub fn new(
        commands: Vec<Command>,
        db: Arc<RoverDb>,
        ai_session: Option<Arc<rusty_foundationmodels::Session>>,
        ai_enabled: bool,
    ) -> Self {
        let candidates: Vec<StringMatchCandidate> = commands
            .iter()
            .enumerate()
            .map(|(i, c)| StringMatchCandidate::new(i, &c.name))
            .collect();
        let matches = match_strings(&candidates, "", 50);
        Self {
            all_commands: commands,
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

    fn render_calculator_row(
        &self,
        selected: bool,
        colors: &rover_theme::ThemeColors,
    ) -> AnyElement {
        let result = self.calculator_result.as_ref().unwrap();
        let mut row = div().mx_1().px_2().py(px(6.0)).w_full().rounded_md();
        if selected {
            row = row.bg(colors.element_selected);
        }
        row.child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .child(
                    div()
                        .w(px(20.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_color(colors.text_muted)
                        .text_size(px(14.0))
                        .child(SharedString::from("=")),
                )
                .child(
                    div()
                        .text_color(colors.text_accent)
                        .text_size(px(14.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(SharedString::from(result.clone())),
                ),
        )
        .into_any_element()
    }

    fn render_command_row(
        &self,
        command: &Command,
        selected: bool,
        colors: &rover_theme::ThemeColors,
    ) -> AnyElement {
        let mut row = div().mx_1().px_2().py(px(6.0)).w_full().rounded_md();
        if selected {
            row = row.bg(colors.element_selected);
        }
        row.child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .text_color(colors.text)
                                .text_size(px(14.0))
                                .child(SharedString::from(command.name.clone())),
                        )
                        .child(
                            div()
                                .text_color(colors.text_muted)
                                .text_size(px(12.0))
                                .child(SharedString::from(command.description.clone())),
                        ),
                )
                .when(command.has_submenu, |el: Div| {
                    el.child(
                        div()
                            .text_color(colors.text_muted)
                            .text_size(px(12.0))
                            .child(SharedString::from(">")),
                    )
                }),
        )
        .into_any_element()
    }

    fn render_clipboard_row(
        &self,
        entry: &ClipboardEntry,
        selected: bool,
        colors: &rover_theme::ThemeColors,
    ) -> AnyElement {
        let mut row = div().mx_1().px_2().py(px(6.0)).w_full().rounded_md();
        if selected {
            row = row.bg(colors.element_selected);
        }
        let preview: String = entry
            .content
            .chars()
            .take(80)
            .collect::<String>()
            .replace('\n', " \u{21B5} ");
        row.child(
            div().flex().items_center().gap_2().child(
                div()
                    .text_color(colors.text)
                    .text_size(px(13.0))
                    .overflow_hidden()
                    .child(SharedString::from(preview)),
            ),
        )
        .into_any_element()
    }

    fn render_file_row(
        &self,
        file: &file_search::FileResult,
        selected: bool,
        colors: &rover_theme::ThemeColors,
    ) -> AnyElement {
        let mut row = div().mx_1().px_2().py(px(6.0)).w_full().rounded_md();
        if selected {
            row = row.bg(colors.element_selected);
        }
        row.child(
            div().flex().items_center().gap_2().child(
                div()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .text_color(colors.text)
                            .text_size(px(13.0))
                            .child(SharedString::from(file.name.clone())),
                    )
                    .child(
                        div()
                            .text_color(colors.text_muted)
                            .text_size(px(11.0))
                            .child(SharedString::from(file.path.to_string_lossy().to_string())),
                    ),
            ),
        )
        .into_any_element()
    }

    fn render_ai_row(&self, selected: bool, colors: &rover_theme::ThemeColors) -> AnyElement {
        let mut row = div().mx_1().px_2().py(px(6.0)).w_full().rounded_md();
        if selected {
            row = row.bg(colors.element_selected);
        }
        let text = if self.ai_loading {
            "...".to_string()
        } else {
            self.ai_result.clone().unwrap_or_default()
        };
        row.child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .child(
                    div()
                        .px(px(5.0))
                        .py(px(2.0))
                        .rounded(px(4.0))
                        .bg(colors.text_accent.opacity(0.15))
                        .text_color(colors.text_accent)
                        .text_size(px(11.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(SharedString::from("AI")),
                )
                .child(
                    div()
                        .text_color(colors.text)
                        .text_size(px(13.0))
                        .child(SharedString::from(text)),
                ),
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
                let extra = if self.calculator_result.is_some() { 1 } else { 0 };
                let ai_extra = if self.ai_loading || self.ai_result.is_some() { 1 } else { 0 };
                self.matches.len() + extra + ai_extra
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
            ViewMode::Main => Arc::from("Search apps, commands, or type math..."),
            ViewMode::ClipboardHistory => Arc::from("Search clipboard history..."),
            ViewMode::FileSearch => Arc::from("Search for files..."),
        }
    }

    fn update_matches(&mut self, query: String, cx: &mut Context<Picker<Self>>) -> Task<()> {
        self.query = query.clone();
        self.query_history.reset_cursor();

        match self.view_mode {
            ViewMode::Main => {
                let normalized = normalize_query(&query);

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

                // Check for AI trigger prefix
                let is_ai_query = self.ai_enabled
                    && self.ai_session.is_some()
                    && (query.starts_with("? ") || query.starts_with("ai "));

                if is_ai_query {
                    let prompt = if query.starts_with("? ") {
                        query[2..].to_string()
                    } else {
                        query[3..].to_string()
                    };

                    if !prompt.is_empty() {
                        self.ai_result = None;
                        self.ai_loading = true;

                        let session = self.ai_session.clone().unwrap();
                        cx.spawn(async move |this, cx: &mut AsyncApp| {
                            let result = session.respond(&prompt).await;
                            let _ = cx.update(|cx| {
                                let _ = this.update(cx, |picker, cx| {
                                    picker.delegate.ai_loading = false;
                                    picker.delegate.ai_result = Some(match result {
                                        Ok(text) => text,
                                        Err(e) => format!("Error: {e}"),
                                    });
                                    cx.notify();
                                });
                            });
                        })
                        .detach();
                    } else {
                        self.ai_result = None;
                        self.ai_loading = false;
                    }
                } else {
                    self.ai_result = None;
                    self.ai_loading = false;
                }

                let candidates: Vec<StringMatchCandidate> = self
                    .all_commands
                    .iter()
                    .enumerate()
                    .map(|(i, c)| StringMatchCandidate::new(i, &c.name))
                    .collect();

                self.matches = match_strings(&candidates, &normalized, 50);
                self.selected_ix = 0;
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
                let has_calc = self.calculator_result.is_some();
                let has_ai = self.ai_loading || self.ai_result.is_some();
                let ai_ix = if has_calc { 1 } else { 0 };
                let command_offset = (if has_calc { 1 } else { 0 }) + (if has_ai { 1 } else { 0 });

                if has_calc && self.selected_ix == 0 {
                    if let Some(result) = &self.calculator_result {
                        cx.write_to_clipboard(ClipboardItem::new_string(result.clone()));
                    }
                    return false;
                }

                if has_ai && self.selected_ix == ai_ix {
                    if let Some(result) = &self.ai_result {
                        cx.write_to_clipboard(ClipboardItem::new_string(result.clone()));
                    }
                    return false;
                }

                let match_ix = self.selected_ix.saturating_sub(command_offset);

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
                let has_calc = self.calculator_result.is_some();
                let has_ai = self.ai_loading || self.ai_result.is_some();
                let ai_ix = if has_calc { 1 } else { 0 };
                let command_offset = (if has_calc { 1 } else { 0 }) + (if has_ai { 1 } else { 0 });

                if has_calc && self.selected_ix == 0 {
                    return vec![ItemAction {
                        name: "Copy Result".into(),
                        shortcut: Some("\u{21A9}".into()),
                    }];
                }

                if has_ai && self.selected_ix == ai_ix {
                    return vec![ItemAction {
                        name: "Copy Answer".into(),
                        shortcut: Some("\u{21A9}".into()),
                    }];
                }

                let match_ix = self.selected_ix.saturating_sub(command_offset);

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
                let has_calc = self.calculator_result.is_some();
                let has_ai = self.ai_loading || self.ai_result.is_some();
                let ai_ix = if has_calc { 1 } else { 0 };
                let command_offset = (if has_calc { 1 } else { 0 }) + (if has_ai { 1 } else { 0 });

                if has_calc && self.selected_ix == 0 {
                    if let Some(result) = &self.calculator_result {
                        cx.write_to_clipboard(ClipboardItem::new_string(result.clone()));
                    }
                    return false;
                }

                if has_ai && self.selected_ix == ai_ix {
                    if let Some(result) = &self.ai_result {
                        cx.write_to_clipboard(ClipboardItem::new_string(result.clone()));
                    }
                    return false;
                }

                let match_ix = self.selected_ix.saturating_sub(command_offset);

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
                let has_calc = self.calculator_result.is_some();
                let has_ai = self.ai_loading || self.ai_result.is_some();
                let ai_ix = if has_calc { 1 } else { 0 };
                let command_offset = (if has_calc { 1 } else { 0 }) + (if has_ai { 1 } else { 0 });

                if has_calc && ix == 0 {
                    return Some(self.render_calculator_row(selected, colors));
                }

                if has_ai && ix == ai_ix {
                    return Some(self.render_ai_row(selected, colors));
                }

                let match_ix = ix.saturating_sub(command_offset);
                let string_match = self.matches.get(match_ix)?;
                let command = self.all_commands.get(string_match.candidate_id)?;
                Some(self.render_command_row(command, selected, colors))
            }
            ViewMode::ClipboardHistory => {
                let &entry_ix = self.clipboard_filtered.get(ix)?;
                let entry = self.clipboard_entries.get(entry_ix)?;
                Some(self.render_clipboard_row(entry, selected, colors))
            }
            ViewMode::FileSearch => {
                let file = self.file_results.get(ix)?;
                Some(self.render_file_row(file, selected, colors))
            }
        }
    }
}
