mod text_input;

pub use text_input::{bind_input_keys, TextInput, TextInputEvent};

use gpui::prelude::*;
use gpui::*;
use rover_theme::theme;
use std::sync::Arc;

actions!(
    rover_picker,
    [SelectNext, SelectPrev, Confirm, Cancel, ShowActions]
);

pub fn bind_picker_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("up", SelectPrev, Some("Picker")),
        KeyBinding::new("down", SelectNext, Some("Picker")),
        KeyBinding::new("enter", Confirm, Some("Picker")),
        KeyBinding::new("escape", Cancel, Some("Picker")),
        KeyBinding::new("cmd-k", ShowActions, Some("Picker")),
    ]);
    bind_input_keys(cx);
}

pub enum PickerEvent {
    Dismiss,
    OpenSettings,
    ToggleActions,
    OpenTerminal,
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
    fn dismissed(&mut self, cx: &mut Context<Picker<Self>>);

    fn render_match(
        &self,
        ix: usize,
        selected: bool,
        cx: &mut Context<Picker<Self>>,
    ) -> Option<Self::ListItem>;

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

    /// Return sub-actions for the currently selected item (shown on Cmd+K)
    fn actions_for_selected(&self) -> Vec<ItemAction> {
        Vec::new()
    }

    /// Execute a sub-action by index
    fn execute_action(&mut self, _action_ix: usize, _cx: &mut Context<Picker<Self>>) -> bool {
        false
    }

    /// Optional section header label rendered above item `ix` in the Tinycast list
    fn section_header(&self, _ix: usize) -> Option<SharedString> {
        None
    }

    /// Primary action verb shown in the bottom-right Liquid-Glass capsule
    fn primary_action_label(&self) -> SharedString {
        "Open".into()
    }
}

#[derive(Clone)]
pub struct ItemAction {
    pub name: SharedString,
    pub shortcut: Option<SharedString>,
}

// Tinycast Theme Token Ramp (white alpha ramp over dark scrim)
pub const TC_SCRIM: u32 = 0x161618;
pub const TC_PANEL_STROKE: u32 = 0x323238;
pub const TC_SELECTION: u32 = 0x2d2d32; // white 0.10 over scrim
pub const TC_GLASS_BG: u32 = 0x26262b; // Liquid-Glass surface
pub const TC_BORDER: u32 = 0x46464e; // white 0.20 border for KeyCapChips & PopoverMenu
pub const TC_CARD_FILL: u32 = 0x202025; // white 0.05 card fill
pub const TC_CARD_STROKE: u32 = 0x2e2e34; // white 0.10 card stroke
pub const TC_TEXT_PRIMARY: u32 = 0xffffff; // white 1.00
pub const TC_TEXT_SECONDARY: u32 = 0x9a9aa0; // white 0.60
pub const TC_TEXT_TERTIARY: u32 = 0x68686e; // white 0.40

pub fn render_keycap_chip(label: impl Into<SharedString>) -> Div {
    div()
        .h(px(18.0))
        .min_w(px(18.0))
        .px(px(5.0))
        .rounded(px(5.0))
        .bg(rgb(TC_CARD_FILL))
        .border_1()
        .border_color(rgb(TC_BORDER))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(10.5))
        .text_color(rgb(TC_TEXT_SECONDARY))
        .child(label.into())
}

pub fn render_keycap_sequence(keys: &[&str]) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(3.0))
        .children(keys.iter().map(|k| render_keycap_chip(SharedString::from((*k).to_string()))))
}

