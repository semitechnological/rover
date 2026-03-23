//! Floating transport bar — shows tab pills + URL/command input at the top
//! center of the screen, like a browser address bar.
//!
//! In browser context: shows tab pills + URL/command input.
//! Triggered by Cmd+\ from the browser window.
//! Closes automatically when focus leaves (click outside or switch apps).

use crate::{BrowserAction, BrowserWindow, TabState, domain_from_url, parse_command, looks_like_url};
use gpui::prelude::*;
use gpui::*;
use std::time::Duration;

// ─── Layout constants ─────────────────────────────────────────────────────────

const TRANSPORT_W: f32 = 640.0;
const PILL_ROW_H: f32 = 30.0;
const INPUT_ROW_H: f32 = 32.0;
const HINT_ROW_H: f32 = 20.0;
/// Distance from the top of the screen.
const TOP_OFFSET: f32 = 8.0;

// ─── Platform: position top-center of screen ─────────────────────────────────

#[cfg(target_os = "macos")]
fn transport_bounds(transport_h: f32) -> Bounds<Pixels> {
    use objc::{class, msg_send, sel, sel_impl};
    use cocoa::foundation::NSRect;

    let (sw, _sh): (f32, f32) = unsafe {
        let main: *mut objc::runtime::Object = msg_send![class!(NSScreen), mainScreen];
        let frame: NSRect = msg_send![main, frame];
        (frame.size.width as f32, frame.size.height as f32)
    };

    let w = TRANSPORT_W;
    let h = transport_h;
    let x = (sw - w) / 2.0;

    Bounds {
        origin: point(px(x), px(TOP_OFFSET)),
        size: size(px(w), px(h)),
    }
}

#[cfg(not(target_os = "macos"))]
fn transport_bounds(transport_h: f32) -> Bounds<Pixels> {
    Bounds {
        origin: point(px(200.0), px(TOP_OFFSET)),
        size: size(px(TRANSPORT_W), px(transport_h)),
    }
}

// ─── BrowserTransport ─────────────────────────────────────────────────────────

pub struct BrowserTransport {
    browser: WeakEntity<BrowserWindow>,
    text: String,
    focus: FocusHandle,
    _dismiss_task: Task<()>,
}

impl BrowserTransport {
    fn new(browser: WeakEntity<BrowserWindow>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus);

        // Capture window handle so the dismiss task can close just this popup.
        let win_handle = window.window_handle();

        // Poll every 100ms; close when the transport loses focus
        // (user clicked outside or switched apps).
        let dismiss_task = cx.spawn(async move |this, cx: &mut AsyncApp| {
            // Give the window a moment to fully initialize focus.
            cx.background_executor()
                .timer(Duration::from_millis(200))
                .await;
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                let focused = win_handle
                    .update(cx, |_, window, _cx| window.is_window_active())
                    .unwrap_or(false);
                if !focused {
                    let _ = win_handle.update(cx, |_, window, _cx| {
                        window.remove_window();
                    });
                    break;
                }
            }
        });

        Self {
            browser,
            text: String::new(),
            focus,
            _dismiss_task: dismiss_task,
        }
    }

    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = std::mem::take(&mut self.text);
        if let Some(browser) = self.browser.upgrade() {
            let action = parse_command(&text);
            browser.update(cx, |bw, cx| {
                bw.execute_action_external(action, cx);
            });
        }
        window.remove_window();
    }

    fn hint_for(text: &str) -> Option<String> {
        if text.is_empty() {
            return None;
        }
        match parse_command(text) {
            BrowserAction::Navigate(url) => Some(format!("↵  open {url}")),
            BrowserAction::NewSearch { engine_name, query } => {
                Some(format!("↵  {engine_name}: {query}"))
            }
            BrowserAction::Back => Some("↵  go back".to_string()),
            BrowserAction::Forward => Some("↵  go forward".to_string()),
            BrowserAction::Reload => Some("↵  reload".to_string()),
            BrowserAction::CloseTab => Some("↵  close tab".to_string()),
            BrowserAction::NewTab => Some("↵  new tab".to_string()),
            BrowserAction::Find(q) => Some(format!("↵  find: {q}")),
            BrowserAction::AddBookmark => Some("↵  bookmark this page".to_string()),
            BrowserAction::ShowHistory => Some("↵  show history".to_string()),
            BrowserAction::SearchHistory(q) => Some(format!("↵  search history: {q}")),
            BrowserAction::SearchBookmarks(q) => Some(format!("↵  search bookmarks: {q}")),
            BrowserAction::Share => Some("↵  copy URL to clipboard".to_string()),
            _ => None,
        }
    }
}

