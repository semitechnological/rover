mod text_input;

pub use text_input::{bind_input_keys, TextInput, TextInputEvent};

use gpui::prelude::*;
use gpui::*;
use rover_theme::theme;
use std::sync::Arc;
use std::time::Duration;

actions!(
    rover_picker,
    [
        SelectNext,
        SelectPrev,
        SelectLeft,
        SelectRight,
        Confirm,
        ShiftConfirm,
        Cancel,
        ShowActions
    ]
);

pub fn bind_picker_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("up", SelectPrev, Some("Picker")),
        KeyBinding::new("down", SelectNext, Some("Picker")),
        KeyBinding::new("left", SelectLeft, Some("Picker")),
        KeyBinding::new("right", SelectRight, Some("Picker")),
        KeyBinding::new("enter", Confirm, Some("Picker")),
        KeyBinding::new("shift-enter", ShiftConfirm, Some("Picker")),
        KeyBinding::new("escape", Cancel, Some("Picker")),
        KeyBinding::new("cmd-k", ShowActions, Some("Picker")),
    ]);
    bind_input_keys(cx);
}

pub enum PickerEvent {
    Dismiss,
    OpenSettings,
    ToggleActions,
    OpenTerminal { cmd: Option<String> },
    OpenBrowser { url: String },
}

pub trait PickerDelegate: Sized + 'static {
    type ListItem: IntoElement;

    fn match_count(&self) -> usize;
    fn selected_index(&self) -> usize;
    fn set_selected_index(&mut self, ix: usize, cx: &mut Context<Picker<Self>>);

    fn placeholder_text(&self) -> Arc<str>;
    fn no_matches_text(&self) -> SharedString {
        "No matches".into()
    }

    fn update_matches(&mut self, query: String, cx: &mut Context<Picker<Self>>) -> Task<()>;
    /// Returns true if the picker should be dismissed after confirm.
    fn confirm(&mut self, cx: &mut Context<Picker<Self>>) -> bool;
    /// Shift+Enter confirm — e.g. open in new window instead of existing tab.
    /// Default falls back to regular confirm.
    fn shift_confirm(&mut self, cx: &mut Context<Picker<Self>>) -> bool {
        self.confirm(cx)
    }
    fn dismissed(&mut self, cx: &mut Context<Picker<Self>>);

    fn render_match(
        &self,
        ix: usize,
        selected: bool,
        cx: &mut Context<Picker<Self>>,
    ) -> Option<Self::ListItem>;

    /// Render item as a list row (non-compact mode). Falls back to render_match.
    fn render_row(
        &self,
        ix: usize,
        selected: bool,
        cx: &mut Context<Picker<Self>>,
    ) -> Option<Self::ListItem> {
        self.render_match(ix, selected, cx)
    }

    /// Number of context items at the start that should always render as pills
    /// (e.g. browser tabs, clipboard previews). Remaining items render as list rows
    /// in non-compact mode.
    fn context_item_count(&self) -> usize {
        0
    }

    /// Called before handling up/down. Return Some(query) to set the input text
    /// and prevent normal selection (used for query history).
    fn select_history(&mut self, _up: bool, _query: &str) -> Option<String> {
        None
    }

    /// Whether we're in a sub-view that escape should exit first
    fn in_submenu(&self) -> bool {
        false
    }

    /// Exit the current sub-view
    fn exit_submenu(&mut self, _cx: &mut Context<Picker<Self>>) {}

    /// Return AI widget content if an AI response should be displayed as a card.
    fn ai_widget_content(&self) -> Option<&str> {
        None
    }

    /// Return terminal output if inline command output should be displayed.
    fn terminal_output(&self) -> Option<&str> {
        None
    }

    /// Whether an inline terminal command is currently running.
    fn terminal_running(&self) -> bool {
        false
    }

    /// Return sub-actions for the currently selected item (shown on Cmd+K)
    fn actions_for_selected(&self) -> Vec<ItemAction> {
        Vec::new()
    }

    /// Execute a sub-action by index
    fn execute_action(&mut self, _action_ix: usize, _cx: &mut Context<Picker<Self>>) -> bool {
        false
    }
}

