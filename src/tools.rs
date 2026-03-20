use rusty_foundationmodels::{Schema, SchemaProperty, SchemaPropertyType, ToolDefinition};

pub fn build_os_tools() -> Vec<ToolDefinition> {
    vec![
        applescript_tool(),
        open_url_tool(),
        launch_app_tool(),
        shell_tool(),
        system_info_tool(),
        get_frontmost_app_tool(),
    ]
}

fn applescript_tool() -> ToolDefinition {
    ToolDefinition::new(
        "run_applescript",
        "Run an AppleScript and return its output",
        Schema::new("RunApplescriptArgs")
            .property(SchemaProperty::new("script", SchemaPropertyType::String).description("The AppleScript source to execute")),
        |args| {
            let script = args["script"].as_str().unwrap_or("").to_string();
            let output = std::process::Command::new("osascript")
                .arg("-e")
                .arg(&script)
                .output()
                .map_err(|e| e.to_string())?;
            if output.status.success() {
                Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
            } else {
                Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
            }
        },
    )
}

fn open_url_tool() -> ToolDefinition {
    ToolDefinition::new(
        "open_url",
        "Open a URL in the default browser",
        Schema::new("OpenUrlArgs")
            .property(SchemaProperty::new("url", SchemaPropertyType::String).description("The URL to open")),
        |args| {
            let url = args["url"].as_str().unwrap_or("").to_string();
            std::process::Command::new("open")
                .arg(&url)
                .spawn()
                .map_err(|e| e.to_string())?;
            Ok(format!("Opened {url}"))
        },
    )
}

fn launch_app_tool() -> ToolDefinition {
    ToolDefinition::new(
        "launch_app",
        "Launch a macOS application by name",
        Schema::new("LaunchAppArgs")
            .property(SchemaProperty::new("name", SchemaPropertyType::String).description("Application name")),
        |args| {
            let name = args["name"].as_str().unwrap_or("").to_string();
            std::process::Command::new("open")
                .arg("-a")
                .arg(&name)
                .spawn()
                .map_err(|e| e.to_string())?;
            Ok(format!("Launched {name}"))
        },
    )
}

fn shell_tool() -> ToolDefinition {
    ToolDefinition::new(
        "run_shell",
        "Run a shell command and return its output",
        Schema::new("RunShellArgs")
            .property(SchemaProperty::new("command", SchemaPropertyType::String).description("Shell command to execute")),
        |args| {
            let command = args["command"].as_str().unwrap_or("").to_string();
            let output = std::process::Command::new("sh")
                .arg("-c")
                .arg(&command)
                .output()
                .map_err(|e| e.to_string())?;
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            if output.status.success() {
                Ok(stdout)
            } else {
                Ok(format!("stdout: {stdout}\nstderr: {stderr}"))
            }
        },
    )
}

fn system_info_tool() -> ToolDefinition {
    ToolDefinition::new(
        "get_system_info",
        "Get macOS version and hostname information",
        Schema::new("GetSystemInfoArgs"),
        |_args| {
            let sw_vers = std::process::Command::new("sw_vers")
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
                .unwrap_or_default();
            let hostname = std::process::Command::new("hostname")
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_default();
            Ok(format!("{sw_vers}\nHostname: {hostname}"))
        },
    )
}

fn get_frontmost_app_tool() -> ToolDefinition {
    ToolDefinition::new(
        "get_frontmost_app",
        "Get the name of the currently frontmost (focused) application",
        Schema::new("GetFrontmostAppArgs"),
        |_args| {
            let script = r#"tell application "System Events" to get name of first application process whose frontmost is true"#;
            let output = std::process::Command::new("osascript")
                .arg("-e")
                .arg(script)
                .output()
                .map_err(|e| e.to_string())?;
            if output.status.success() {
                Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
            } else {
                Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
            }
        },
    )
}
