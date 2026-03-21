use std::sync::Arc;

#[derive(Clone)]
pub struct Command {
    pub name: String,
    pub description: String,
    pub icon: Option<String>,
    pub kind: CommandKind,
    /// If true, this command opens a sub-menu when confirmed
    pub has_submenu: bool,
}

#[derive(Clone)]
pub enum CommandKind {
    LaunchApp {
        path: String,
    },
    Calculator {
        expression: String,
    },
    OpenUrl {
        url: String,
    },
    Action(Arc<dyn Fn() + Send + Sync>),
    /// A built-in submenu command (e.g. Clipboard History, File Search)
    BuiltIn {
        id: BuiltInCommand,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BuiltInCommand {
    ClipboardHistory,
    FileSearch,
    Settings,
    Snippets,
    OpenTerminal,
    Browser,
    Help,
    CloseWindow,
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
            CommandKind::BuiltIn { .. } => {}
        }
    }
}

/// Register the browser command
pub fn register_browser(registry: &mut CommandRegistry) {
    registry.register(Command {
        name: "Open Browser".into(),
        description: "Open the keyboard-driven browser".into(),
        icon: None,
        kind: CommandKind::BuiltIn {
            id: BuiltInCommand::Browser,
        },
        has_submenu: false,
    });
}

/// Register built-in commands that open sub-menus
pub fn register_builtins(registry: &mut CommandRegistry) {
    registry.register(Command {
        name: "Clipboard History".into(),
        description: "View and paste from clipboard history".into(),
        icon: None,
        kind: CommandKind::BuiltIn {
            id: BuiltInCommand::ClipboardHistory,
        },
        has_submenu: true,
    });
    registry.register(Command {
        name: "File Search".into(),
        description: "Search for files on your system".into(),
        icon: None,
        kind: CommandKind::BuiltIn {
            id: BuiltInCommand::FileSearch,
        },
        has_submenu: true,
    });
    registry.register(Command {
        name: "Settings".into(),
        description: "Open Rover settings".into(),
        icon: None,
        kind: CommandKind::BuiltIn {
            id: BuiltInCommand::Settings,
        },
        has_submenu: false,
    });
    registry.register(Command {
        name: "Terminal".into(),
        description: "Open floating terminal window".into(),
        icon: None,
        kind: CommandKind::BuiltIn {
            id: BuiltInCommand::OpenTerminal,
        },
        has_submenu: false,
    });
    registry.register(Command {
        name: "Help".into(),
        description: "Show Rover keyboard shortcuts and tips".into(),
        icon: None,
        kind: CommandKind::BuiltIn {
            id: BuiltInCommand::Help,
        },
        has_submenu: false,
    });
    registry.register(Command {
        name: "Close Window".into(),
        description: "Close the frontmost window".into(),
        icon: None,
        kind: CommandKind::BuiltIn {
            id: BuiltInCommand::CloseWindow,
        },
        has_submenu: false,
    });
}
