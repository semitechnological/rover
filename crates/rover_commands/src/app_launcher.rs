use crate::{Command, CommandKind, CommandRegistry};
use std::path::PathBuf;

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
