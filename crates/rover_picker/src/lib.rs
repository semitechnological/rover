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
}

#[derive(Clone)]
pub struct ItemAction {
    pub name: SharedString,
    pub shortcut: Option<SharedString>,
}

pub struct Picker<D: PickerDelegate> {
    pub delegate: D,
    pub text_input: Entity<TextInput>,
    scroll_handle: UniformListScrollHandle,
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
            scroll_handle: UniformListScrollHandle::new(),
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
        self.scroll_handle.scroll_to_item(ix, ScrollStrategy::Top);
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
        self.scroll_handle.scroll_to_item(ix, ScrollStrategy::Top);
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
        let theme = theme(cx);
        let colors = &theme.colors;
        let match_count = self.delegate.match_count();
        let query_is_empty = self.text_input.read(cx).text().is_empty();

        let no_matches = self.delegate.no_matches_text();
        let bg = colors.surface_background;
        let border_v = colors.border_variant;
        let text_muted = colors.text_muted;

        let in_submenu = self.delegate.in_submenu();

        let mut root = div()
            .id("picker")
            .key_context("Picker")
            .track_focus(&self.focus_handle)
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(bg)
            .rounded_xl()
            .shadow_lg()
            .on_action(cx.listener(Self::handle_select_next))
            .on_action(cx.listener(Self::handle_select_prev))
            .on_action(cx.listener(Self::handle_confirm))
            .on_action(cx.listener(Self::handle_cancel))
            .on_action(cx.listener(Self::handle_show_actions))
            .child(
                div()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(border_v)
                    .flex()
                    .items_center()
                    .gap_2()
                    .when(in_submenu, |el| {
                        el.child(
                            div()
                                .text_color(colors.text_muted)
                                .text_size(px(14.0))
                                .child(SharedString::from("‹ ")),
                        )
                    })
                    .child(
                        div()
                            .flex_grow()
                            .text_color(colors.text)
                            .text_size(px(16.0))
                            .line_height(px(24.0))
                            .child(self.text_input.clone()),
                    ),
            );

        if match_count > 0 {
            root = root.child(
                uniform_list(
                    "picker-list",
                    match_count,
                    cx.processor(
                        |picker: &mut Self,
                         range: std::ops::Range<usize>,
                         _window: &mut Window,
                         cx: &mut Context<Self>| {
                            range
                                .filter_map(|ix| {
                                    let selected = ix == picker.delegate.selected_index();
                                    picker.delegate.render_match(ix, selected, cx)
                                })
                                .map(|el: D::ListItem| el.into_any_element())
                                .collect::<Vec<_>>()
                        },
                    ),
                )
                .flex_grow()
                .px_1()
                .py_1()
                .track_scroll(self.scroll_handle.clone()),
            );
        } else if !query_is_empty {
            root = root.child(div().px_3().py_4().text_color(text_muted).child(no_matches));
        }

        // Action bar overlay
        if self.show_actions {
            let actions = self.delegate.actions_for_selected();
            if !actions.is_empty() {
                root = root.child(
                    div()
                        .border_t_1()
                        .border_color(border_v)
                        .bg(colors.surface_background)
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

        // Footer
        let footer_left = if in_submenu {
            "ESC to go back"
        } else if self.show_actions {
            "ESC to close"
        } else {
            ""
        };

        let footer_right = if self.show_actions {
            SharedString::from("Actions")
        } else if in_submenu {
            SharedString::from(format!("{} items", match_count))
        } else if match_count > 0 {
            SharedString::from("\u{2318}K Actions")
        } else {
            SharedString::default()
        };

        if !footer_left.is_empty() || !footer_right.is_empty() {
            root = root.child(
                div()
                    .px_3()
                    .py_1()
                    .border_t_1()
                    .border_color(border_v)
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_color(colors.text_muted)
                            .text_size(px(11.0))
                            .child(SharedString::from(footer_left)),
                    )
                    .child(
                        div()
                            .text_color(colors.text_muted)
                            .text_size(px(11.0))
                            .child(footer_right),
                    ),
            );
        }

        root
    }
}
