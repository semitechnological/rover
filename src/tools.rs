use rusty_foundationmodels::{Schema, SchemaProperty, SchemaPropertyType, ToolDefinition};
use std::sync::atomic::{AtomicBool, Ordering};

static YOLO_MODE: AtomicBool = AtomicBool::new(false);

pub(crate) fn set_yolo_mode(enabled: bool) {
    YOLO_MODE.store(enabled, Ordering::Relaxed);
}

/// Escapes a string for safe embedding inside an AppleScript double-quoted string literal.
///
/// AppleScript has no backslash escape sequences; double-quotes are represented using
/// the built-in `quote` constant via string concatenation. Control characters (newlines,
/// carriage returns, etc.) are replaced with spaces to prevent statement injection.
pub(crate) fn escape_applescript(s: &str) -> String {
    let sanitized: String = s
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    sanitized.replace('"', "\" & quote & \"")
}

/// Run an AppleScript via `osascript -e` and return trimmed stdout on success
/// or trimmed stderr as an `Err` on failure.
fn run_osascript(script: &str) -> Result<String, String> {
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
}

/// When Yolo Mode is off, shows a native macOS confirmation dialog before executing
/// a sensitive AI tool action. Returns Err if the user denies or dismisses.
fn require_permission(action: &str) -> Result<(), String> {
    if YOLO_MODE.load(Ordering::Relaxed) {
        return Ok(());
    }
    let safe = escape_applescript(action);
    let script = format!(
        r#"display dialog "{safe}" with title "Rover" buttons {{"Deny", "Allow"}} default button "Allow" cancel button "Deny""#
    );
    match run_osascript(&script) {
        Ok(_) => Ok(()),
        Err(_) => Err(
            "Permission denied. Enable Yolo Mode in Settings \u{2192} AI to allow AI tool access without prompts.".to_string()
        ),
    }
}

/// Shared shell execution with 64 KB output truncation.
const SHELL_OUTPUT_LIMIT: usize = 65_536;

fn run_shell_command(command: &str) -> Result<String, String> {
    let output = std::process::Command::new("sh")
        .arg("-c")
        .arg(command)
        .output()
        .map_err(|e| e.to_string())?;
    let mut stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let mut stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    if stdout.len() > SHELL_OUTPUT_LIMIT {
        stdout.truncate(SHELL_OUTPUT_LIMIT);
        stdout.push_str("\n[output truncated]");
    }
    if stderr.len() > SHELL_OUTPUT_LIMIT {
        stderr.truncate(SHELL_OUTPUT_LIMIT);
        stderr.push_str("\n[output truncated]");
    }
    if output.status.success() {
        Ok(stdout)
    } else {
        Ok(format!("stdout: {stdout}\nstderr: {stderr}"))
    }
}

pub fn build_os_tools() -> Vec<ToolDefinition> {
    vec![
        applescript_tool(),
        open_url_tool(),
        launch_app_tool(),
        shell_tool(),
        system_info_tool(),
        get_frontmost_app_tool(),
        // Calendar / Siri-like
        get_calendar_events_tool(),
        get_reminders_tool(),
        add_reminder_tool(),
        get_weather_tool(),
        control_music_tool(),
        create_note_tool(),
        // Knowledge
        search_wikipedia_tool(),
        // Device / system resources
        get_system_resources_tool(),
        get_network_info_tool(),
        // Clipboard
        get_clipboard_tool(),
        set_clipboard_tool(),
        // Notifications
        send_notification_tool(),
        // Volume
        get_volume_tool(),
        set_volume_tool(),
        // Screenshot
        take_screenshot_tool(),
        // Contacts & messaging
        lookup_contact_tool(),
        send_imessage_tool(),
        send_email_tool(),
        // Discovery
        spotlight_search_tool(),
        list_bluetooth_devices_tool(),
        // Keychain (read-only)
        get_keychain_secret_tool(),
        // Window management
        close_window_tool(),
        quit_app_tool(),
        // Browser tab management
        close_browser_tab_tool(),
        // Inline command execution
        run_inline_command_tool(),
    ]
}

fn applescript_tool() -> ToolDefinition {
    ToolDefinition::new(
        "run_applescript",
        "Run an AppleScript and return its output",
        Schema::new("RunApplescriptArgs")
            .property(SchemaProperty::new("script", SchemaPropertyType::String).description("The AppleScript source to execute")),
        |args| {
            let script = args["script"].as_str().unwrap_or("");
            let preview = script.chars().take(80).collect::<String>();
            require_permission(&format!("Run AppleScript: {preview}"))?;
            run_osascript(script)
        },
    )
}

