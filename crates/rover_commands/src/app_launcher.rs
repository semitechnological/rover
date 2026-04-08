use crate::{Command, CommandKind, CommandRegistry};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

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
    static CACHE: OnceLock<Mutex<(Instant, Vec<Command>)>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| {
        Mutex::new((
            Instant::now().checked_sub(Duration::from_secs(120)).unwrap_or_else(Instant::now),
            Vec::new(),
        ))
    });
    if let Ok(guard) = cache.lock() {
        if !guard.1.is_empty() && guard.0.elapsed() < Duration::from_secs(30) {
            return guard.1.clone();
        }
    }

    let mut apps = Vec::new();
    let search_dirs = [
        PathBuf::from("/Applications"),
        PathBuf::from("/Applications/Utilities"),
        PathBuf::from("/System/Applications"),
        PathBuf::from("/System/Applications/Utilities"),
        PathBuf::from("/opt/homebrew/Applications"),
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
    if let Ok(mut guard) = cache.lock() {
        *guard = (Instant::now(), apps.clone());
    }
    apps
}

/// Register all discovered apps into a CommandRegistry.
pub fn register_apps(registry: &mut CommandRegistry) {
    for app in discover_apps() {
        registry.register(app);
    }
}
