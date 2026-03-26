//! Editor plugin — opens files in the user's preferred editor.
//!
//! Match mode: `FuzzyCommand` — provides an "Open in Editor" command and
//! supports opening recently-used files.

use rover_plugin_api::*;
use std::any::Any;

/// Supported editor applications.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EditorApp {
    Zed,
    VSCode,
    Xcode,
    Sublime,
    Vim,
    Default,
}

impl EditorApp {
    fn name(&self) -> &'static str {
        match self {
            Self::Zed => "Zed",
            Self::VSCode => "Visual Studio Code",
            Self::Xcode => "Xcode",
            Self::Sublime => "Sublime Text",
            Self::Vim => "MacVim",
            Self::Default => "Default Editor",
        }
    }

    /// Command to open a file path in this editor.
    fn open_command(&self, path: &str) -> (String, Vec<String>) {
        match self {
            Self::Zed => ("zed".to_string(), vec![path.to_string()]),
            Self::VSCode => ("code".to_string(), vec![path.to_string()]),
            Self::Xcode => (
                "open".to_string(),
                vec!["-a".to_string(), "Xcode".to_string(), path.to_string()],
            ),
            Self::Sublime => ("subl".to_string(), vec![path.to_string()]),
            Self::Vim => ("mvim".to_string(), vec![path.to_string()]),
            Self::Default => ("open".to_string(), vec!["-t".to_string(), path.to_string()]),
        }
    }
}

pub struct EditorPlugin {
    preferred_editor: EditorApp,
}

impl EditorPlugin {
    pub fn new() -> Self {
        Self {
            preferred_editor: EditorApp::Zed,
        }
    }

    /// Detect which editors are installed and pick the preferred one.
    fn detect_editor(&mut self) {
        // Check for common editors in order of preference
        let editors = [
            (EditorApp::Zed, "zed"),
            (EditorApp::VSCode, "code"),
            (EditorApp::Sublime, "subl"),
        ];

        for (editor, cmd) in &editors {
            if std::process::Command::new("which")
                .arg(cmd)
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
            {
                self.preferred_editor = *editor;
                return;
            }
        }

        self.preferred_editor = EditorApp::Default;
    }
}

impl Default for EditorPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for EditorPlugin {
    fn id(&self) -> &str {
        "builtin.editor"
    }

    fn name(&self) -> &str {
        "Editor"
    }

    fn description(&self) -> &str {
        "Open files in your preferred editor"
    }

    fn on_load(&mut self, _ctx: &mut PluginContext) -> Result<(), PluginError> {
        self.detect_editor();
        log::info!(
            "EditorPlugin: preferred editor is {}",
            self.preferred_editor.name()
        );
        Ok(())
    }

    fn match_mode(&self) -> MatchMode {
        MatchMode::FuzzyCommand
    }

    fn priority(&self) -> i32 {
        6
    }

    fn commands(&self) -> Vec<CommandDescriptor> {
        vec![CommandDescriptor {
            id: "open_in_editor".to_string(),
            name: format!("Open in {}", self.preferred_editor.name()),
            description: format!("Open a file in {}", self.preferred_editor.name()),
            icon: Some(IconHint::SystemSymbol("doc.text".to_string())),
            keywords: vec![
                "edit".to_string(),
                "open".to_string(),
                "editor".to_string(),
                "code".to_string(),
                "file".to_string(),
            ],
            has_submenu: false,
        }]
    }

    fn on_query(&mut self, _query: &str, _ctx: &PluginContext) -> Vec<MatchResult> {
        // FuzzyCommand mode — matched by the registry's fuzzy layer.
        vec![]
    }

    fn on_confirm(
        &mut self,
        match_id: &str,
        query: &str,
        _secondary: bool,
        _ctx: &mut PluginContext,
    ) -> ConfirmResult {
        if match_id == "open_in_editor" {
            // If the query looks like a file path, open it directly.
            let path = query.trim();
            if !path.is_empty()
                && (path.starts_with('/') || path.starts_with('~') || path.starts_with('.'))
            {
                let expanded = if path.starts_with('~') {
                    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
                    path.replacen('~', &home, 1)
                } else {
                    path.to_string()
                };

                let (cmd, args) = self.preferred_editor.open_command(&expanded);
                let _ = std::process::Command::new(&cmd).args(&args).spawn();
                return ConfirmResult::Dismiss;
            }

            // No path provided — just activate the editor app.
            let script = format!(
                r#"tell application "{}" to activate"#,
                self.preferred_editor.name().replace('"', "\\\"")
            );
            let _ = std::process::Command::new("osascript")
                .arg("-e")
                .arg(&script)
                .spawn();
            return ConfirmResult::Dismiss;
        }
        ConfirmResult::Stay
    }

    fn actions_for_match(&self, match_id: &str) -> Vec<ActionDescriptor> {
        if match_id == "open_in_editor" {
            vec![
                ActionDescriptor {
                    id: "open".to_string(),
                    name: format!("Open in {}", self.preferred_editor.name()),
                    shortcut: Some("Enter".to_string()),
                },
                ActionDescriptor {
                    id: "open_default".to_string(),
                    name: "Open in default editor".to_string(),
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
            "open" => self.on_confirm(match_id, "", false, ctx),
            "open_default" => {
                let script = r#"tell application "Finder" to activate"#;
                let _ = std::process::Command::new("osascript")
                    .arg("-e")
                    .arg(script)
                    .spawn();
                ConfirmResult::Dismiss
            }
            _ => ConfirmResult::Stay,
        }
    }

    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
}
