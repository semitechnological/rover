use gpui::prelude::*;
use gpui::*;
use rover_theme::theme;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

// ── Ghostty launcher ──

fn config_path() -> PathBuf {
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("~/.config"))
        .join("rover");
    config_dir.join("ghostty-rover.conf")
}

fn ensure_config(path: &PathBuf) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let config = include_str!("../assets/ghostty-rover.conf");
    let _ = std::fs::write(path, config);
}

fn spawn_ghostty() {
    let path = config_path();
    ensure_config(&path);
    let _ = Command::new("open")
        .arg("-na")
        .arg("Ghostty.app")
        .arg("--args")
        .arg(format!("--config-file={}", path.display()))
        .spawn();
}

fn send_text_to_ghostty(text: &str) {
    let escaped = text.replace('\\', "\\\\").replace('"', "\\\"");
    let script = format!(
        r#"tell application "Ghostty" to activate
delay 0.05
tell application "System Events"
    tell process "Ghostty"
        keystroke "{escaped}"
        keystroke return
    end tell
end tell"#
    );
    std::thread::spawn(move || {
        let _ = Command::new("osascript").arg("-e").arg(&script).output();
    });
}

/// Check if Ghostty is currently the frontmost application
fn is_ghostty_focused() -> bool {
    let output = Command::new("osascript")
        .arg("-e")
        .arg(r#"tell application "System Events" to get name of first application process whose frontmost is true"#)
        .output()
        .ok();
    output
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "Ghostty")
        .unwrap_or(false)
}

// ── Shell context (polled from the environment) ──

#[derive(Clone, Default)]
struct ShellContext {
    cwd: String,
    git_branch: String,
    git_dirty: bool,
    hostname: String,
}

fn poll_shell_context() -> ShellContext {
    let cwd = std::env::current_dir()
        .map(|p| {
            let home = dirs::home_dir().unwrap_or_default();
            if let Ok(rel) = p.strip_prefix(&home) {
                format!("~/{}", rel.display())
            } else {
                p.display().to_string()
            }
        })
        .unwrap_or_default();

    let git_branch = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_default();

    let git_dirty = Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .ok()
        .map(|o| o.status.success() && !o.stdout.is_empty())
        .unwrap_or(false);

    let hostname = Command::new("hostname")
        .arg("-s")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();

    ShellContext {
        cwd,
        git_branch,
        git_dirty,
        hostname,
    }
}

// ── Transport bar view (read-only pills, no input) ──

pub struct TransportBar {
    focus_handle: FocusHandle,
    shell_ctx: Arc<Mutex<ShellContext>>,
}

impl TransportBar {
    fn new(cx: &mut Context<Self>) -> Self {
        // Poll shell context every 2s
        let shell_ctx = Arc::new(Mutex::new(poll_shell_context()));
        let ctx_clone = shell_ctx.clone();
        cx.spawn(async move |this, cx: &mut AsyncApp| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_secs(2))
                    .await;
                let new_ctx = poll_shell_context();
                if let Ok(mut ctx) = ctx_clone.lock() {
                    *ctx = new_ctx;
                }
                let _ = cx.update(|cx| {
                    let _ = this.update(cx, |_, cx| cx.notify());
                });
            }
        })
        .detach();

        Self {
            focus_handle: cx.focus_handle(),
            shell_ctx,
        }
    }
}

impl Focusable for TransportBar {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TransportBar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = theme(cx).colors.clone();
        let ctx = self.shell_ctx.lock().map(|c| c.clone()).unwrap_or_default();

        let pill_bg = colors.elevated_surface_background;
        let pill_text = colors.text_muted;
        let accent = colors.text_accent;

        let mut pills = div()
            .flex()
            .flex_row()
            .gap(px(6.0))
            .items_center();

        // Hostname pill
        if !ctx.hostname.is_empty() {
            pills = pills.child(
                div()
                    .px(px(8.0))
                    .py(px(2.0))
                    .rounded_full()
                    .bg(pill_bg)
                    .text_size(px(11.0))
                    .text_color(pill_text)
                    .child(SharedString::from(ctx.hostname.clone())),
            );
        }