#[derive(Clone)]
pub struct ItemAction {
    pub name: SharedString,
    pub shortcut: Option<SharedString>,
}

pub struct Picker<D: PickerDelegate> {
    pub delegate: D,
    pub text_input: Entity<TextInput>,
    pub compact: bool,
    focus_handle: FocusHandle,
    show_actions: bool,
    action_selected_ix: usize,
    compact_scroll_offset: usize,
    list_scroll_handle: ScrollHandle,
    emoji_mode: bool,
}

impl<D: PickerDelegate> EventEmitter<PickerEvent> for Picker<D> {}

impl<D: PickerDelegate> Picker<D> {
    pub fn new(delegate: D, cx: &mut Context<Self>) -> Self {
        let placeholder = delegate.placeholder_text();
        let text_input = cx.new(|cx| TextInput::new(placeholder, cx));

        cx.subscribe(
            &text_input,
            |picker: &mut Self, _input, event: &TextInputEvent, cx| match event {
                TextInputEvent::Changed { text } => {
                    // Handle emoji completion: `:something:` → auto-select and copy
                    if let Some(emoji) = Self::check_emoji_completion(&text) {
                        let emoji_char = emoji.to_string();
                        let emoji_str = emoji_char.clone();
                        // Copy to clipboard and dismiss
                        std::thread::spawn(move || {
                            if let Some(mut cb) = arboard::Clipboard::new().ok() {
                                let _ = cb.set_text(&emoji_str);
                            }
                        });
                        picker.text_input.update(cx, |input, cx| input.clear(cx));
                        cx.emit(PickerEvent::Dismiss);
                        return;
                    }

                    // Enter emoji mode if text starts with `:`
                    picker.emoji_mode = text.starts_with(':');

                    picker.delegate.update_matches(text.clone(), cx).detach();
                    cx.notify();
                },
                TextInputEvent::NavigateLeft => {
                    let count = picker.delegate.match_count();
                    if count == 0 {
                        return;
                    }
                    if picker.compact {
                        let ix = picker.delegate.selected_index().saturating_sub(1);
                        picker.delegate.set_selected_index(ix, cx);
                        picker.adjust_compact_scroll();
                    } else {
                        let ctx = picker.delegate.context_item_count();
                        let ix = picker.delegate.selected_index();
                        if ix > 0 && (ctx == 0 || ix < ctx) {
                            picker.delegate.set_selected_index(ix - 1, cx);
                        } else if ix >= ctx && ctx > 0 {
                            picker
                                .delegate
                                .set_selected_index(ctx.saturating_sub(1), cx);
                        }
                    }
                    cx.notify();
                },
                TextInputEvent::NavigateRight => {
                    let count = picker.delegate.match_count();
                    if count == 0 {
                        return;
                    }
                    if picker.compact {
                        let ix = (picker.delegate.selected_index() + 1).min(count - 1);
                        picker.delegate.set_selected_index(ix, cx);
                        picker.adjust_compact_scroll();
                    } else {
                        let ctx = picker.delegate.context_item_count();
                        let ix = picker.delegate.selected_index();
                        if ix < ctx.saturating_sub(1) {
                            picker.delegate.set_selected_index(ix + 1, cx);
                        } else if ix >= ctx && ix < count - 1 {
                            picker.delegate.set_selected_index(ix + 1, cx);
                        }
                    }
                    cx.notify();
                },
            },
        )
        .detach();

        let mut picker = Self {
            delegate,
            text_input,
            compact: false,
            focus_handle: cx.focus_handle(),
            show_actions: false,
            action_selected_ix: 0,
            compact_scroll_offset: 0,
            list_scroll_handle: ScrollHandle::new(),
            emoji_mode: false,
        };
        picker.delegate.update_matches(String::new(), cx).detach();
        picker
    }

