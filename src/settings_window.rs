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
    compact_mode: bool,
    hotkey_key_name: String,
    hotkey_modifiers: u32,
    hotkey_display: String,
    // Focus handle so the Hotkeys section can receive raw key events
    focus_handle: FocusHandle,
    // Pulse states — true while the corresponding key is being pressed
    pulse_key: bool,
    pulse_cmd: bool,
    pulse_opt: bool,
    pulse_ctrl: bool,
    // Recording state — Raycast-style click-to-record
    hotkey_recording: bool,
    recording_mods: u32,
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
        let compact_mode = db
            .get_setting("compact_mode")
            .ok()
            .flatten()
            .map(|v| v == "true")
            .unwrap_or(false);
        // Try to load a custom recorded hotkey; fall back to preset
        let (hotkey_key_name, hotkey_modifiers, hotkey_display) = {
            let stored_key = db.get_setting("hotkey_key_name").ok().flatten();
            let stored_mods = db
                .get_setting("hotkey_modifiers")
                .ok()
                .flatten()
                .and_then(|v| v.parse::<u32>().ok());
            match (stored_key, stored_mods) {
                (Some(k), Some(m)) => {
                    let display = hotkey::display_for_combo(m, &k);
                    (k, m, display)
                }
                _ => {
                    let preset = db
                        .get_setting("hotkey_preset")
                        .ok()
                        .flatten()
                        .unwrap_or_else(|| "cmd-space".to_string());
                    let (_, _, _, m) = hotkey::PRESETS
                        .iter()
                        .find(|(id, _, _, _)| *id == preset.as_str())
                        .copied()
                        .unwrap_or(("cmd-space", "⌘ Space", hotkey::K_VK_SPACE, hotkey::CMD_KEY));
                    let display = hotkey::display_for_combo(m, "space");
                    ("space".to_string(), m, display)
                }
            }
        };

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
            compact_mode,
            hotkey_key_name,
            hotkey_modifiers,
            hotkey_display,
            focus_handle: cx.focus_handle(),
            pulse_key: false,
            pulse_cmd: false,
            pulse_opt: false,
            pulse_ctrl: false,
            hotkey_recording: false,
            recording_mods: 0,
        }
    }

    // ── Hotkey pulse helpers ──────────────────────────────────────────────────

    /// Returns (modifier_parts, key_part) where each is (display_text, is_pulsing).
    fn hotkey_badge_parts(&self) -> (Vec<(&'static str, bool)>, (String, bool)) {
        let mods = self.hotkey_modifiers;
        let mut modifier_parts = Vec::new();
        if mods & hotkey::CTRL_KEY  != 0 { modifier_parts.push(("⌃", self.pulse_ctrl)); }
        if mods & hotkey::OPT_KEY   != 0 { modifier_parts.push(("⌥", self.pulse_opt)); }
        if mods & hotkey::CMD_KEY   != 0 { modifier_parts.push(("⌘", self.pulse_cmd)); }
        let key_display = match self.hotkey_key_name.as_str() {
            "space" => "Space".to_string(),
            "return" => "↩".to_string(),
            "tab" => "⇥".to_string(),
            "backspace" | "delete" => "⌫".to_string(),
            "escape" => "⎋".to_string(),
            other => other.to_uppercase(),
        };
        (modifier_parts, (key_display, self.pulse_key))
    }

    fn on_hotkey_key_down(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        if self.active_section != SettingsSection::Hotkeys {
            return;
        }

        let key = event.keystroke.key.as_str();

        // Ignore bare modifier keys — they don't count as a trigger key
        if matches!(key, "control" | "alt" | "shift" | "cmd" | "super" | "meta" | "capslock") {
            return;
        }

        if self.hotkey_recording {
            // Escape cancels recording
            if key == "escape" {
                self.hotkey_recording = false;
                self.recording_mods = 0;
                cx.notify();
                return;
            }

            let mods = self.recording_mods;
            let key_name = key.to_string();

            // Need at least one modifier for a global hotkey
            if mods == 0 {
                return;
            }

            if let Some(vk) = hotkey::key_name_to_vk(&key_name) {
                let display = hotkey::display_for_combo(mods, &key_name);
                self.hotkey_key_name = key_name.clone();
                self.hotkey_modifiers = mods;
                self.hotkey_display = display.clone();
                self.hotkey_recording = false;
                self.recording_mods = 0;

                // Persist the custom hotkey to DB
                let _ = self.db.set_setting("hotkey_key_name", &key_name);
                let _ = self.db.set_setting("hotkey_modifiers", &mods.to_string());

                // Re-register the Carbon hotkey
                hotkey::unregister_main_hotkey();
                hotkey::register_main_hotkey(vk, mods);

                cx.notify();
            }
            return;
        }

        // Not recording — pulse preview mode (Space key only)
        if event.keystroke.key != "space" {
            return;
        }
        self.pulse_key = true;
        cx.notify();
        // Reset after 300 ms
        cx.spawn(async move |entity, cx: &mut AsyncApp| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(300))
                .await;
            let _ = entity.update(cx, |this: &mut SettingsView, cx| {
                this.pulse_key = false;
                cx.notify();
            });
        })
        .detach();
    }

    fn on_hotkey_mods_changed(&mut self, event: &ModifiersChangedEvent, cx: &mut Context<Self>) {
        if self.active_section != SettingsSection::Hotkeys {
            return;
        }

        if self.hotkey_recording {
            // Track which modifiers are held during recording
            self.recording_mods = hotkey::mods_from_gpui(
                event.modifiers.platform,
                event.modifiers.alt,
                event.modifiers.control,
                event.modifiers.shift,
            );
            cx.notify();
            return;
        }

        // Pulse preview: light up modifier badges matching the current hotkey
        let mods = self.hotkey_modifiers;
        let mut changed = false;
        if mods & hotkey::CMD_KEY != 0 {
            let v = event.modifiers.platform;
            if self.pulse_cmd != v { self.pulse_cmd = v; changed = true; }
        }
        if mods & hotkey::OPT_KEY != 0 {
            let v = event.modifiers.alt;
            if self.pulse_opt != v { self.pulse_opt = v; changed = true; }
        }
        if mods & hotkey::CTRL_KEY != 0 {
            let v = event.modifiers.control;
            if self.pulse_ctrl != v { self.pulse_ctrl = v; changed = true; }
        }
        if changed {
            cx.notify();
        }
    }

    // ── UI helpers ────────────────────────────────────────────────────────────

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

    fn kbd_badge_pulsed(text: &str, pulsing: bool, colors: &rover_theme::ThemeColors) -> Div {
        div()
            .px(px(6.0))
            .py(px(3.0))
            .rounded(px(4.0))
            .bg(if pulsing {
                colors.text_accent.opacity(0.15)
            } else {
                colors.element_hover
            })
            .border_1()
            .border_color(if pulsing {
                colors.text_accent
            } else {
                colors.border_variant
            })
            .text_color(if pulsing {
                colors.text_accent
            } else {
                colors.text_muted
            })
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
                            .children(["Top", "Center", "Bottom", "Cursor"].iter().map(|&pos| {
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
            .child(
                Self::setting_row(colors)
                    .id("compact-mode")
                    .cursor_pointer()
                    .child(Self::setting_label(
                        "Compact Mode",
                        "Hide suggestions until you start typing, show as pills",
                        colors,
                    ))
                    .child(Self::toggle_pill(self.compact_mode, colors))
                    .on_click(cx.listener(|this, _e, _w, cx| {
                        this.compact_mode = !this.compact_mode;
                        let _ = this
                            .db
                            .set_setting("compact_mode", &this.compact_mode.to_string());
                        cx.notify();
                    })),
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
        let (mod_parts, (key_text, key_pulsing)) = self.hotkey_badge_parts();
        let is_recording = self.hotkey_recording;
        let recording_mods = self.recording_mods;

        div()
            .flex()
            .flex_col()
            .child(Self::section_header("Hotkeys", colors))
            // Row: current hotkey display with pulsable badges + click-to-record
            .child(
                Self::setting_row(colors)
                    .child(Self::setting_label(
                        "Toggle Rover",
                        if is_recording {
                            "Press a key combination (Esc to cancel)"
                        } else {
                            "Click the badge to change • press keys to preview"
                        },
                        colors,
                    ))
                    .child(
                        div()
                            .id("hotkey-record-area")
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _e, w, cx| {
                                this.hotkey_recording = !this.hotkey_recording;
                                this.recording_mods = 0;
                                if this.hotkey_recording {
                                    w.focus(&this.focus_handle);
                                }
                                cx.notify();
                            }))
                            .map(|el| {
                                if is_recording {
                                    // Show "recording" state: highlight held mods
                                    let display = if recording_mods == 0 {
                                        "Hold modifiers...".to_string()
                                    } else {
                                        hotkey::display_for_combo(recording_mods, "?")
                                    };
                                    el.child(
                                        div()
                                            .px(px(10.0))
                                            .py(px(4.0))
                                            .rounded(px(5.0))
                                            .border_1()
                                            .border_color(colors.text_accent)
                                            .bg(colors.text_accent.opacity(0.12))
                                            .text_color(colors.text_accent)
                                            .text_size(px(12.0))
                                            .child(SharedString::from(display)),
                                    )
                                } else {
                                    // Show current hotkey with pulsable badges
                                    el.children(
                                        mod_parts
                                            .into_iter()
                                            .map(|(text, pulsing)| {
                                                Self::kbd_badge_pulsed(text, pulsing, colors)
                                            })
                                            .collect::<Vec<_>>(),
                                    )
                                    .child(Self::kbd_badge_pulsed(&key_text, key_pulsing, colors))
                                }
                            }),
                    ),
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
                        let _ = this.db.clear_query_history();
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

impl Focusable for SettingsView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
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

        // Capture the entity handle so the key/modifier closures can update our state.
        // These closures are `Fn + 'static` — Entity<T> is Clone + 'static.
        let entity_kd = cx.entity();
        let entity_mc = entity_kd.clone();

        div()
            .size_full()
            .flex()
            .flex_col()
            // Track focus so this root div can receive key events (used for hotkey preview)
            .track_focus(&self.focus_handle)
            .on_key_down(move |event: &KeyDownEvent, _: &mut Window, cx: &mut App| {
                entity_kd.update(cx, |this, cx| this.on_hotkey_key_down(event, cx));
            })
            .on_modifiers_changed(
                move |event: &ModifiersChangedEvent, _: &mut Window, cx: &mut App| {
                    entity_mc.update(cx, |this, cx| this.on_hotkey_mods_changed(event, cx));
                },
            )
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
                                    .on_click(cx.listener(move |this, _e, w, cx| {
                                        this.active_section = section;
                                        // Give focus to the settings view when switching to
                                        // Hotkeys so key events reach the pulse preview
                                        if section == SettingsSection::Hotkeys {
                                            w.focus(&this.focus_handle);
                                        }
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
