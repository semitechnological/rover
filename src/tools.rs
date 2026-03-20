use rusty_foundationmodels::{Schema, SchemaProperty, SchemaPropertyType, ToolDefinition};

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
                other => format!("set d to date \"{other}\"\nset hours of d to 0\nset minutes of d to 0\nset seconds of d to 0"),
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
            let out = std::process::Command::new("osascript").arg("-e").arg(&script).output()
                .map_err(|e| e.to_string())?;
            if out.status.success() {
                Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
            } else {
                Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
            }
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
            let list_name = args["list_name"].as_str().unwrap_or("all");
            let filter = if list_name == "all" { "every list".to_string() } else { format!("list \"{list_name}\"") };
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
            let out = std::process::Command::new("osascript").arg("-e").arg(&script).output()
                .map_err(|e| e.to_string())?;
            if out.status.success() {
                Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
            } else {
                Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
            }
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
            let name = args["name"].as_str().unwrap_or("").replace('"', "'");
            let list = args["list_name"].as_str().unwrap_or("Reminders").replace('"', "'");
            let script = format!(
                r#"tell application "Reminders"
    tell list "{list}"
        make new reminder with properties {{name:"{name}"}}
    end tell
end tell"#
            );
            let out = std::process::Command::new("osascript").arg("-e").arg(&script).output()
                .map_err(|e| e.to_string())?;
            if out.status.success() {
                Ok(format!("Added reminder: {name}"))
            } else {
                Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
            }
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
                    let name = args["track_name"].as_str().unwrap_or("").replace('"', "'");
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
            let out = std::process::Command::new("osascript").arg("-e").arg(&script).output()
                .map_err(|e| e.to_string())?;
            if out.status.success() {
                let r = String::from_utf8_lossy(&out.stdout).trim().to_string();
                Ok(if r.is_empty() { "Done".into() } else { r })
            } else {
                Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
            }
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
            let title = args["title"].as_str().unwrap_or("New Note").replace('"', "'");
            let body  = args["body"].as_str().unwrap_or("").replace('"', "'");
            let script = format!(r#"tell application "Notes"
    tell account "iCloud"
        make new note with properties {{name:"{title}", body:"{body}"}}
    end tell
end tell"#);
            let out = std::process::Command::new("osascript").arg("-e").arg(&script).output()
                .map_err(|e| e.to_string())?;
            if out.status.success() {
                Ok(format!("Created note: {title}"))
            } else {
                Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
            }
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
            let extract = if extract.len() > 600 { format!("{}…", &extract[..600]) } else { extract.to_string() };
            Ok(format!("{title}: {extract}"))
        },
    )
}