    pub fn clear_and_reset(&mut self, cx: &mut Context<Self>) {
        self.text_input.update(cx, |input, cx| input.clear(cx));
        self.delegate.update_matches(String::new(), cx).detach();
        self.delegate.set_selected_index(0, cx);
        cx.notify();
    }

    pub fn set_query(&self, query: &str, cx: &mut Context<Self>) {
        self.text_input
            .update(cx, |input, cx| input.set_text(query.to_string(), cx));
    }

    pub fn refresh_placeholder(&mut self, cx: &mut Context<Self>) {
        let placeholder = self.delegate.placeholder_text();
        self.text_input
            .update(cx, |input, cx| input.set_placeholder(placeholder, cx));
    }

    fn handle_select_next(&mut self, _: &SelectNext, _window: &mut Window, cx: &mut Context<Self>) {
        if self.show_actions {
            let actions = self.delegate.actions_for_selected();
            if !actions.is_empty() {
                self.action_selected_ix = (self.action_selected_ix + 1).min(actions.len() - 1);
                cx.notify();
            }
            return;
        }
        let query = self.text_input.read(cx).text().to_string();
        if let Some(new_query) = self.delegate.select_history(false, &query) {
            self.set_query(&new_query, cx);
            return;
        }
        let count = self.delegate.match_count();
        if count == 0 {
            return;
        }
        let ix = (self.delegate.selected_index() + 1).min(count - 1);
        self.delegate.set_selected_index(ix, cx);
        self.scroll_to_selected();
        cx.notify();
    }

    fn handle_select_prev(&mut self, _: &SelectPrev, _window: &mut Window, cx: &mut Context<Self>) {
        if self.show_actions {
            self.action_selected_ix = self.action_selected_ix.saturating_sub(1);
            cx.notify();
            return;
        }
        let query = self.text_input.read(cx).text().to_string();
        if let Some(new_query) = self.delegate.select_history(true, &query) {
            self.set_query(&new_query, cx);
            return;
        }
        if self.delegate.match_count() == 0 {
            return;
        }
        let ix = self.delegate.selected_index().saturating_sub(1);
        self.delegate.set_selected_index(ix, cx);
        self.scroll_to_selected();
        cx.notify();
    }

    fn handle_confirm(&mut self, _: &Confirm, _window: &mut Window, cx: &mut Context<Self>) {
        if self.show_actions {
            let should_dismiss = self.delegate.execute_action(self.action_selected_ix, cx);
            self.show_actions = false;
            self.action_selected_ix = 0;
            if should_dismiss {
                self.clear_and_reset(cx);
                cx.emit(PickerEvent::Dismiss);
            } else {
                cx.notify();
            }
            return;
        }
        let should_dismiss = self.delegate.confirm(cx);
        if should_dismiss {
            self.clear_and_reset(cx);
            cx.emit(PickerEvent::Dismiss);
        } else {
            self.refresh_placeholder(cx);
            self.text_input.update(cx, |input, cx| input.clear(cx));
            cx.notify();
        }
    }

    fn handle_shift_confirm(
        &mut self,
        _: &ShiftConfirm,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let should_dismiss = self.delegate.shift_confirm(cx);
        if should_dismiss {
            self.clear_and_reset(cx);
            cx.emit(PickerEvent::Dismiss);
        } else {
            self.refresh_placeholder(cx);
            self.text_input.update(cx, |input, cx| input.clear(cx));
            cx.notify();
        }
    }

    fn handle_cancel(&mut self, _: &Cancel, _window: &mut Window, cx: &mut Context<Self>) {
        if self.show_actions {
            self.show_actions = false;
            self.action_selected_ix = 0;
            cx.notify();
            return;
        }
        if self.delegate.in_submenu() {
            self.delegate.exit_submenu(cx);
            self.refresh_placeholder(cx);
            self.text_input.update(cx, |input, cx| input.clear(cx));
            self.delegate.update_matches(String::new(), cx).detach();
            cx.notify();
            return;
        }
        let has_text = !self.text_input.read(cx).text().is_empty();
        if has_text {
            self.clear_and_reset(cx);
        } else {
            self.delegate.dismissed(cx);
            cx.emit(PickerEvent::Dismiss);
        }
    }