pub struct Picker<D: PickerDelegate> {
    pub delegate: D,
    pub text_input: Entity<TextInput>,
    pub compact: bool,
    focus_handle: FocusHandle,
    show_actions: bool,
    action_selected_ix: usize,
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
                    picker.delegate.update_matches(text.clone(), cx).detach();
                    cx.notify();
                }
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
                self.action_selected_ix = (self.action_selected_ix + 1) % actions.len();
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
        cx.notify();
    }

    fn handle_select_prev(&mut self, _: &SelectPrev, _window: &mut Window, cx: &mut Context<Self>) {
        if self.show_actions {
            let actions = self.delegate.actions_for_selected();
            if !actions.is_empty() {
                self.action_selected_ix = if self.action_selected_ix == 0 {
                    actions.len() - 1
                } else {
                    self.action_selected_ix - 1
                };
            }
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
}

impl<D: PickerDelegate> Focusable for Picker<D> {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl<D: PickerDelegate> Render for Picker<D> {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let _ = theme(cx);
        let match_count = self.delegate.match_count();
        let query_is_empty = self.text_input.read(cx).text().is_empty();
        let no_matches = self.delegate.no_matches_text();
        let in_submenu = self.delegate.in_submenu();
        let primary_label = self.delegate.primary_action_label();

        let show_items = if self.compact {
            !query_is_empty && match_count > 0
        } else {
            match_count > 0
        };

        // Tinycast floating 44px borderless search header
        let input_row = div()
            .w_full()
            .h(px(44.0))
            .px(px(16.0))
            .flex()
            .items_center()
            .gap(px(12.0))
            .child(if in_submenu {
                div()
                    .w(px(28.0))
                    .h(px(28.0))
                    .rounded(px(8.0))
                    .bg(rgb(TC_CARD_FILL))
                    .border_1()
                    .border_color(rgb(TC_CARD_STROKE))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(rgb(TC_TEXT_SECONDARY))
                    .text_size(px(16.0))
                    .child(SharedString::from("‹"))
            } else {
                div()
                    .w(px(22.0))
                    .h(px(22.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(rgb(TC_TEXT_SECONDARY))
                    .text_size(px(15.0))
                    .child(SharedString::from("⌕"))
            })
            .child(
                div()
                    .flex_grow()
                    .text_color(rgb(TC_TEXT_PRIMARY))
                    .text_size(px(19.0))
                    .line_height(px(24.0))
                    .child(self.text_input.clone()),
            );

        // Full-bleed vertical scrollable LauncherList with section headers
        let list_view = if show_items {
            let mut list_div = div()
                .id("picker-launcher-list")
                .w_full()
                .flex_1()
                .overflow_y_scroll()
                .px(px(8.0))
                .pt(px(4.0))
                .pb(px(10.0))
                .flex()
                .flex_col()
                .gap(px(2.0));

            for ix in 0..match_count {
                if let Some(sec) = self.delegate.section_header(ix) {
                    list_div = list_div.child(
                        div()
                            .w_full()
                            .px(px(10.0))
                            .pt(px(8.0))
                            .pb(px(4.0))
                            .text_size(px(11.5))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(TC_TEXT_SECONDARY))
                            .child(sec),
                    );
                }
                let selected = ix == self.delegate.selected_index();
                if let Some(el) = self.delegate.render_match(ix, selected, cx) {
                    list_div = list_div.child(el);
                }
            }
            Some(list_div)
        } else {
            None
        };

        let mut root = div()
            .id("picker")
            .key_context("Picker")
            .track_focus(&self.focus_handle)
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .justify_between()
            .overflow_hidden()
            .bg(rgb(TC_SCRIM))
            .border_1()
            .border_color(rgb(TC_PANEL_STROKE))
            .rounded(px(26.0))
            .shadow_lg()
            .on_action(cx.listener(Self::handle_select_next))
            .on_action(cx.listener(Self::handle_select_prev))
            .on_action(cx.listener(Self::handle_confirm))
            .on_action(cx.listener(Self::handle_cancel))
            .on_action(cx.listener(Self::handle_show_actions));

        root = root.child(input_row);
        if let Some(list_div) = list_view {
            root = root.child(list_div);
        } else if !query_is_empty {
            root = root.child(
                div()
                    .w_full()
                    .flex_1()
                    .px(px(18.0))
                    .py(px(20.0))
                    .text_size(px(13.5))
                    .text_color(rgb(TC_TEXT_SECONDARY))
                    .child(no_matches),
            );
        }

        // Tinycast floating 52px Liquid-Glass bottom bar:
        // Left: 36x36 circular `=` menu button
        // Right: 36px-high Liquid-Glass Action Capsule (`Open [↵] | Actions [⌘] [K]`)
        let bottom_bar = div()
            .w_full()
            .h(px(52.0))
            .px(px(12.0))
            .pb(px(10.0))
            .pt(px(6.0))
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .w(px(36.0))
                    .h(px(36.0))
                    .rounded(px(18.0))
                    .bg(rgb(TC_GLASS_BG))
                    .border_1()
                    .border_color(rgb(TC_BORDER))
                    .shadow_sm()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px(15.0))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(TC_TEXT_SECONDARY))
                    .child(SharedString::from("=")),
            )
            .child(
                div()
                    .h(px(36.0))
                    .px(px(6.0))
                    .rounded(px(18.0))
                    .bg(rgb(TC_GLASS_BG))
                    .border_1()
                    .border_color(rgb(TC_BORDER))
                    .shadow_sm()
                    .flex()
                    .items_center()
                    .gap(px(2.0))
                    .child(
                        div()
                            .h(px(28.0))
                            .px(px(8.0))
                            .rounded(px(14.0))
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(rgb(TC_TEXT_PRIMARY))
                                    .child(primary_label),
                            )
                            .child(render_keycap_chip("↵")),
                    )
                    .child(div().w(px(1.0)).h(px(14.0)).bg(rgb(TC_BORDER)))
                    .child(
                        div()
                            .h(px(28.0))
                            .px(px(8.0))
                            .rounded(px(14.0))
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(rgb(TC_TEXT_SECONDARY))
                                    .child(SharedString::from("Actions")),
                            )
                            .child(render_keycap_sequence(&["⌘", "K"])),
                    ),
            );

        root = root.child(bottom_bar);

        // Tinycast 284px PopoverMenu floating above the bottom bar
        if self.show_actions {
            let actions = self.delegate.actions_for_selected();
            if !actions.is_empty() {
                root = root.child(
                    div()
                        .absolute()
                        .bottom(px(52.0))
                        .left(px(0.0))
                        .right(px(0.0))
                        .px(px(12.0))
                        .pb(px(4.0))
                        .flex()
                        .justify_end()
                        .child(
                            div()
                                .w(px(284.0))
                                .rounded(px(16.0))
                                .bg(rgb(0x1e1e22))
                                .border_1()
                                .border_color(rgb(TC_BORDER))
                                .shadow_lg()
                                .p(px(6.0))
                                .flex()
                                .flex_col()
                                .gap(px(2.0))
                                .children(actions.iter().enumerate().map(|(ix, action)| {
                                    let selected = ix == self.action_selected_ix;
                                    div()
                                        .px(px(10.0))
                                        .py(px(6.0))
                                        .rounded(px(10.0))
                                        .bg(rgb(if selected { TC_SELECTION } else { 0x1e1e22 }))
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .child(
                                            div()
                                                .text_color(rgb(TC_TEXT_PRIMARY))
                                                .text_size(px(12.5))
                                                .child(action.name.clone()),
                                        )
                                        .when_some(action.shortcut.clone(), |el, shortcut| {
                                            el.child(render_keycap_chip(shortcut))
                                        })
                                }))
                                .child(
                                    div()
                                        .mt(px(4.0))
                                        .pt(px(6.0))
                                        .px(px(10.0))
                                        .pb(px(4.0))
                                        .border_t_1()
                                        .border_color(rgb(TC_CARD_STROKE))
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .child(
                                            div()
                                                .text_size(px(12.0))
                                                .text_color(rgb(TC_TEXT_TERTIARY))
                                                .child(SharedString::from("Search for actions...")),
                                        )
                                        .child(render_keycap_sequence(&["⌘", "K"])),
                                ),
                        ),
                );
            }
        }

        root
    }
}

