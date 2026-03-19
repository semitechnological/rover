use gpui::prelude::*;
use gpui::*;
use rover_db::RoverDb;
use rover_theme::{self, theme, ThemeRegistry};
use std::sync::Arc;

pub struct SettingsView {
    db: Arc<RoverDb>,
    selected_theme_ix: usize,
    theme_names: Vec<String>,
}

impl SettingsView {
    pub fn new(db: Arc<RoverDb>, cx: &mut Context<Self>) -> Self {
        let theme_names: Vec<String> = {
            let registry = cx.global::<ThemeRegistry>();
            registry.list().iter().map(|t| t.name.clone()).collect()
        };
        let current = theme(cx).name.clone();
        let selected_theme_ix = theme_names.iter().position(|n| n == &current).unwrap_or(0);
        Self {
            db,
            selected_theme_ix,
            theme_names,
        }
    }
}

impl Render for SettingsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme(cx);
        let colors = &t.colors;

        div()
            .size_full()
            .bg(colors.surface_background)
            .flex()
            .flex_col()
            .p_4()
            .gap_4()
            .child(
                div()
                    .text_color(colors.text)
                    .text_size(px(20.0))
                    .font_weight(FontWeight::BOLD)
                    .child("Rover Settings"),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_color(colors.text)
                            .text_size(px(14.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Theme"),
                    )
                    .child(div().flex().flex_col().gap_1().children(
                        self.theme_names.iter().enumerate().map(|(ix, name)| {
                            let selected = ix == self.selected_theme_ix;
                            let name_clone = name.clone();
                            div()
                                .id(SharedString::from(format!("theme-{ix}")))
                                .px_3()
                                .py(px(6.0))
                                .rounded_md()
                                .cursor_pointer()
                                .when(selected, |el: Stateful<Div>| el.bg(colors.element_selected))
                                .when(!selected, |el: Stateful<Div>| {
                                    el.hover(|s| s.bg(colors.element_hover))
                                })
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            div()
                                                .text_color(if selected {
                                                    colors.text_accent
                                                } else {
                                                    colors.text
                                                })
                                                .text_size(px(14.0))
                                                .child(SharedString::from(name.clone())),
                                        )
                                        .when(selected, |el: Div| {
                                            el.child(
                                                div()
                                                    .text_color(colors.text_accent)
                                                    .text_size(px(12.0))
                                                    .child("✓"),
                                            )
                                        }),
                                )
                                .on_click(cx.listener(move |this, _event, _window, cx| {
                                    this.selected_theme_ix = ix;
                                    let registry = cx.global::<ThemeRegistry>();
                                    if let Some(theme) = registry.list().get(ix) {
                                        rover_theme::set_active_theme(theme.clone(), cx);
                                    }
                                    let _ = this.db.set_setting("theme", &name_clone);
                                    cx.notify();
                                }))
                        }),
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_color(colors.text)
                            .text_size(px(14.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Data"),
                    )
                    .child(
                        div()
                            .id("clear-clipboard")
                            .px_3()
                            .py(px(6.0))
                            .rounded_md()
                            .cursor_pointer()
                            .hover(|el| el.bg(colors.element_hover))
                            .child(
                                div()
                                    .text_color(colors.text)
                                    .text_size(px(14.0))
                                    .child("Clear Clipboard History"),
                            )
                            .on_click(cx.listener(|this, _event, _window, _cx| {
                                let _ = this.db.clear_clipboard_history();
                            })),
                    ),
            )
            .child(
                div().flex_grow().flex().items_end().child(
                    div()
                        .text_color(colors.text_muted)
                        .text_size(px(12.0))
                        .child("Rover v0.1.0"),
                ),
            )
    }
}

pub fn open_settings_window(db: Arc<RoverDb>, cx: &mut App) {
    let bounds = Bounds::centered(None, size(px(480.0), px(400.0)), cx);

    let _window = cx
        .open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Rover Settings".into()),
                    appears_transparent: false,
                    ..Default::default()
                }),
                kind: WindowKind::Normal,
                focus: true,
                is_movable: true,
                is_resizable: true,
                ..Default::default()
            },
            |_window, cx| cx.new(|cx| SettingsView::new(db, cx)),
        )
        .ok();
}