    fn handle_select_left(&mut self, _: &SelectLeft, _window: &mut Window, cx: &mut Context<Self>) {
        if self.compact {
            // In compact mode, left = prev
            if self.delegate.match_count() == 0 {
                return;
            }
            let ix = self.delegate.selected_index().saturating_sub(1);
            self.delegate.set_selected_index(ix, cx);
            self.adjust_compact_scroll();
            cx.notify();
        } else {
            // In non-compact, navigate within context pills only
            let ctx = self.delegate.context_item_count();
            if ctx == 0 {
                return;
            }
            let ix = self.delegate.selected_index();
            if ix < ctx && ix > 0 {
                self.delegate.set_selected_index(ix - 1, cx);
                cx.notify();
            } else if ix >= ctx {
                // Jump to last context pill
                self.delegate.set_selected_index(ctx.saturating_sub(1), cx);
                cx.notify();
            }
        }
    }

    fn handle_select_right(
        &mut self,
        _: &SelectRight,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.compact {
            // In compact mode, right = next
            let count = self.delegate.match_count();
            if count == 0 {
                return;
            }
            let ix = (self.delegate.selected_index() + 1).min(count - 1);
            self.delegate.set_selected_index(ix, cx);
            self.adjust_compact_scroll();
            cx.notify();
        } else {
            // In non-compact, navigate within context pills only
            let ctx = self.delegate.context_item_count();
            if ctx == 0 {
                return;
            }
            let ix = self.delegate.selected_index();
            if ix < ctx.saturating_sub(1) {
                self.delegate.set_selected_index(ix + 1, cx);
                cx.notify();
            }
        }
    }

    fn adjust_compact_scroll(&mut self) {
        let ix = self.delegate.selected_index();
        let visible = 8usize;
        if ix < self.compact_scroll_offset {
            self.compact_scroll_offset = ix;
        } else if ix >= self.compact_scroll_offset + visible {
            self.compact_scroll_offset = ix.saturating_sub(visible - 1);
        }
    }

    fn scroll_to_selected(&self) {
        if !self.compact {
            let ctx = self.delegate.context_item_count();
            let ix = self.delegate.selected_index();
            if ix >= ctx {
                // Scroll to the list row (index relative to list start)
                self.list_scroll_handle.scroll_to_item(ix - ctx);
            }
        }
    }

    fn handle_show_actions(
        &mut self,
        _: &ShowActions,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.show_actions {
            self.show_actions = false;
            self.action_selected_ix = 0;
        } else {
            let actions = self.delegate.actions_for_selected();
            if !actions.is_empty() {
                self.show_actions = true;
                self.action_selected_ix = 0;
            }
        }
        cx.notify();
    }

    /// Check if text matches `:something:` pattern and return the best emoji match.
    /// Uses fuzzy matching for typo tolerance - no hardcoded alias tables.
    fn check_emoji_completion(text: &str) -> Option<&'static emojis::Emoji> {
        use emojis::{get, get_by_shortcode, iter as emoji_iter};

        // Check if text matches `:something:` pattern (at least :x:)
        if !text.starts_with(':') || text.len() < 3 {
            return None;
        }

