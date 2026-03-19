mod text_input;

pub use text_input::{TextInput, TextInputEvent, bind_input_keys};

use gpui::*;
use rover_theme::theme;
use std::sync::Arc;

actions!(rover_picker, [SelectNext, SelectPrev, Confirm, Cancel]);

pub fn bind_picker_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("up", SelectPrev, Some("Picker")),
        KeyBinding::new("down", SelectNext, Some("Picker")),
        KeyBinding::new("enter", Confirm, Some("Picker")),
        KeyBinding::new("escape", Cancel, Some("Picker")),
    ]);
    bind_input_keys(cx);
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
    fn confirm(&mut self, cx: &mut Context<Picker<Self>>);
    fn dismissed(&mut self, cx: &mut Context<Picker<Self>>);

    fn render_match(
        &self,
        ix: usize,
        selected: bool,
        cx: &mut Context<Picker<Self>>,
    ) -> Option<Self::ListItem>;
}

pub struct Picker<D: PickerDelegate> {
    pub delegate: D,
    pub text_input: Entity<TextInput>,
    scroll_handle: UniformListScrollHandle,
    focus_handle: FocusHandle,
}

impl<D: PickerDelegate> Picker<D> {
    pub fn new(delegate: D, cx: &mut Context<Self>) -> Self {
        let placeholder = delegate.placeholder_text();
        let text_input = cx.new(|cx| TextInput::new(placeholder, cx));

        cx.subscribe(&text_input, |picker: &mut Self, _input, event: &TextInputEvent, cx| {
            match event {
                TextInputEvent::Changed { text } => {
                    picker.delegate.update_matches(text.clone(), cx).detach();
                    cx.notify();
                }
            }
        })
        .detach();

        let mut picker = Self {
            delegate,
            text_input,
            scroll_handle: UniformListScrollHandle::new(),
            focus_handle: cx.focus_handle(),
        };
        picker.delegate.update_matches(String::new(), cx).detach();
        picker
    }

    fn handle_select_next(&mut self, _: &SelectNext, _window: &mut Window, cx: &mut Context<Self>) {
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
        if self.delegate.match_count() == 0 {
            return;
        }
        let ix = self.delegate.selected_index().saturating_sub(1);
        self.delegate.set_selected_index(ix, cx);
        self.scroll_handle.scroll_to_item(ix, ScrollStrategy::Top);
        cx.notify();
    }

    fn handle_confirm(&mut self, _: &Confirm, _window: &mut Window, cx: &mut Context<Self>) {
        self.delegate.confirm(cx);
    }

    fn handle_cancel(&mut self, _: &Cancel, _window: &mut Window, cx: &mut Context<Self>) {
        self.delegate.dismissed(cx);
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
        let border = colors.border;
        let border_v = colors.border_variant;
        let text_muted = colors.text_muted;

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
            .child(
                div()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(border_v)
                    .text_color(colors.text)
                    .text_size(px(16.0))
                    .line_height(px(24.0))
                    .child(self.text_input.clone()),
            );

        if match_count > 0 {
            root = root.child(
                div().flex_1().py_1().child(
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
                    .flex_1()
                    .track_scroll(self.scroll_handle.clone()),
                ),
            );
        } else if !query_is_empty {
            root = root.child(div().px_3().py_4().text_color(text_muted).child(no_matches));
        }

        root
    }
}