        // CWD pill
        if !ctx.cwd.is_empty() {
            pills = pills.child(
                div()
                    .px(px(8.0))
                    .py(px(2.0))
                    .rounded_full()
                    .bg(pill_bg)
                    .text_size(px(11.0))
                    .text_color(accent)
                    .child(SharedString::from(ctx.cwd.clone())),
            );
        }

        // Git branch pill
        if !ctx.git_branch.is_empty() {
            let branch_label = if ctx.git_dirty {
                format!("{} *", ctx.git_branch)
            } else {
                ctx.git_branch.clone()
            };
            pills = pills.child(
                div()
                    .px(px(8.0))
                    .py(px(2.0))
                    .rounded_full()
                    .bg(pill_bg)
                    .text_size(px(11.0))
                    .text_color(if ctx.git_dirty {
                        hsla(0.08, 0.9, 0.65, 1.0)
                    } else {
                        hsla(0.35, 0.7, 0.55, 1.0)
                    })
                    .child(SharedString::from(branch_label)),
            );
        }

        div()
            .id("transport-bar")
            .track_focus(&self.focus_handle)
            .w_full()
            .h_full()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .px(px(10.0))
            .bg(hsla(0.0, 0.0, 0.04, 0.85))
            .border_t_1()
            .border_color(hsla(0.0, 0.0, 0.2, 0.5))
            .child(pills)
    }
}

// ── Public API ──

/// Get Ghostty window bounds via AppleScript (blocking)
fn ghostty_window_bounds() -> Option<(f32, f32, f32, f32)> {
    let script = r#"tell application "System Events"
    tell process "Ghostty"
        if (count of windows) > 0 then
            set w to window 1
            set {x, y} to position of w
            set {width, height} to size of w
            return (x as text) & "," & (y as text) & "," & (width as text) & "," & (height as text)
        end if
    end tell
end tell"#;
    let output = Command::new("osascript").arg("-e").arg(script).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let parts: Vec<f32> = text.split(',').filter_map(|s| s.trim().parse().ok()).collect();
    if parts.len() == 4 {
        Some((parts[0], parts[1], parts[2], parts[3]))
    } else {
        None
    }
}

/// Open Ghostty and optionally run a command.
/// If Ghostty is already focused, just send the command.
/// If a command is given from the picker (> cmd), open/focus Ghostty and type it.
pub fn open_ghostty(cmd: Option<&str>, cx: &mut App) {
    let ghostty_focused = is_ghostty_focused();

    if let Some(cmd) = cmd {
        if ghostty_focused {
            // Ghostty is already open and focused — just send the command
            send_text_to_ghostty(cmd);
            return;
        }
    }

    // Launch or focus Ghostty
    spawn_ghostty();

    let cmd_owned = cmd.map(|s| s.to_string());

    cx.spawn(async move |cx: &mut AsyncApp| {
        cx.background_executor()
            .timer(Duration::from_millis(800))
            .await;

        if let Some(cmd) = &cmd_owned {
            send_text_to_ghostty(cmd);
        }

        let bounds = cx
            .background_executor()
            .spawn(async { ghostty_window_bounds() })
            .await;

        let _ = cx.update(|cx| {
            open_transport_bar(bounds, cx);
        });
    })
    .detach();
}

fn open_transport_bar(ghostty_bounds: Option<(f32, f32, f32, f32)>, cx: &mut App) {
    let bar_h = 28.0f32;

    let (x, y, w) = if let Some((gx, gy, _gw, gh)) = ghostty_bounds {
        (gx, gy + gh - bar_h, _gw)
    } else {
        let (sw, _sh) = crate::screen_size();
        let w = 640.0f32;
        ((sw - w) / 2.0, 600.0, w)
    };

    let _ = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: point(px(x), px(y)),
                size: size(px(w), px(bar_h)),
            })),
            titlebar: None,
            kind: WindowKind::PopUp,
            focus: false,
            window_background: WindowBackgroundAppearance::Transparent,
            ..Default::default()
        },
        |_window, cx| cx.new(|cx| TransportBar::new(cx)),
    );
}