        // Find the second `:`
        let remaining = &text[1..];
        if let Some(pos) = remaining.find(':') {
            let shortcode = &remaining[..pos];
            if shortcode.is_empty() {
                return None;
            }

            // 1. Try exact shortcode match first
            if let Some(emoji) = get_by_shortcode(shortcode) {
                return Some(emoji);
            }

            // 2. Try direct emoji lookup (for cases like :heart: where heart is the emoji name)
            if let Some(emoji) = get(shortcode) {
                return Some(emoji);
            }

            // 3. Fall back to fuzzy/prefix matching on shortcodes
            let lower_shortcode = shortcode.to_lowercase();
            let mut best_match: Option<(&str, &str)> = None;

            for emoji in emoji_iter() {
                if let Some(sc) = emoji.shortcode() {
                    let sc_lower = sc.to_lowercase();
                    // Check if query is a prefix/substring of shortcode
                    if sc_lower.starts_with(&lower_shortcode) || sc_lower.contains(&lower_shortcode)
                    {
                        match &best_match {
                            None => best_match = Some((sc, emoji.as_str())),
                            Some((_, existing_emoji)) => {
                                if sc_lower.len() < existing_emoji.len() {
                                    best_match = Some((sc, emoji.as_str()));
                                }
                            },
                        }
                    }
                }
            }

            if let Some((_, emoji_str)) = best_match {
                return get(emoji_str);
            }

            None
        } else {
            None
        }
    }
}

impl<D: PickerDelegate> Focusable for Picker<D> {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl<D: PickerDelegate> Render for Picker<D> {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = {
            let t = theme(cx);
            t.colors.clone()
        };
        let match_count = self.delegate.match_count();
        let _query_is_empty = self.text_input.read(cx).text().is_empty();
        let bg = colors.surface_background;
        let elevated_bg = colors.elevated_surface_background;
        let text_muted = colors.text_muted;

        let in_submenu = self.delegate.in_submenu();
        let context_count = self.delegate.context_item_count();
        // Always show items when there are any (compact shows context/clipboard by default)
        let show_items = match_count > 0;

        // Root: fills window, anchors content to bottom.
        // No background — window is transparent, only content elements are visible.
        let mut root = div()
            .id("picker")
            .key_context("Picker")
            .track_focus(&self.focus_handle)
            .size_full()
            .flex()
            .flex_col()
            .justify_end()
            .on_action(cx.listener(Self::handle_select_next))
            .on_action(cx.listener(Self::handle_select_prev))
            .on_action(cx.listener(Self::handle_select_left))
            .on_action(cx.listener(Self::handle_select_right))
            .on_action(cx.listener(Self::handle_confirm))
            .on_action(cx.listener(Self::handle_shift_confirm))
            .on_action(cx.listener(Self::handle_cancel))
            .on_action(cx.listener(Self::handle_show_actions));

