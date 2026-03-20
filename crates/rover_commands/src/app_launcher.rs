use crate::{Command, CommandKind, CommandRegistry};
use std::path::PathBuf;

/// Terminal emulator app names to exclude from the launcher list.
/// Rover has its own built-in terminal; these apps just confuse the picker.
const TERMINAL_APPS: &[&str] = &[
    "Terminal",
    "iTerm2",
    "iTerm",
    "Alacritty",
    "Kitty",
    "WezTerm",
    "Warp",
    "Ghostty",
    "Hyper",
];

/// Scan /Applications and ~/Applications for .app bundles.
pub fn discover_apps() -> Vec<Command> {
    let mut apps = Vec::new();
    let search_dirs = [
        PathBuf::from("/Applications"),
        PathBuf::from("/Applications/Utilities"),
        PathBuf::from("/System/Applications"),
        PathBuf::from("/System/Applications/Utilities"),
        dirs::home_dir().unwrap_or_default().join("Applications"),
    ];

    for dir in &search_dirs {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "app") {
                    let name = path
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    if TERMINAL_APPS.iter().any(|t| name.eq_ignore_ascii_case(t)) {
                        continue;
                    }
                    apps.push(Command {
                        name: name.clone(),
                        description: format!("Launch {}", name),
                        icon: None,
                        kind: CommandKind::LaunchApp {
                            path: path.to_string_lossy().to_string(),
                        },
                        has_submenu: false,
                    });
                }
            }
        }
    }

    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    apps
}

/// Register all discovered apps into a CommandRegistry.
pub fn register_apps(registry: &mut CommandRegistry) {
    for app in discover_apps() {
        registry.register(app);
    }
}
