//! App Launcher plugin — scans macOS application directories and provides
//! fuzzy-matched app launch commands.
//!
//! Match mode: `FuzzyCommand` — commands are registered and matched via the
//! central fuzzy matcher.

use rover_commands::app_launcher::discover_apps;
use rover_fuzzy::{match_strings, StringMatch, StringMatchCandidate};
use rover_plugin_api::*;

/// Info about a discovered macOS application.
#[derive(Debug, Clone)]
struct AppInfo {
    name: String,
    path: String,
}

pub struct AppLauncherPlugin {
    apps: Vec<AppInfo>,
    candidates: Vec<StringMatchCandidate>,
    last_matches: Vec<(StringMatch, usize)>,
}

impl AppLauncherPlugin {
    pub fn new() -> Self {
        Self {
            apps: Vec::new(),
            candidates: Vec::new(),
            last_matches: Vec::new(),
        }
    }

    fn rescan_apps(&mut self) {
        let commands = discover_apps();
        self.apps = commands
            .into_iter()
            .filter_map(|cmd| {
                if let rover_commands::CommandKind::LaunchApp { path } = cmd.kind {
                    Some(AppInfo {
                        name: cmd.name,
                        path,
                    })
                } else {
                    None
                }
            })
            .collect();

        self.candidates = self
            .apps
            .iter()
            .enumerate()
            .map(|(i, app)| StringMatchCandidate::new(i, &app.name))
            .collect();
    }
}

impl Default for AppLauncherPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for AppLauncherPlugin {
    fn id(&self) -> &str {
        "builtin.app_launcher"
    }
    fn name(&self) -> &str {
        "App Launcher"
    }
    fn description(&self) -> &str {
        "Launch macOS applications"
    }

    fn on_load(&mut self, _ctx: &mut PluginContext) -> Result<(), PluginError> {
        self.rescan_apps();
        log::info!("AppLauncherPlugin: discovered {} apps", self.apps.len());
        Ok(())
    }

    fn match_mode(&self) -> MatchMode {
        MatchMode::FuzzyCommand
    }

    fn priority(&self) -> i32 {
        10
    }

    fn commands(&self) -> Vec<CommandDescriptor> {
        self.apps
            .iter()
            .map(|app| CommandDescriptor {
                id: format!("launch:{}", app.name),
                name: app.name.clone(),
                description: format!("Launch {}", app.name),
                icon: Some(IconHint::AppIcon(app.name.clone())),
                keywords: vec![],
                has_submenu: false,
            })
            .collect()
    }

    fn on_query(&mut self, query: &str, _ctx: &PluginContext) -> Vec<MatchResult> {
        let fuzzy_matches = match_strings(&self.candidates, query, 50);

        self.last_matches = fuzzy_matches
            .iter()
            .map(|m| (m.clone(), m.candidate_id))
            .collect();

        fuzzy_matches
            .into_iter()
            .filter_map(|m| {
                let app = self.apps.get(m.candidate_id)?;
                Some(MatchResult {
                    id: format!("launch:{}", app.name),
                    title: app.name.clone(),
                    subtitle: Some(format!("Launch {}", app.name)),
                    icon: Some(IconHint::AppIcon(app.name.clone())),
                    score: m.score as f64,
                    plugin_id: self.id().to_string(),
                })
            })
            .collect()
    }

    fn on_confirm(
        &mut self,
        match_id: &str,
        _query: &str,
        _secondary: bool,
        _ctx: &mut PluginContext,
    ) -> ConfirmResult {
        if let Some(app_name) = match_id.strip_prefix("launch:") {
            if let Some(app) = self.apps.iter().find(|a| a.name == app_name) {
                // Use AppleScript to activate (per CLAUDE.md: don't use `open -a`)
                let script = format!(
                    r#"tell application "{}" to activate"#,
                    app.name.replace('"', "\\\"")
                );
                let _ = std::process::Command::new("osascript")
                    .arg("-e")
                    .arg(&script)
                    .spawn();
                return ConfirmResult::Dismiss;
            }
        }
        ConfirmResult::Stay
    }

    fn actions_for_match(&self, match_id: &str) -> Vec<ActionDescriptor> {
        if match_id.starts_with("launch:") {
            vec![
                ActionDescriptor {
                    id: "launch".to_string(),
                    name: "Launch".to_string(),
                    shortcut: Some("Enter".to_string()),
                },
                ActionDescriptor {
                    id: "reveal".to_string(),
                    name: "Reveal in Finder".to_string(),
                    shortcut: None,
                },
            ]
        } else {
            vec![]
        }
    }

    fn execute_action(
        &mut self,
        match_id: &str,
        action_id: &str,
        ctx: &mut PluginContext,
    ) -> ConfirmResult {
        match action_id {
            "launch" => self.on_confirm(match_id, "", false, ctx),
            "reveal" => {
                if let Some(app_name) = match_id.strip_prefix("launch:") {
                    if let Some(app) = self.apps.iter().find(|a| a.name == app_name) {
                        let _ = std::process::Command::new("open")
                            .arg("-R")
                            .arg(&app.path)
                            .spawn();
                        return ConfirmResult::Dismiss;
                    }
                }
                ConfirmResult::Stay
            },
            _ => ConfirmResult::Stay,
        }
    }
}