        if show_items {
            if self.compact {
                // ── Compact mode: everything as floating pills, single scrollable row ──
                let mut pills_div = div()
                    .id("compact-pills")
                    .flex()
                    .flex_row()
                    .gap(px(4.0))
                    .px_2()
                    .py(px(6.0))
                    .mb(px(6.0))
                    .overflow_x_scroll()
                    .flex_shrink_0();

                let hover_bg = colors.element_hover;
                for ix in 0..match_count {
                    let selected = ix == self.delegate.selected_index();
                    if let Some(el) = self.delegate.render_match(ix, selected, cx) {
                        pills_div = pills_div.child(
                            div()
                                .id(ElementId::Name(SharedString::from(format!("pill-{ix}"))))
                                .rounded_full()
                                .hover(move |s| s.bg(hover_bg))
                                .child(el)
                                .on_click(cx.listener(move |this, _event, _window, cx| {
                                    this.delegate.set_selected_index(ix, cx);
                                    let should_dismiss = this.delegate.confirm(cx);
                                    if should_dismiss {
                                        this.clear_and_reset(cx);
                                        cx.emit(PickerEvent::Dismiss);
                                    } else {
                                        this.refresh_placeholder(cx);
                                        this.text_input.update(cx, |input, cx| input.clear(cx));
                                        cx.notify();
                                    }
                                })),
                        );
                    }
                }

                root = root.child(pills_div.with_animation(
                    "compact-pills-fade",
                    Animation::new(Duration::from_millis(120)).with_easing(ease_in_out),
                    |el, delta| el.opacity(delta),
                ));
            } else {
                // ── Non-compact mode: context items as pills, rest as list rows ──

                // Context pills (browser tabs, etc.) — single scrollable row
                if context_count > 0 {
                    let mut ctx_pills = div()
                        .flex()
                        .flex_row()
                        .gap(px(4.0))
                        .px_2()
                        .py(px(4.0))
                        .mb(px(4.0))
                        .overflow_x_hidden()
                        .flex_shrink_0();

                    let hover_bg = colors.element_hover;
                    for ix in 0..context_count.min(match_count) {
                        let selected = ix == self.delegate.selected_index();
                        if let Some(el) = self.delegate.render_match(ix, selected, cx) {
                            ctx_pills = ctx_pills.child(
                                div()
                                    .id(ElementId::Name(SharedString::from(format!("pill-{ix}"))))
                                    .rounded_full()
                                    .hover(move |s| s.bg(hover_bg))
                                    .child(el)
                                    .on_click(cx.listener(move |this, _event, _window, cx| {
                                        this.delegate.set_selected_index(ix, cx);
                                        let should_dismiss = this.delegate.confirm(cx);
                                        if should_dismiss {
                                            this.clear_and_reset(cx);
                                            cx.emit(PickerEvent::Dismiss);
                                        } else {
                                            this.refresh_placeholder(cx);
                                            this.text_input.update(cx, |input, cx| input.clear(cx));
                                            cx.notify();
                                        }
                                    })),
                            );
                        }
                    }
                    root = root.child(ctx_pills.with_animation(
                        "ctx-pills-fade",
                        Animation::new(Duration::from_millis(120)).with_easing(ease_in_out),
                        |el, delta| el.opacity(delta),
                    ));
                }

                // List rows — vertical list with bg
                let list_start = context_count;
                if list_start < match_count {
                    let hover_bg = colors.element_hover;
                    let mut list_div = div()
                        .id("picker-list")
                        .flex()
                        .flex_col()
                        .gap(px(1.0))
                        .px_1()
                        .py_1()
                        .mb(px(6.0))
                        .bg(bg)
                        .rounded_lg()
                        .shadow_md()
                        .max_h(px(240.0))
                        .overflow_y_scroll()
                        .track_scroll(&self.list_scroll_handle);

                    for ix in list_start..match_count {
                        let selected = ix == self.delegate.selected_index();
                        if let Some(el) = self.delegate.render_row(ix, selected, cx) {
                            list_div = list_div.child(
                                div()
                                    .id(ElementId::Name(SharedString::from(format!("row-{ix}"))))
                                    .rounded_md()
                                    .cursor_pointer()
                                    .hover(move |s| s.bg(hover_bg))
                                    .child(el)
                                    .on_click(cx.listener(move |this, _event, _window, cx| {
                                        this.delegate.set_selected_index(ix, cx);
                                        let should_dismiss = this.delegate.confirm(cx);
                                        if should_dismiss {
                                            this.clear_and_reset(cx);
                                            cx.emit(PickerEvent::Dismiss);
                                        } else {
                                            this.refresh_placeholder(cx);
                                            this.text_input.update(cx, |input, cx| input.clear(cx));
                                            cx.notify();
                                        }
                                    })),
                            );
                        }
                    }
                    root = root.child(list_div.with_animation(
                        "list-fade",
                        Animation::new(Duration::from_millis(150)).with_easing(ease_in_out),
                        |el, delta| el.opacity(delta),
                    ));
                }
            }
        }