impl Focusable for BrowserTransport {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for BrowserTransport {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Read browser state
        let (tab_infos, active_tab, active_url, overlay_hint) =
            if let Some(browser) = self.browser.upgrade() {
                let bw = browser.read(cx);
                let (infos, idx) = bw.tab_infos();
                let url = bw.active_url();
                let hint = bw.overlay_hint.clone();
                (infos, idx, url, hint)
            } else {
                (vec![], 0, String::new(), None)
            };

        let text = self.text.clone();
        let hint = Self::hint_for(&text).or(overlay_hint);

        // ── Tab pills row (floating above the input bar) ───────────────────────
        let mut pills_row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.0))
            .px(px(12.0))
            .h(px(PILL_ROW_H))
            .overflow_x_hidden();

        for (i, (title, state)) in tab_infos.iter().enumerate() {
            let is_active = i == active_tab;
            let short = if title.chars().count() > 24 {
                let end = title
                    .char_indices()
                    .nth(22)
                    .map(|(b, _)| b)
                    .unwrap_or(title.len());
                format!("{}…", &title[..end])
            } else {
                title.clone()
            };
            let dot_color = match state {
                TabState::Active => rgb(0x34c759u32),
                TabState::Cached => rgb(0xffbd2eu32),
                TabState::Frozen => rgb(0x555555u32),
            };
            let pill_bg = if is_active { rgb(0x2a2a2cu32) } else { rgb(0x1c1c1eu32) };
            let pill_border = if is_active { rgb(0x5588ccu32) } else { rgb(0x333335u32) };
            let text_color = if is_active { rgb(0xffffffu32) } else { rgb(0x8e8e93u32) };

            pills_row = pills_row.child(
                div()
                    .id(ElementId::NamedInteger("tp".into(), i as u64))
                    .flex()
                    .items_center()
                    .gap(px(5.0))
                    .px(px(8.0))
                    .py(px(4.0))
                    .rounded_lg()
                    .bg(pill_bg)
                    .border_1()
                    .border_color(pill_border)
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if is_active {
                            // Re-focus the input so the user can start typing immediately.
                            window.focus(&this.focus);
                        } else if let Some(browser) = this.browser.upgrade() {
                            browser.update(cx, |bw, cx| {
                                bw.switch_tab_external(i, cx);
                            });
                        }
                        cx.notify();
                    }))
                    .child(
                        div()
                            .w(px(6.0))
                            .h(px(6.0))
                            .rounded_full()
                            .bg(dot_color),
                    )
                    .child(
                        div()
                            .text_color(text_color)
                            .text_size(px(11.0))
                            .child(SharedString::from(short)),
                    ),
            );
        }

        // + New tab button
        pills_row = pills_row.child(
            div()
                .id("new-tab-btn")
                .px(px(8.0))
                .py(px(4.0))
                .rounded_lg()
                .bg(rgb(0x1c1c1eu32))
                .border_1()
                .border_color(rgb(0x333335u32))
                .cursor_pointer()
                .on_click(cx.listener(|this, _, _window, cx| {
                    if let Some(browser) = this.browser.upgrade() {
                        browser.update(cx, |bw, cx| {
                            bw.execute_action_external(BrowserAction::NewTab, cx);
                        });
                    }
                    cx.notify();
                }))
                .child(
                    div()
                        .text_color(rgb(0x8e8e93u32))
                        .text_size(px(12.0))
                        .child(SharedString::from("+")),
                ),
        );

        // ── Address / input row ────────────────────────────────────────────────
        let full_url = active_url.clone();
        let display_url = domain_from_url(&active_url).to_string();
        let input_content = if text.is_empty() {
            format!("{display_url}▌")
        } else {
            format!("{text}▌")
        };
        let input_color = if text.is_empty() {
            rgb(0x636366u32)
        } else {
            rgb(0xffffffu32)
        };

        let input_row = div()
            .flex()
            .flex_row()
            .items_center()
            .px(px(12.0))
            .h(px(INPUT_ROW_H))
            .gap_2()
            .child(
                div()
                    .text_color(rgb(0x636366u32))
                    .text_size(px(12.0))
                    .child(SharedString::from("→")),
            )
            .child(
                div()
                    .flex_1()
                    .px(px(8.0))
                    .py(px(4.0))
                    .rounded_md()
                    .bg(rgb(0x111111u32))
                    .border_1()
                    .border_color(rgb(0x333335u32))
                    .child(
                        div()
                            .text_color(input_color)
                            .text_size(px(12.0))
                            .child(SharedString::from(input_content)),
                    ),
            )
            // Show full URL on the right when not typing
            .when(text.is_empty() && !full_url.is_empty(), |el| {
                el.child(
                    div()
                        .text_color(rgb(0x48484au32))
                        .text_size(px(10.0))
                        .max_w(px(200.0))
                        .overflow_hidden()
                        .child(SharedString::from(full_url)),
                )
            });

        // ── Root container — horizontal bar ─────────────────────────────────────
        let mut root = div()
            .key_context("BrowserTransport")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                let key = event.keystroke.key.as_str();
                let mods = &event.keystroke.modifiers;
                if mods.platform || mods.control {
                    return;
                }
                match key {
                    "escape" => window.remove_window(),
                    "return" | "enter" => this.submit(window, cx),
                    "backspace" => {
                        this.text.pop();
                        cx.notify();
                    }
                    "space" => {
                        this.text.push(' ');
                        cx.notify();
                    }
                    k if k.chars().count() == 1
                        && k.chars().next().map_or(false, |c| !c.is_control()) =>
                    {
                        this.text.push_str(k);
                        cx.notify();
                    }
                    _ => {}
                }
            }))
            .w(px(TRANSPORT_W))
            .flex()
            .flex_col()
            .rounded_xl()
            .bg(rgb(0x19191bu32))
            .border_1()
            .border_color(rgb(0x2c2c2eu32))
            .shadow_lg()
            .child(pills_row)
            .child(input_row);

        // Hint row (optional)
        if let Some(hint_text) = hint {
            root = root.child(
                div()
                    .px(px(14.0))
                    .pb(px(6.0))
                    .text_color(rgb(0x5588ccu32))
                    .text_size(px(10.0))
                    .child(SharedString::from(hint_text)),
            );
        }

        root
    }
}

// ─── Entry point ──────────────────────────────────────────────────────────────

/// Open the floating transport popup near the cursor's nearest screen corner.
/// Passing the `BrowserWindow` entity lets the transport read/control tabs.
pub fn open_browser_transport(browser: WeakEntity<BrowserWindow>, cx: &mut App) {
    let has_hint = browser
        .upgrade()
        .map(|b| b.read(cx).overlay_hint.is_some())
        .unwrap_or(false);

    let transport_h = PILL_ROW_H
        + INPUT_ROW_H
        + if has_hint { HINT_ROW_H } else { 0.0 }
        + 2.0; // border

    let bounds = transport_bounds(transport_h);

    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: None,
            kind: WindowKind::PopUp,
            focus: true,
            is_movable: false,
            is_resizable: false,
            window_background: WindowBackgroundAppearance::Opaque,
            ..Default::default()
        },
        move |window, cx| {
            cx.new(|cx| {
                let transport = BrowserTransport::new(browser, window, cx);
                transport
            })
        },
    )
    .ok();
}
