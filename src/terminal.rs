use gpui::*;
use std::path::PathBuf;
use std::process::Command;
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

    if let Some(cmd) = cmd {
        let cmd_owned = cmd.to_string();
        cx.spawn(async move |cx: &mut AsyncApp| {
            cx.background_executor()
                .timer(Duration::from_millis(800))
                .await;
            send_text_to_ghostty(&cmd_owned);
        })
        .detach();
    }
}
