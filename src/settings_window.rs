use gpui::prelude::*;
use gpui::*;
use rover_db::RoverDb;
use rover_theme::{self, theme, ThemeRegistry};
use std::sync::Arc;

use crate::hotkey;

pub struct SettingsView {
    db: Arc<RoverDb>,
    selected_theme_ix: usize,
    theme_names: Vec<String>,
    active_section: SettingsSection,
    hyperkey_enabled: bool,
    show_recent_apps: bool,
    launch_at_login: bool,
    ai_enabled: bool,
    ai_terminal_enabled: bool,
    picker_position: String,
}

#[derive(Clone, Copy, PartialEq)]
pub enum SettingsSection {
    General,
    Appearance,
    Hotkeys,
    AI,
    Data,
    About,
}

impl SettingsSection {
    fn all() -> &'static [SettingsSection] {
        &[
            Self::General,
            Self::Appearance,
            Self::Hotkeys,
            Self::AI,
            Self::Data,
            Self::About,
        ]
    }

    fn label(&self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Appearance => "Appearance",
            Self::Hotkeys => "Hotkeys",
            Self::AI => "AI",
            Self::Data => "Data",
            Self::About => "About",
        }
    }
}

impl SettingsView {
    pub fn new(db: Arc<RoverDb>, initial_section: SettingsSection, cx: &mut Context<Self>) -> Self {
        let theme_names: Vec<String> = {
            let registry = cx.global::<ThemeRegistry>();
            registry.list().iter().map(|t| t.name.clone()).collect()
        };
        let current = theme(cx).name.clone();
        let selected_theme_ix = theme_names.iter().position(|n| n == &current).unwrap_or(0);
        let hyperkey_enabled = db
            .get_setting("hyperkey_enabled")
            .ok()
            .flatten()
            .map(|v| v == "true")
            .unwrap_or(false);
        let show_recent_apps = db
            .get_setting("show_recent_apps")
            .ok()
            .flatten()
            .map(|v| v != "false")
            .unwrap_or(true);
        let launch_at_login = db
            .get_setting("launch_at_login")
            .ok()
            .flatten()
            .map(|v| v == "true")
            .unwrap_or(false);
        let ai_enabled = db
            .get_setting("ai_enabled")
            .ok()
            .flatten()
            .map(|v| v == "true")
            .unwrap_or(false);
        let ai_terminal_enabled = db
            .get_setting("ai_terminal_enabled")
            .ok()
            .flatten()
            .map(|v| v == "true")
            .unwrap_or(false);
        let picker_position = db
            .get_setting("picker_position")
            .ok()
            .flatten()
            .unwrap_or_else(|| "center".to_string());

        Self {
            db,
            selected_theme_ix,
            theme_names,
            active_section: initial_section,
            hyperkey_enabled,
            show_recent_apps,
            launch_at_login,
            ai_enabled,
            ai_terminal_enabled,
            picker_position,
        }
    }

