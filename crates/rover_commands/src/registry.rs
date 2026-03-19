use std::sync::Arc;

/// A command that can appear in the picker.
#[derive(Clone)]
pub struct Command {
    pub name: String,
    pub description: String,
    pub icon: Option<String>,
    pub kind: CommandKind,
}

#[derive(Clone)]
pub enum CommandKind {
    /// Launch an application at the given path.
    LaunchApp { path: String },
    /// Evaluate a math expression.
    Calculator { expression: String },
    /// Open a URL.
    OpenUrl { url: String },
    /// A custom action callback.
    Action(Arc<dyn Fn() + Send + Sync>),
}

pub struct CommandRegistry {
    commands: Vec<Command>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
        }
    }

    pub fn register(&mut self, command: Command) {
        self.commands.push(command);
    }

    pub fn commands(&self) -> &[Command] {
        &self.commands
    }

    pub fn execute(command: &Command) {
        match &command.kind {
            CommandKind::LaunchApp { path } => {
                let _ = std::process::Command::new("open")
                    .arg("-a")
                    .arg(path)
                    .spawn();
            }
            CommandKind::OpenUrl { url } => {
                let _ = std::process::Command::new("open").arg(url).spawn();
            }
            CommandKind::Calculator { .. } => {}
            CommandKind::Action(action) => action(),
        }
    }
}