fn open_url_tool() -> ToolDefinition {
    ToolDefinition::new(
        "open_url",
        "Open a URL in the default browser using AppleScript",
        Schema::new("OpenUrlArgs")
            .property(SchemaProperty::new("url", SchemaPropertyType::String).description("The URL to open")),
        |args| {
            let raw_url = args["url"].as_str().unwrap_or("");
            let safe_url = escape_applescript(raw_url);
            let script = format!(
                r#"tell application "Safari"
    activate
    open location "{safe_url}"
end tell"#
            );
            match run_osascript(&script) {
                Ok(_) => Ok(format!("Opened {raw_url}")),
                Err(_) => {
                    std::process::Command::new("open")
                        .arg(raw_url)
                        .spawn()
                        .map_err(|e| e.to_string())?;
                    Ok(format!("Opened {raw_url}"))
                }
            }
        },
    )
}

fn launch_app_tool() -> ToolDefinition {
    ToolDefinition::new(
        "launch_app",
        "Activate or launch a macOS application by name using AppleScript. Brings the app to the foreground.",
        Schema::new("LaunchAppArgs")
            .property(SchemaProperty::new("name", SchemaPropertyType::String).description("Application name (e.g. 'Finder', 'Safari', 'Mail')")),
        |args| {
            let name = escape_applescript(args["name"].as_str().unwrap_or(""));
            let script = format!(r#"tell application "{name}" to activate"#);
            run_osascript(&script)?;
            Ok(format!("Activated {name}"))
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
            require_permission(&format!("Run shell command: {command}"))?;
            run_shell_command(&command)
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
            run_osascript(r#"tell application "System Events" to get name of first application process whose frontmost is true"#)
        },
    )
}

// ─── Calendar ────────────────────────────────────────────────────────────────

fn get_calendar_events_tool() -> ToolDefinition {
    ToolDefinition::new(
        "get_calendar_events",
        "Get events from the macOS Calendar app. Use when the user asks about their schedule, \
         appointments, or what's on their calendar for a day ('today', 'tomorrow', a date like '2026-03-22').",
        Schema::new("GetCalendarEventsArgs").property(
            SchemaProperty::new("date", SchemaPropertyType::String)
                .description("'today', 'tomorrow', or a date in YYYY-MM-DD format"),
        ),
        |args| {
            let date_str = args["date"].as_str().unwrap_or("today");
            let date_setup = match date_str {
                "today" => "set d to current date\nset hours of d to 0\nset minutes of d to 0\nset seconds of d to 0".to_string(),
                "tomorrow" => "set d to current date\nset hours of d to 0\nset minutes of d to 0\nset seconds of d to 0\nset d to d + (1 * days)".to_string(),
                other => {
                    // Validate: only accept YYYY-MM-DD to prevent injection
                    let valid = other.len() == 10
                        && other.chars().enumerate().all(|(i, c)| match i {
                            4 | 7 => c == '-',
                            _ => c.is_ascii_digit(),
                        });
                    if !valid {
                        return Err(format!("Invalid date format '{other}': use YYYY-MM-DD, 'today', or 'tomorrow'"));
                    }
                    format!("set d to date \"{other}\"\nset hours of d to 0\nset minutes of d to 0\nset seconds of d to 0")
                }
            };
            let script = format!(
                r#"tell application "Calendar"
    {date_setup}
    set nextD to d + (1 * days)
    set results to ""
    repeat with aCal in calendars
        try
            set evts to (every event of aCal whose start date >= d and start date < nextD)
            repeat with e in evts
                set results to results & (summary of e) & " at " & (time string of (start date of e)) & linefeed
            end repeat
        end try
    end repeat
    if results is "" then return "No events"
    return results
end tell"#
            );
            run_osascript(&script)
        },
    )
}

// ─── Reminders ───────────────────────────────────────────────────────────────

fn get_reminders_tool() -> ToolDefinition {
    ToolDefinition::new(
        "get_reminders",
        "Get incomplete reminders from the macOS Reminders app. Use when the user asks what they need to do or what reminders they have.",
        Schema::new("GetRemindersArgs").property(
            SchemaProperty::new("list_name", SchemaPropertyType::String)
                .description("Reminders list name, or 'all' for every list")
                .optional(),
        ),
        |args| {
            let list_name_raw = args["list_name"].as_str().unwrap_or("all");
            let filter = if list_name_raw == "all" {
                "every list".to_string()
            } else {
                let list_name = escape_applescript(list_name_raw);
                format!("list \"{list_name}\"")
            };
            let script = format!(
                r#"tell application "Reminders"
    set results to ""
    repeat with l in {filter}
        repeat with r in (every reminder of l whose completed is false)
            set results to results & (name of r) & linefeed
        end repeat
    end repeat
    if results is "" then return "No reminders"
    return results
end tell"#
            );
            run_osascript(&script)
        },
    )
}

fn add_reminder_tool() -> ToolDefinition {
    ToolDefinition::new(
        "add_reminder",
        "Add a new reminder to the macOS Reminders app.",
        Schema::new("AddReminderArgs")
            .property(SchemaProperty::new("name", SchemaPropertyType::String).description("Reminder text"))
            .property(
                SchemaProperty::new("list_name", SchemaPropertyType::String)
                    .description("List to add to (default: Reminders)")
                    .optional(),
            ),
        |args| {
            let name = escape_applescript(args["name"].as_str().unwrap_or(""));
            let list = escape_applescript(args["list_name"].as_str().unwrap_or("Reminders"));
            require_permission(&format!("Add reminder: {name}"))?;
            let script = format!(
                r#"tell application "Reminders"
    tell list "{list}"
        make new reminder with properties {{name:"{name}"}}
    end tell
end tell"#
            );
            run_osascript(&script)?;
            Ok(format!("Added reminder: {name}"))
        },
    )
}

// ─── Weather ─────────────────────────────────────────────────────────────────

fn get_weather_tool() -> ToolDefinition {
    ToolDefinition::new(
        "get_weather",
        "Get current weather for a location. Use when the user asks about weather or temperature.",
        Schema::new("GetWeatherArgs").property(
            SchemaProperty::new("location", SchemaPropertyType::String)
                .description("City or location (e.g. 'London', 'New York')"),
        ),
        |args| {
            let location = args["location"].as_str().unwrap_or("").replace(' ', "+");
            let url = format!("https://wttr.in/{location}?format=3");
            let out = std::process::Command::new("curl")
                .args(["-s", "--max-time", "5", &url])
                .output()
                .map_err(|e| e.to_string())?;
            let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if text.is_empty() { Err("Could not fetch weather".into()) } else { Ok(text) }
        },
    )
}

// ─── Music ───────────────────────────────────────────────────────────────────

fn control_music_tool() -> ToolDefinition {
    ToolDefinition::new(
        "control_music",
        "Control music playback in the macOS Music app. Actions: play, pause, next, previous, \
         stop, get_current, play_track. Use when the user asks to play music, skip tracks, etc.",
        Schema::new("ControlMusicArgs")
            .property(SchemaProperty::new("action", SchemaPropertyType::String)
                .description("play, pause, next, previous, stop, get_current, or play_track"))
            .property(SchemaProperty::new("track_name", SchemaPropertyType::String)
                .description("Track name to find and play (only for play_track)")
                .optional()),
        |args| {
            let action = args["action"].as_str().unwrap_or("get_current");
            let script = match action {
                "play"     => r#"tell application "Music" to play"#.to_string(),
                "pause"    => r#"tell application "Music" to pause"#.to_string(),
                "next"     => r#"tell application "Music" to next track"#.to_string(),
                "previous" => r#"tell application "Music" to previous track"#.to_string(),
                "stop"     => r#"tell application "Music" to stop"#.to_string(),
                "get_current" => r#"tell application "Music"
    if player state is playing then
        set t to current track
        return (name of t) & " by " & (artist of t)
    else
        return "Nothing playing"
    end if
end tell"#.to_string(),
                "play_track" => {
                    let name = escape_applescript(args["track_name"].as_str().unwrap_or(""));
                    format!(r#"tell application "Music"
    set results to (every track whose name contains "{name}")
    if results is not {{}} then
        play item 1 of results
        return "Playing " & (name of item 1 of results)
    else
        return "Track not found"
    end if
end tell"#)
                }
                _ => return Err(format!("Unknown action: {action}")),
            };
            let result = run_osascript(&script)?;
            Ok(if result.is_empty() { "Done".into() } else { result })
        },
    )
}

// ─── Notes ───────────────────────────────────────────────────────────────────

fn create_note_tool() -> ToolDefinition {
    ToolDefinition::new(
        "create_note",
        "Create a new note in the macOS Notes app.",
        Schema::new("CreateNoteArgs")
            .property(SchemaProperty::new("title", SchemaPropertyType::String).description("Note title"))
            .property(SchemaProperty::new("body", SchemaPropertyType::String).description("Note content")),
        |args| {
            let title = escape_applescript(args["title"].as_str().unwrap_or("New Note"));
            let body  = escape_applescript(args["body"].as_str().unwrap_or(""));
            require_permission(&format!("Create note: {title}"))?;
            let script = format!(r#"tell application "Notes"
    tell account "iCloud"
        make new note with properties {{name:"{title}", body:"{body}"}}
    end tell
end tell"#);
            run_osascript(&script)?;
            Ok(format!("Created note: {title}"))
        },
    )
}

// ─── Wikipedia ───────────────────────────────────────────────────────────────

fn search_wikipedia_tool() -> ToolDefinition {
    ToolDefinition::new(
        "search_wikipedia",
        "Search Wikipedia for factual information about a person, place, concept, or event. \
         Use this when the user asks a factual or encyclopedic question where a Wikipedia \
         summary would be relevant.",
        Schema::new("SearchWikipediaArgs").property(
            SchemaProperty::new("topic", SchemaPropertyType::String)
                .description("The subject to look up (e.g. 'Eiffel Tower', 'Python programming language')"),
        ),
        |args| {
            let topic = args["topic"].as_str().unwrap_or("").replace(' ', "_");
            let url = format!("https://en.wikipedia.org/api/rest_v1/page/summary/{topic}");
            let out = std::process::Command::new("curl")
                .args(["-s", "-L", "--max-time", "5", "-H", "Accept: application/json", &url])
                .output()
                .map_err(|e| e.to_string())?;
            let json: serde_json::Value =
                serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;
            if json["type"].as_str() == Some("https://mediawiki.org/wiki/HyperSwitch/errors/not_found") {
                return Err(format!("No Wikipedia article found for '{topic}'"));
            }
            let title   = json["title"].as_str().unwrap_or(&topic);
            let extract = json["extract"].as_str().unwrap_or("No summary available.");
            // Return the full extract — the AI will synthesise its own summary
            Ok(format!("[Wikipedia: {title}]\n{extract}"))
        },
    )
}

// ─── System Resources ─────────────────────────────────────────────────────────

fn get_system_resources_tool() -> ToolDefinition {
    ToolDefinition::new(
        "get_system_resources",
        "Get current system resource usage: battery percentage and charging state, CPU load, \
         and memory pressure. Use when the user asks about battery life, CPU usage, or how \
         much memory is being used.",
        Schema::new("GetSystemResourcesArgs"),
        |_args| {
            // Battery via pmset
            let battery = std::process::Command::new("pmset")
                .args(["-g", "batt"])
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
                .unwrap_or_default();

            // CPU load average (1-min) via sysctl
            let load = std::process::Command::new("sysctl")
                .args(["-n", "vm.loadavg"])
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_default();

            // Memory pressure via memory_pressure tool
            let mem = std::process::Command::new("sh")
                .args(["-c", "vm_stat | awk '/Pages free/{f=$3} /Pages active/{a=$3} /Pages wired/{w=$4} END{total=f+a+w; printf \"free=%d active=%d wired=%d pages\\n\",f,a,w}'"])
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_default();

            Ok(format!("Battery:\n{battery}\nCPU load avg (1m 5m 15m): {load}\nMemory pages: {mem}"))
        },
    )
}

// ─── Network Info ─────────────────────────────────────────────────────────────

fn get_network_info_tool() -> ToolDefinition {
    ToolDefinition::new(
        "get_network_info",
        "Get current network information: WiFi SSID and local IP address. \
         Use when the user asks what network they're on or their IP address.",
        Schema::new("GetNetworkInfoArgs"),
        |_args| {
            let ssid = std::process::Command::new("networksetup")
                .args(["-getairportnetwork", "en0"])
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_else(|_| "WiFi info unavailable".into());

            let ip = std::process::Command::new("sh")
                .args(["-c", "ipconfig getifaddr en0 2>/dev/null || ipconfig getifaddr en1 2>/dev/null || echo 'No IP'"])
                .output()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_else(|_| "IP unavailable".into());

            Ok(format!("{ssid}\nLocal IP: {ip}"))
        },
    )
}

// ─── Clipboard ────────────────────────────────────────────────────────────────

fn get_clipboard_tool() -> ToolDefinition {
    ToolDefinition::new(
        "get_clipboard_text",
        "Read the current text contents of the system clipboard. \
         Use when the user asks what's on their clipboard or to use clipboard contents.",
        Schema::new("GetClipboardArgs"),
        |_args| {
            let out = std::process::Command::new("pbpaste")
                .output()
                .map_err(|e| e.to_string())?;
            let text = String::from_utf8_lossy(&out.stdout).to_string();
            if text.is_empty() {
                Ok("Clipboard is empty".into())
            } else {
                Ok(text)
            }
        },
    )
}

fn set_clipboard_tool() -> ToolDefinition {
    ToolDefinition::new(
        "set_clipboard_text",
        "Write text to the system clipboard. Use when the user asks to copy something to the clipboard.",
        Schema::new("SetClipboardArgs")
            .property(SchemaProperty::new("text", SchemaPropertyType::String).description("Text to put on the clipboard")),
        |args| {
            let text = args["text"].as_str().unwrap_or("").to_string();
            let preview = text.chars().take(50).collect::<String>();
            require_permission(&format!("Write to clipboard: {preview}"))?;
            use std::io::Write;
            let mut child = std::process::Command::new("pbcopy")
                .stdin(std::process::Stdio::piped())
                .spawn()
                .map_err(|e| e.to_string())?;
            if let Some(stdin) = child.stdin.as_mut() {
                stdin.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
            }
            child.wait().map_err(|e| e.to_string())?;
            Ok("Copied to clipboard".into())
        },
    )
}

// ─── Notifications ────────────────────────────────────────────────────────────

fn send_notification_tool() -> ToolDefinition {
    ToolDefinition::new(
        "send_notification",
        "Show a macOS system notification banner. Use when the user asks to be notified, \
         set an alert, or remind themselves of something.",
        Schema::new("SendNotificationArgs")
            .property(SchemaProperty::new("title", SchemaPropertyType::String).description("Notification title"))
            .property(SchemaProperty::new("message", SchemaPropertyType::String).description("Notification body text"))
            .property(SchemaProperty::new("subtitle", SchemaPropertyType::String).description("Optional subtitle line").optional()),
        |args| {
            let title    = escape_applescript(args["title"].as_str().unwrap_or("Rover"));
            let message  = escape_applescript(args["message"].as_str().unwrap_or(""));
            let subtitle = args["subtitle"].as_str().unwrap_or("");
            let sub_clause = if subtitle.is_empty() {
                String::new()
            } else {
                format!(" subtitle \"{}\"", escape_applescript(subtitle))
            };
            let script = format!(
                r#"display notification "{message}"{sub_clause} with title "{title}""#
            );
            run_osascript(&script)?;
            Ok("Notification sent".into())
        },
    )
}

// ─── Volume ───────────────────────────────────────────────────────────────────

fn get_volume_tool() -> ToolDefinition {
    ToolDefinition::new(
        "get_volume",
        "Get the current system output volume level (0–100) and whether it's muted.",
        Schema::new("GetVolumeArgs"),
        |_args| {
            run_osascript("output volume of (get volume settings) & \", muted: \" & (output muted of (get volume settings))")
        },
    )
}

fn set_volume_tool() -> ToolDefinition {
    ToolDefinition::new(
        "set_volume",
        "Set the system output volume. Use when the user asks to turn the volume up, down, \
         mute, or set it to a specific level.",
        Schema::new("SetVolumeArgs")
            .property(SchemaProperty::new("level", SchemaPropertyType::Double).description("Volume level 0–100, or -1 to mute").optional())
            .property(SchemaProperty::new("mute", SchemaPropertyType::Bool).description("true to mute, false to unmute").optional()),
        |args| {
            if let Some(true) = args["mute"].as_bool() {
                run_osascript("set volume with output muted")?;
                return Ok("Muted".into());
            }
            if let Some(false) = args["mute"].as_bool() {
                run_osascript("set volume without output muted")?;
                return Ok("Unmuted".into());
            }
            if let Some(level) = args["level"].as_f64() {
                let level = level.clamp(0.0, 100.0) as u32;
                run_osascript(&format!("set volume output volume {level}"))?;
                return Ok(format!("Volume set to {level}"));
            }
            Err("Provide 'level' (0-100) or 'mute' (true/false)".into())
        },
    )
}

// ─── Screenshot ───────────────────────────────────────────────────────────────

fn take_screenshot_tool() -> ToolDefinition {
    ToolDefinition::new(
        "take_screenshot",
        "Take a screenshot of the screen or a specific window and save it to ~/Desktop. \
         Returns the file path. Use when the user asks to capture their screen.",
        Schema::new("TakeScreenshotArgs")
            .property(SchemaProperty::new("filename", SchemaPropertyType::String).description("Output filename without extension (default: screenshot)").optional())
            .property(SchemaProperty::new("interactive", SchemaPropertyType::Bool).description("true to let the user select a region (default: false)").optional()),
        |args| {
            let name = args["filename"].as_str().unwrap_or("screenshot");
            // Sanitize: only allow alphanumeric, dash, underscore
            let safe_name: String = name.chars()
                .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
                .collect();
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
            let path = format!("{home}/Desktop/{safe_name}.png");

            require_permission(&format!("Take a screenshot and save to {path}"))?;
            let interactive = args["interactive"].as_bool().unwrap_or(false);
            let mut cmd = std::process::Command::new("screencapture");
            cmd.arg("-x"); // no camera click
            if interactive {
                cmd.arg("-i"); // interactive region selection
            }
            cmd.arg(&path);
            let out = cmd.output().map_err(|e| e.to_string())?;
            if out.status.success() {
                Ok(format!("Screenshot saved to {path}"))
            } else {
                Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
            }
        },
    )
}

// ─── Contacts ─────────────────────────────────────────────────────────────────

fn lookup_contact_tool() -> ToolDefinition {
    ToolDefinition::new(
        "lookup_contact",
        "Search for a contact in macOS Contacts.app by name. Returns name, phone numbers, \
         and email addresses. Use when the user asks for someone's contact info.",
        Schema::new("LookupContactArgs")
            .property(SchemaProperty::new("name", SchemaPropertyType::String).description("Full or partial name to search for")),
        |args| {
            let name = escape_applescript(args["name"].as_str().unwrap_or(""));
            let script = format!(
                r#"tell application "Contacts"
    set results to every person whose name contains "{name}"
    if results is {{}} then return "No contacts found for '{name}'"
    set output to ""
    repeat with p in results
        set output to output & (name of p)
        try
            set phones to phones of p
            repeat with ph in phones
                set output to output & linefeed & "  Phone: " & (value of ph)
            end repeat
        end try
        try
            set emails to emails of p
            repeat with em in emails
                set output to output & linefeed & "  Email: " & (value of em)
            end repeat
        end try
        set output to output & linefeed
    end repeat
    return output
end tell"#
            );
            run_osascript(&script)
        },
    )
}

// ─── iMessage ─────────────────────────────────────────────────────────────────

fn send_imessage_tool() -> ToolDefinition {
    ToolDefinition::new(
        "send_imessage",
        "Send an iMessage or SMS via Messages.app. Use when the user asks to send a message \
         to someone. Requires Messages.app to be signed in.",
        Schema::new("SendIMessageArgs")
            .property(SchemaProperty::new("recipient", SchemaPropertyType::String).description("Phone number (e.g. '+447700900000') or Apple ID email"))
            .property(SchemaProperty::new("message", SchemaPropertyType::String).description("Message text to send")),
        |args| {
            let recipient = escape_applescript(args["recipient"].as_str().unwrap_or(""));
            let message   = escape_applescript(args["message"].as_str().unwrap_or(""));
            require_permission(&format!("Send iMessage to {recipient}"))?;
            let script = format!(
                r#"tell application "Messages"
    set targetService to 1st service whose service type = iMessage
    set targetBuddy to buddy "{recipient}" of targetService
    send "{message}" to targetBuddy
end tell"#
            );
            run_osascript(&script)?;
            Ok(format!("Message sent to {recipient}"))
        },
    )
}

// ─── Email ────────────────────────────────────────────────────────────────────

fn send_email_tool() -> ToolDefinition {
    ToolDefinition::new(
        "send_email",
        "Compose and send an email via macOS Mail.app. Use when the user asks to send an email.",
        Schema::new("SendEmailArgs")
            .property(SchemaProperty::new("to", SchemaPropertyType::String).description("Recipient email address"))
            .property(SchemaProperty::new("subject", SchemaPropertyType::String).description("Email subject"))
            .property(SchemaProperty::new("body", SchemaPropertyType::String).description("Email body text"))
            .property(SchemaProperty::new("cc", SchemaPropertyType::String).description("CC email address").optional()),
        |args| {
            let to      = escape_applescript(args["to"].as_str().unwrap_or(""));
            let subject = escape_applescript(args["subject"].as_str().unwrap_or(""));
            let body    = escape_applescript(args["body"].as_str().unwrap_or(""));
            let cc      = args["cc"].as_str().unwrap_or("");
            require_permission(&format!("Send email to {to}: {subject}"))?;

            let cc_clause = if cc.is_empty() {
                String::new()
            } else {
                let cc_safe = escape_applescript(cc);
                format!("make new to recipient at theMessage with properties {{address:\"{cc_safe}\", recipient type:cc recipient}}\n        ")
            };

            let script = format!(
                r#"tell application "Mail"
    set theMessage to make new outgoing message with properties {{subject:"{subject}", content:"{body}", visible:false}}
    tell theMessage
        make new to recipient with properties {{address:"{to}"}}
        {cc_clause}
    end tell
    send theMessage
end tell"#
            );
            run_osascript(&script)?;
            Ok(format!("Email sent to {to}"))
        },
    )
}

// ─── Spotlight Search ─────────────────────────────────────────────────────────

fn spotlight_search_tool() -> ToolDefinition {
    ToolDefinition::new(
        "spotlight_search",
        "Search for files and documents on this Mac using Spotlight (mdfind). \
         Use when the user wants to find a file, document, or any item by name or content.",
        Schema::new("SpotlightSearchArgs")
            .property(SchemaProperty::new("query", SchemaPropertyType::String).description("Search query (filename, content, or metadata query)"))
            .property(SchemaProperty::new("max_results", SchemaPropertyType::Double).description("Maximum number of results (default: 20)").optional()),
        |args| {
            let query = args["query"].as_str().unwrap_or("").to_string();
            let max = args["max_results"].as_u64().unwrap_or(20).min(50);

            // Run mdfind with the user's query
            let out = std::process::Command::new("mdfind")
                .arg("-name")
                .arg(&query)
                .output()
                .map_err(|e| e.to_string())?;

            let results: Vec<&str> = std::str::from_utf8(&out.stdout)
                .unwrap_or("")
                .lines()
                .filter(|l| !l.is_empty())
                .take(max as usize)
                .collect();

            if results.is_empty() {
                Ok(format!("No files found matching '{query}'"))
            } else {
                Ok(results.join("\n"))
            }
        },
    )
}

// ─── Bluetooth ────────────────────────────────────────────────────────────────

fn list_bluetooth_devices_tool() -> ToolDefinition {
    ToolDefinition::new(
        "list_bluetooth_devices",
        "List paired and connected Bluetooth devices on this Mac. \
         Use when the user asks what Bluetooth devices are connected or available.",
        Schema::new("ListBluetoothDevicesArgs"),
        |_args| {
            let out = std::process::Command::new("system_profiler")
                .args(["SPBluetoothDataType", "-json"])
                .output()
                .map_err(|e| e.to_string())?;

            let json: serde_json::Value =
                serde_json::from_slice(&out.stdout).map_err(|e| format!("Parse error: {e}"))?;

            let mut lines: Vec<String> = Vec::new();
            if let Some(items) = json["SPBluetoothDataType"].as_array() {
                for item in items {
                    if let Some(devices) = item["device_connected"].as_array() {
                        for d in devices {
                            if let Some(obj) = d.as_object() {
                                for (name, info) in obj {
                                    let minor_type = info["device_minorType"].as_str().unwrap_or("");
                                    lines.push(format!("✓ {name} [{minor_type}] (connected)"));
                                }
                            }
                        }
                    }
                    if let Some(devices) = item["device_not_connected"].as_array() {
                        for d in devices {
                            if let Some(obj) = d.as_object() {
                                for (name, info) in obj {
                                    let minor_type = info["device_minorType"].as_str().unwrap_or("");
                                    lines.push(format!("○ {name} [{minor_type}]"));
                                }
                            }
                        }
                    }
                }
            }

            if lines.is_empty() {
                Ok("No Bluetooth devices found".into())
            } else {
                Ok(lines.join("\n"))
            }
        },
    )
}

// ─── Window management ────────────────────────────────────────────────────────

fn close_window_tool() -> ToolDefinition {
    ToolDefinition::new(
        "close_window",
        "Close the frontmost window of the currently focused application. \
         Use when the user asks to close, quit, or dismiss the current window.",
        Schema::new("CloseWindowArgs"),
        |_args| {
            require_permission("Close frontmost window")?;
            // Send Cmd+W to the frontmost app via System Events.
            // This works universally, even for terminal apps that don't implement
            // the Standard Suite's "close window" command.
            let script = r#"tell application "System Events" to keystroke "w" using command down"#;
            match run_osascript(script) {
                Ok(_) => Ok("Closed frontmost window".to_string()),
                Err(e) => Err(e),
            }
        },
    )
}

fn quit_app_tool() -> ToolDefinition {
    ToolDefinition::new(
        "quit_app",
        "Quit (close) a running macOS application by name. \
         Use when the user asks to quit, exit, or kill an application.",
        Schema::new("QuitAppArgs")
            .property(SchemaProperty::new("app_name", SchemaPropertyType::String).description("The name of the application to quit (e.g. 'Safari', 'Slack')")),
        |args| {
            let app = args["app_name"].as_str().unwrap_or("");
            if app.is_empty() {
                return Err("app_name is required".to_string());
            }
            let safe = escape_applescript(app);
            require_permission(&format!("Quit {safe}"))?;
            // Quit by process name via System Events — works for all apps including
            // terminals that don't expose an AppleScript dictionary.
            let script = format!(
                r#"tell application "System Events"
    try
        quit (first application process whose name is "{safe}")
    end try
end tell"#
            );
            match run_osascript(&script) {
                Ok(_) => Ok(format!("Quit {safe}")),
                Err(e) => Err(e),
            }
        },
    )
}

// ─── Browser Tab Management ───────────────────────────────────────────────────

fn close_browser_tab_tool() -> ToolDefinition {
    ToolDefinition::new(
        "close_browser_tab",
        "Close the active tab in the Rover browser. Use when the user asks to close the current \
         browser tab, close a tab, or close the current page. \
         This works by signalling the Rover browser to close its active tab.",
        Schema::new("CloseBrowserTabArgs"),
        |_args| {
            // Signal Rover's browser to close the active tab via a flag file.
            // The browser polls for this and closes the tab internally.
            let flag_path = dirs::config_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
                .join("rover")
                .join(".close_tab_flag");
            if let Some(parent) = flag_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&flag_path, "close");
            Ok("Closing browser tab".to_string())
        },
    )
}

// ─── Inline Command Execution ─────────────────────────────────────────────────

fn run_inline_command_tool() -> ToolDefinition {
    ToolDefinition::new(
        "run_inline_command",
        "Run a shell command and return its output inline. Use when the user asks to run a command \
         and see the output without opening a terminal window.",
        Schema::new("RunInlineCommandArgs")
            .property(SchemaProperty::new("command", SchemaPropertyType::String).description("Shell command to execute")),
        |args| {
            let command = args["command"].as_str().unwrap_or("").to_string();
            require_permission(&format!("Run inline command: {command}"))?;
            run_shell_command(&command)
        },
    )
}

// ─── Keychain ─────────────────────────────────────────────────────────────────

fn get_keychain_secret_tool() -> ToolDefinition {
    ToolDefinition::new(
        "get_keychain_secret",
        "Retrieve a password or secret from the macOS Keychain by service name and account. \
         Use when the user asks to fetch a stored password or credential. \
         The user will be prompted by macOS to allow access.",
        Schema::new("GetKeychainSecretArgs")
            .property(SchemaProperty::new("service", SchemaPropertyType::String).description("Service name (e.g. 'github.com', 'MyApp')"))
            .property(SchemaProperty::new("account", SchemaPropertyType::String).description("Account name or username").optional()),
        |args| {
            let service = args["service"].as_str().unwrap_or("").to_string();
            let account = args["account"].as_str().unwrap_or("").to_string();
            require_permission(&format!("Access Keychain for service: {service}"))?;

            // Sanitize: only allow safe chars in service/account to prevent shell injection
            let safe_service: String = service.chars()
                .filter(|c| c.is_alphanumeric() || matches!(*c, '.' | '-' | '_' | '@' | ':' | '/'))
                .collect();
            let safe_account: String = account.chars()
                .filter(|c| c.is_alphanumeric() || matches!(*c, '.' | '-' | '_' | '@'))
                .collect();

            let mut cmd = std::process::Command::new("security");
            cmd.args(["find-generic-password", "-s", &safe_service, "-w"]);
            if !safe_account.is_empty() {
                cmd.args(["-a", &safe_account]);
            }
            let out = cmd.output().map_err(|e| e.to_string())?;
            if out.status.success() {
                let secret = String::from_utf8_lossy(&out.stdout).trim().to_string();
                Ok(secret)
            } else {
                // Try internet password
                let mut cmd2 = std::process::Command::new("security");
                cmd2.args(["find-internet-password", "-s", &safe_service, "-w"]);
                if !safe_account.is_empty() {
                    cmd2.args(["-a", &safe_account]);
                }
                let out2 = cmd2.output().map_err(|e| e.to_string())?;
                if out2.status.success() {
                    Ok(String::from_utf8_lossy(&out2.stdout).trim().to_string())
                } else {
                    Err(format!("No keychain entry found for service '{safe_service}'"))
                }
            }
        },
    )
}