    fn section_header(text: &str, colors: &rover_theme::ThemeColors) -> Div {
        div()
            .pb(px(12.0))
            .mb(px(4.0))
            .border_b_1()
            .border_color(colors.border_variant)
            .child(
                div()
                    .text_color(colors.text)
                    .text_size(px(16.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(SharedString::from(text.to_string())),
            )
    }

    fn setting_row(colors: &rover_theme::ThemeColors) -> Div {
        div()
            .flex()
            .items_center()
            .justify_between()
            .py(px(10.0))
            .border_b_1()
            .border_color(colors.border_variant.opacity(0.5))
    }

    fn setting_label(title: &str, description: &str, colors: &rover_theme::ThemeColors) -> Div {
        div()
            .flex()
            .flex_col()
            .gap(px(2.0))
            .child(
                div()
                    .text_color(colors.text)
                    .text_size(px(13.0))
                    .child(SharedString::from(title.to_string())),
            )
            .child(
                div()
                    .text_color(colors.text_muted)
                    .text_size(px(11.0))
                    .child(SharedString::from(description.to_string())),
            )
    }

    fn toggle_pill(enabled: bool, colors: &rover_theme::ThemeColors) -> Div {
        div()
            .w(px(32.0))
            .h(px(18.0))
            .rounded(px(9.0))
            .bg(if enabled {
                colors.text_accent
            } else {
                colors.border_variant
            })
            .flex()
            .items_center()
            .child(
                div()
                    .w(px(14.0))
                    .h(px(14.0))
                    .rounded(px(7.0))
                    .bg(gpui::white())
                    .ml(if enabled { px(16.0) } else { px(2.0) }),
            )
    }

    fn kbd_badge(text: &str, colors: &rover_theme::ThemeColors) -> Div {
        div()
            .px(px(6.0))
            .py(px(3.0))
            .rounded(px(4.0))
            .bg(colors.element_hover)
            .border_1()
            .border_color(colors.border_variant)
            .text_color(colors.text_muted)
            .text_size(px(11.0))
            .child(SharedString::from(text.to_string()))
    }

    fn render_general(&self, colors: &rover_theme::ThemeColors, cx: &mut Context<Self>) -> Div {
        let picker_position = self.picker_position.clone();
        div()
            .flex()
            .flex_col()
            .child(Self::section_header("General", colors))
            .child(
                Self::setting_row(colors)
                    .id("launch-login")
                    .cursor_pointer()
                    .child(Self::setting_label(
                        "Launch at Login",
                        "Automatically start Rover when you log in",
                        colors,
                    ))
                    .child(Self::toggle_pill(self.launch_at_login, colors))
                    .on_click(cx.listener(|this, _e, _w, cx| {
                        this.launch_at_login = !this.launch_at_login;
                        let _ = this
                            .db
                            .set_setting("launch_at_login", &this.launch_at_login.to_string());
                        cx.notify();
                    })),
            )
            .child(
                Self::setting_row(colors)
                    .id("show-recent")
                    .cursor_pointer()
                    .child(Self::setting_label(
                        "Show Recent Apps",
                        "Display recently used applications at the top of results",
                        colors,
                    ))
                    .child(Self::toggle_pill(self.show_recent_apps, colors))
                    .on_click(cx.listener(|this, _e, _w, cx| {
                        this.show_recent_apps = !this.show_recent_apps;
                        let _ = this
                            .db
                            .set_setting("show_recent_apps", &this.show_recent_apps.to_string());
                        cx.notify();
                    })),
            )
            .child(
                Self::setting_row(colors)
                    .child(Self::setting_label(
                        "Picker Position",
                        "Where the picker appears on screen",
                        colors,
                    ))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .children(["Top", "Center", "Bottom"].iter().map(|&pos| {
                                let selected = picker_position == pos.to_lowercase();
                                let pos_lower = pos.to_lowercase();
                                div()
                                    .id(SharedString::from(format!("picker-pos-{pos_lower}")))
                                    .px(px(10.0))
                                    .py(px(4.0))
                                    .rounded(px(5.0))
                                    .cursor_pointer()
                                    .border_1()
                                    .border_color(if selected {
                                        colors.text_accent
                                    } else {
                                        colors.border_variant
                                    })
                                    .bg(if selected {
                                        colors.element_selected
                                    } else {
                                        colors.surface_background
                                    })
                                    .text_color(if selected {
                                        colors.text_accent
                                    } else {
                                        colors.text_muted
                                    })
                                    .text_size(px(12.0))
                                    .child(SharedString::from(pos.to_string()))
                                    .on_click(cx.listener(move |this, _e, _w, cx| {
                                        this.picker_position = pos_lower.clone();
                                        let _ = this.db.set_setting("picker_position", &pos_lower);
                                        cx.notify();
                                    }))
                            })),
                    ),
            )
    }

    fn render_appearance(&self, colors: &rover_theme::ThemeColors, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .flex_col()
            .child(Self::section_header("Appearance", colors))
            .child(
                div()
                    .text_color(colors.text_muted)
                    .text_size(px(12.0))
                    .mb(px(8.0))
                    .child("Select a color theme for Rover."),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(1.0))
                    .rounded_lg()
                    .overflow_hidden()
                    .border_1()
                    .border_color(colors.border_variant)
                    .children(self.theme_names.iter().enumerate().map(|(ix, name)| {
                        let selected = ix == self.selected_theme_ix;
                        let name_clone = name.clone();
                        div()
                            .id(SharedString::from(format!("theme-{ix}")))
                            .px_3()
                            .py(px(8.0))
                            .cursor_pointer()
                            .bg(if selected {
                                colors.element_selected
                            } else {
                                colors.surface_background
                            })
                            .hover(|s| {
                                s.bg(if selected {
                                    colors.element_selected
                                } else {
                                    colors.element_hover
                                })
                            })
                            .when(ix > 0, |el: Stateful<Div>| {
                                el.border_t_1()
                                    .border_color(colors.border_variant.opacity(0.5))
                            })
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .text_color(if selected {
                                                colors.text_accent
                                            } else {
                                                colors.text
                                            })
                                            .text_size(px(13.0))
                                            .child(SharedString::from(name.clone())),
                                    )
                                    .when(selected, |el: Div| {
                                        el.child(
                                            div()
                                                .text_color(colors.text_accent)
                                                .text_size(px(13.0))
                                                .child(SharedString::from("\u{2713}")),
                                        )
                                    }),
                            )
                            .on_click(cx.listener(move |this, _e, _w, cx| {
                                this.selected_theme_ix = ix;
                                let registry = cx.global::<ThemeRegistry>();
                                if let Some(t) = registry.list().get(ix) {
                                    rover_theme::set_active_theme(t.clone(), cx);
                                }
                                let _ = this.db.set_setting("theme", &name_clone);
                                cx.notify();
                            }))
                    })),
            )
    }

    fn render_hotkeys(&self, colors: &rover_theme::ThemeColors, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .flex_col()
            .child(Self::section_header("Hotkeys", colors))
            .child(
                Self::setting_row(colors)
                    .child(Self::setting_label(
                        "Toggle Rover",
                        "Show or hide the main window",
                        colors,
                    ))
                    .child(Self::kbd_badge("\u{2318} Space", colors)),
            )
            .child(
                Self::setting_row(colors)
                    .id("hyperkey-toggle")
                    .cursor_pointer()
                    .child(Self::setting_label(
                        "Hyperkey",
                        "Remap Caps Lock to Ctrl+Shift+Opt+Cmd",
                        colors,
                    ))
                    .child(Self::toggle_pill(self.hyperkey_enabled, colors))
                    .on_click(cx.listener(|this, _e, _w, cx| {
                        this.hyperkey_enabled = !this.hyperkey_enabled;
                        let _ = this
                            .db
                            .set_setting("hyperkey_enabled", &this.hyperkey_enabled.to_string());
                        if this.hyperkey_enabled {
                            let _ = hotkey::enable_hyperkey_remap();
                        } else {
                            let _ = hotkey::disable_hyperkey_remap();
                        }
                        cx.notify();
                    })),
            )
            .child(
                Self::setting_row(colors)
                    .child(Self::setting_label(
                        "Hyperkey + Space",
                        "Alternative trigger when Hyperkey is enabled",
                        colors,
                    ))
                    .child(Self::kbd_badge("\u{21EA} Space", colors)),
            )
            .child(
                Self::setting_row(colors)
                    .child(Self::setting_label(
                        "Item Actions",
                        "Show contextual actions for the selected item",
                        colors,
                    ))
                    .child(Self::kbd_badge("\u{2318}K", colors)),
            )
    }

    fn render_ai(&self, colors: &rover_theme::ThemeColors, cx: &mut Context<Self>) -> Div {
        let ai_available = rusty_foundationmodels::is_available();
        div()
            .flex()
            .flex_col()
            .child(Self::section_header("AI", colors))
            .child(
                Self::setting_row(colors)
                    .id("ai-enabled")
                    .cursor_pointer()
                    .child(Self::setting_label(
                        "Apple Intelligence",
                        "Use on-device AI to interpret queries (requires macOS 26+)",
                        colors,
                    ))
                    .child(Self::toggle_pill(self.ai_enabled, colors))
                    .on_click(cx.listener(|this, _e, _w, cx| {
                        this.ai_enabled = !this.ai_enabled;
                        let _ = this.db.set_setting("ai_enabled", &this.ai_enabled.to_string());
                        cx.notify();
                    })),
            )
            .child(
                Self::setting_row(colors)
                    .child(Self::setting_label(
                        "Availability",
                        "",
                        colors,
                    ))
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(if ai_available {
                                colors.text_accent
                            } else {
                                colors.text_muted
                            })
                            .child(SharedString::from(if ai_available {
                                "Available"
                            } else {
                                "Not available on this device"
                            })),
                    ),
            )
            .child(
                Self::setting_row(colors)
                    .id("ai-terminal-enabled")
                    .cursor_pointer()
                    .child(Self::setting_label(
                        "AI Terminal Commands",
                        "Interpret free-text queries as terminal commands",
                        colors,
                    ))
                    .child(Self::toggle_pill(self.ai_terminal_enabled, colors))
                    .on_click(cx.listener(|this, _e, _w, cx| {
                        this.ai_terminal_enabled = !this.ai_terminal_enabled;
                        let _ = this.db.set_setting(
                            "ai_terminal_enabled",
                            &this.ai_terminal_enabled.to_string(),
                        );
                        cx.notify();
                    })),
            )
    }

    fn render_data(&self, colors: &rover_theme::ThemeColors, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .flex_col()
            .child(Self::section_header("Data", colors))
            .child(
                Self::setting_row(colors)
                    .child(Self::setting_label(
                        "Clipboard History Limit",
                        "Maximum number of clipboard entries to store",
                        colors,
                    ))
                    .child(
                        div()
                            .text_color(colors.text_muted)
                            .text_size(px(13.0))
                            .child("500"),
                    ),
            )
            .child(
                div()
                    .id("clear-clipboard")
                    .flex()
                    .items_center()
                    .py(px(10.0))
                    .cursor_pointer()
                    .hover(|s| s.bg(hsla(0., 0.6, 0.5, 0.1)))
                    .rounded_md()
                    .px(px(2.0))
                    .child(
                        div()
                            .text_color(hsla(0., 0.7, 0.6, 1.0))
                            .text_size(px(13.0))
                            .child("Clear Clipboard History"),
                    )
                    .on_click(cx.listener(|this, _e, _w, _cx| {
                        let _ = this.db.clear_clipboard_history();
                    })),
            )
            .child(
                div()
                    .id("clear-query-history")
                    .flex()
                    .items_center()
                    .py(px(10.0))
                    .cursor_pointer()
                    .hover(|s| s.bg(hsla(0., 0.6, 0.5, 0.1)))
                    .rounded_md()
                    .px(px(2.0))
                    .child(
                        div()
                            .text_color(hsla(0., 0.7, 0.6, 1.0))
                            .text_size(px(13.0))
                            .child("Clear Search History"),
                    )
                    .on_click(cx.listener(|this, _e, _w, _cx| {
                        let conn = this.db.conn();
                        if let Ok(c) = conn.lock() {
                            let _ = c.execute("DELETE FROM query_history", []);
                        }
                    })),
            )
    }

    fn render_about(&self, colors: &rover_theme::ThemeColors) -> Div {
        div()
            .flex()
            .flex_col()
            .child(Self::section_header("About", colors))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.0))
                    .py(px(8.0))
                    .child(
                        div()
                            .text_color(colors.text)
                            .text_size(px(14.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Rover"),
                    )
                    .child(
                        div()
                            .text_color(colors.text_muted)
                            .text_size(px(13.0))
                            .child("Version 0.1.0"),
                    )
                    .child(
                        div()
                            .text_color(colors.text_muted)
                            .text_size(px(12.0))
                            .child("A fast, extensible launcher for macOS."),
                    )
                    .child(
                        div()
                            .text_color(colors.text_muted)
                            .text_size(px(12.0))
                            .child("Built with GPUI."),
                    ),
            )
    }
}