        // WU-9: AI response widget — full-width card above the action bar
        if let Some(ai_text) = self.delegate.ai_widget_content() {
            let ai_text = ai_text.to_string();
            let ai_widget = div()
                .id("ai-widget")
                .w_full()
                .px_3()
                .py_2()
                .mb(px(6.0))
                .bg(elevated_bg)
                .rounded_lg()
                .shadow_md()
                .max_h(px(200.0))
                .overflow_y_scroll()
                .on_click(cx.listener(move |_this, _event, _window, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(ai_text.clone()));
                }))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_color(colors.text_accent)
                                .text_size(px(10.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(SharedString::from("AI")),
                        )
                        .child(div().text_color(colors.text).text_size(px(12.0)).child(
                            SharedString::from(
                                self.delegate.ai_widget_content().unwrap_or("").to_string(),
                            ),
                        ))
                        .child(
                            div()
                                .text_color(text_muted)
                                .text_size(px(9.0))
                                .child(SharedString::from("Click to copy")),
                        ),
                );
            root = root.child(ai_widget.with_animation(
                "ai-widget-fade",
                Animation::new(Duration::from_millis(200)).with_easing(ease_in_out),
                |el, delta| el.opacity(delta),
            ));
        }

        // WU-11: Inline terminal output widget
        if let Some(term_output) = self.delegate.terminal_output() {
            let term_widget = div()
                .id("terminal-output")
                .w_full()
                .px_3()
                .py_2()
                .mb(px(6.0))
                .bg(rgba(0x0a0a0aFFu32))
                .rounded_lg()
                .shadow_md()
                .max_h(px(160.0))
                .overflow_y_scroll()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div().flex().items_center().gap_1().child(
                                div()
                                    .text_color(colors.text_accent)
                                    .text_size(px(10.0))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(SharedString::from(
                                        if self.delegate.terminal_running() {
                                            "$ running..."
                                        } else {
                                            "$"
                                        },
                                    )),
                            ),
                        )
                        .child(
                            div()
                                .text_color(colors.text)
                                .text_size(px(11.0))
                                .child(SharedString::from(term_output.to_string())),
                        ),
                );
            root = root.child(term_widget.with_animation(
                "terminal-fade",
                Animation::new(Duration::from_millis(150)).with_easing(ease_in_out),
                |el, delta| el.opacity(delta),
            ));
        }

        // Action bar (floating between items and input bar)
        if self.show_actions {
            let actions = self.delegate.actions_for_selected();
            if !actions.is_empty() {
                root = root.child(
                    div()
                        .mb(px(4.0))
                        .bg(bg)
                        .rounded_lg()
                        .shadow_md()
                        .px_1()
                        .py_1()
                        .flex()
                        .flex_col()
                        .gap(px(2.0))
                        .children(actions.iter().enumerate().map(|(ix, action)| {
                            let selected = ix == self.action_selected_ix;
                            let mut row = div()
                                .px_2()
                                .py(px(4.0))
                                .rounded_md()
                                .flex()
                                .items_center()
                                .justify_between();
                            if selected {
                                row = row.bg(colors.element_selected);
                            }
                            row.child(
                                div()
                                    .text_color(colors.text)
                                    .text_size(px(13.0))
                                    .child(action.name.clone()),
                            )
                            .when(
                                action.shortcut.is_some(),
                                |el: Div| {
                                    let shortcut = action.shortcut.clone().unwrap();
                                    el.child(
                                        div()
                                            .text_color(colors.text_muted)
                                            .text_size(px(11.0))
                                            .child(shortcut),
                                    )
                                },
                            )
                        })),
                );
            }
        }

        // Input bar — always at the bottom, the main visual element. No border.
        // Compact, copilot-style bar.
        let mut input_bar = div()
            .px(px(12.0))
            .py(px(7.0))
            .bg(bg)
            .rounded_full()
            .shadow_lg()
            .flex()
            .items_center()
            .gap(px(6.0))
            .when(in_submenu, |el| {
                el.child(
                    div()
                        .text_color(colors.text_muted)
                        .text_size(px(12.0))
                        .child(SharedString::from("‹ ")),
                )
            })
            .child(
                div()
                    .flex_grow()
                    .text_color(colors.text)
                    .text_size(px(13.0))
                    .line_height(px(20.0))
                    .child(self.text_input.clone()),
            );

        // Subtle hint on the right side of the bar
        if in_submenu {
            input_bar = input_bar.child(
                div()
                    .text_color(text_muted)
                    .text_size(px(9.0))
                    .child(SharedString::from(format!("{match_count}"))),
            );
        }

        root = root.child(input_bar);

        root
    }
}