impl Render for SettingsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = theme(cx);
        let colors = t.colors.clone();

        let content = match self.active_section {
            SettingsSection::General => self.render_general(&colors, cx),
            SettingsSection::Appearance => self.render_appearance(&colors, cx),
            SettingsSection::Hotkeys => self.render_hotkeys(&colors, cx),
            SettingsSection::AI => self.render_ai(&colors, cx),
            SettingsSection::Data => self.render_data(&colors, cx),
            SettingsSection::About => self.render_about(&colors),
        };

        let active = self.active_section;

        div()
            .size_full()
            .flex()
            .flex_col()
            // Custom titlebar area (transparent native titlebar, we draw our own)
            .child(
                div()
                    .h(px(38.0))
                    .w_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(colors.surface_background)
                    .border_b_1()
                    .border_color(colors.border_variant)
                    .child(
                        div()
                            .text_color(colors.text_muted)
                            .text_size(px(13.0))
                            .font_weight(FontWeight::MEDIUM)
                            .child("Settings"),
                    ),
            )
            // Main content
            .child(
                div()
                    .flex_grow()
                    .flex()
                    .overflow_hidden()
                    // Sidebar
                    .child(
                        div()
                            .w(px(180.0))
                            .flex()
                            .flex_col()
                            .py(px(12.0))
                            .px(px(8.0))
                            .gap(px(2.0))
                            .bg(colors.surface_background)
                            .border_r_1()
                            .border_color(colors.border_variant)
                            .children(SettingsSection::all().iter().map(|section| {
                                let selected = *section == active;
                                let label = section.label();
                                let section = *section;
                                div()
                                    .id(SharedString::from(format!("nav-{label}")))
                                    .px(px(8.0))
                                    .py(px(6.0))
                                    .rounded(px(6.0))
                                    .cursor_pointer()
                                    .when(selected, |el: Stateful<Div>| {
                                        el.bg(colors.element_selected)
                                    })
                                    .when(!selected, |el: Stateful<Div>| {
                                        el.hover(|s| s.bg(colors.element_hover))
                                    })
                                    .child(
                                        div()
                                            .text_color(if selected {
                                                colors.text
                                            } else {
                                                colors.text_muted
                                            })
                                            .text_size(px(13.0))
                                            .font_weight(if selected {
                                                FontWeight::MEDIUM
                                            } else {
                                                FontWeight::NORMAL
                                            })
                                            .child(SharedString::from(label)),
                                    )
                                    .on_click(cx.listener(move |this, _e, _w, cx| {
                                        this.active_section = section;
                                        cx.notify();
                                    }))
                            })),
                    )
                    // Content pane
                    .child(
                        div()
                            .flex_grow()
                            .bg(colors.background)
                            .p(px(24.0))
                            .overflow_hidden()
                            .child(content),
                    ),
            )
    }
}

fn open_settings_window_with_section(db: Arc<RoverDb>, section: SettingsSection, cx: &mut App) {
    let bounds = Bounds::centered(None, size(px(640.0), px(480.0)), cx);

    let _window = cx
        .open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Settings".into()),
                    appears_transparent: true,
                    traffic_light_position: Some(point(px(12.0), px(12.0))),
                    ..Default::default()
                }),
                kind: WindowKind::Normal,
                focus: true,
                is_movable: true,
                is_resizable: true,
                ..Default::default()
            },
            |_window, cx| cx.new(|cx| SettingsView::new(db, section, cx)),
        )
        .ok();
}

pub fn open_settings_window(db: Arc<RoverDb>, cx: &mut App) {
    open_settings_window_with_section(db, SettingsSection::General, cx);
}

pub fn open_settings_window_on_about(db: Arc<RoverDb>, cx: &mut App) {
    open_settings_window_with_section(db, SettingsSection::About, cx);
}
