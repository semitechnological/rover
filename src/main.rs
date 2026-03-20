mod ai;
mod assets;
mod clipboard_monitor;
mod delegate;
mod hotkey;
mod settings_window;
mod tools;

use delegate::RoverDelegate;
use gpui::*;
use rover_commands::{register_apps, register_browser, register_builtins, CommandRegistry};
use rover_db::RoverDb;
use rover_extensions::{load_extensions, register_extension_themes};
use rover_picker::{bind_picker_keys, Picker, PickerEvent};
use rover_theme::{self, ThemeRegistry};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

actions!(rover, [Toggle, Quit, OpenSettings, ShowAbout]);

struct DbGlobal(pub Arc<RoverDb>);
impl Global for DbGlobal {}

struct CommandsGlobal(Vec<rover_commands::Command>);
impl Global for CommandsGlobal {}

struct WindowOpenGlobal(Arc<AtomicBool>);
impl Global for WindowOpenGlobal {}

// ─── CLI ──────────────────────────────────────────────────────────────────────

#[derive(Debug)]
enum CliCommand {
    /// No CLI args — normal GUI launch.
    None,
    /// Open the browser at a specific URL.
    OpenUrl(String),
    /// Search the web (opens browser).
    Search(String),
    /// Open the browser at the default URL.
    Browser,
}

/// Parse argv into either a headless action (run it and return) or a
/// `CliCommand` describing what the GUI should open on startup.
fn parse_args() -> CliCommand {
    let args: Vec<String> = std::env::args().skip(1).collect();

    match args.first().map(|s| s.as_str()) {
        None | Some("") => CliCommand::None,

        // ── Headless commands (no GUI) ─────────────────────────────────────
        Some("help") | Some("--help") | Some("-h") => {
            print!(
                "Rover — macOS launcher\n\
                 \n\
                 USAGE\n\
                 \n\
                 rover                        Open the launcher (GUI)\n\
                 rover open <url>             Open URL in the Rover browser\n\
                 rover search <query...>      Search the web in Rover browser\n\
                 rover browser                Open the Rover browser\n\
                 rover launch <app>           Activate a macOS app\n\
                 rover run <cmd...>           Run a shell command\n\
                 rover screenshot [name]      Take a screenshot to ~/Desktop\n\
                 rover help                   Show this help\n\
                 rover version                Print version\n\
                 \n"
            );
            std::process::exit(0);
        }

        Some("version") | Some("--version") | Some("-V") => {
            println!("rover {}", env!("CARGO_PKG_VERSION"));
            std::process::exit(0);
        }

        Some("launch") => {
            let app = args[1..].join(" ");
            if app.is_empty() {
                eprintln!("Usage: rover launch <App Name>");
                std::process::exit(1);
            }
            // Sanitize: strip control chars & double-quotes to prevent injection.
            let safe: String = app.chars().filter(|c| !c.is_control() && *c != '"').collect();
            let script = format!(r#"tell application "{safe}" to activate"#);
            let status = std::process::Command::new("osascript")
                .arg("-e")
                .arg(&script)
                .status()
                .unwrap_or_else(|e| { eprintln!("osascript error: {e}"); std::process::exit(1) });
            std::process::exit(if status.success() { 0 } else { 1 });
        }

        Some("run") => {
            let cmd = args[1..].join(" ");
            if cmd.is_empty() {
                eprintln!("Usage: rover run <shell command>");
                std::process::exit(1);
            }
            let output = std::process::Command::new("sh")
                .arg("-c")
                .arg(&cmd)
                .output()
                .unwrap_or_else(|e| { eprintln!("spawn error: {e}"); std::process::exit(1) });
            print!("{}", String::from_utf8_lossy(&output.stdout));
            eprint!("{}", String::from_utf8_lossy(&output.stderr));
            std::process::exit(output.status.code().unwrap_or(1));
        }

        Some("screenshot") => {
            let name = args.get(1).map(|s| s.as_str()).unwrap_or("screenshot");
            let safe: String = name.chars()
                .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
                .collect();
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
            let path = format!("{home}/Desktop/{safe}.png");
            let status = std::process::Command::new("screencapture")
                .args(["-x", &path])
                .status()
                .unwrap_or_else(|e| { eprintln!("screencapture error: {e}"); std::process::exit(1) });
            if status.success() {
                println!("{path}");
                std::process::exit(0);
            } else {
                eprintln!("screencapture failed");
                std::process::exit(1);
            }
        }

        // ── GUI commands (start the app, skip the picker) ──────────────────
        Some("open") => {
            let raw = args[1..].join(" ");
            if raw.is_empty() {
                eprintln!("Usage: rover open <url>");
                std::process::exit(1);
            }
            let url = if raw.starts_with("http://") || raw.starts_with("https://") {
                raw
            } else {
                format!("https://{raw}")
            };
            CliCommand::OpenUrl(url)
        }

        Some("search") | Some("s") => {
            let query = args[1..].join(" ");
            if query.is_empty() {
                eprintln!("Usage: rover search <query>");
                std::process::exit(1);
            }
            CliCommand::Search(query)
        }

        Some("browser") | Some("b") => CliCommand::Browser,

        Some(unknown) => {
            eprintln!("Unknown command: {unknown}\nRun `rover help` for usage.");
            std::process::exit(1);
        }
    }
}

fn main() {
    let cli = parse_args();

    Application::new().run(|cx| {
        rover_theme::init(cx);

        if let Ok(extensions) = load_extensions() {
            let registry = cx.global_mut::<ThemeRegistry>();
            register_extension_themes(&extensions, registry);
        }

        let mut cmd_registry = CommandRegistry::new();
        register_builtins(&mut cmd_registry);
        register_browser(&mut cmd_registry);
        register_apps(&mut cmd_registry);
        let commands: Vec<_> = cmd_registry.commands().to_vec();

        let db = Arc::new(RoverDb::open().expect("Failed to open database"));

        cx.set_global(DbGlobal(db.clone()));
        cx.set_global(CommandsGlobal(commands.clone()));
        cx.set_global(WindowOpenGlobal(Arc::new(AtomicBool::new(false))));

        clipboard_monitor::start(db.clone(), cx);

        if db
            .get_setting("hyperkey_enabled")
            .ok()
            .flatten()
            .map(|v| v == "true")
            .unwrap_or(false)
        {
            let _ = hotkey::enable_hyperkey_remap();
        }

        bind_picker_keys(cx);
        cx.bind_keys([
            KeyBinding::new("cmd-q", Quit, None),
            KeyBinding::new("cmd-,", OpenSettings, None),
        ]);

        cx.set_menus(vec![Menu {
            name: "Rover".into(),
            items: vec![
                MenuItem::action("Open Rover", Toggle),
                MenuItem::separator(),
                MenuItem::action("About Rover", ShowAbout),
                MenuItem::action("Settings...", OpenSettings),
                MenuItem::separator(),
                MenuItem::action("Quit", Quit),
            ],
        }]);

        cx.on_action::<Quit>(|_, cx| cx.quit());

        cx.on_action::<Toggle>(|_, cx| {
            let window_open = cx.global::<WindowOpenGlobal>().0.load(Ordering::Relaxed);
            if !window_open {
                let commands = cx.global::<CommandsGlobal>().0.clone();
                let db = cx.global::<DbGlobal>().0.clone();
                open_window(commands, db, cx);
            }
        });

        cx.on_action::<OpenSettings>(|_, cx| {
            let db = cx.global::<DbGlobal>().0.clone();
            settings_window::open_settings_window(db, cx);
        });

        cx.on_action::<ShowAbout>(|_, cx| {
            let db = cx.global::<DbGlobal>().0.clone();
            settings_window::open_settings_window_on_about(db, cx);
        });

        // Load hotkey: prefer custom recorded combo, fall back to preset
        let (hk_code, hk_mods) = {
            let key_name = db.get_setting("hotkey_key_name").ok().flatten();
            let mods = db
                .get_setting("hotkey_modifiers")
                .ok()
                .flatten()
                .and_then(|v| v.parse::<u32>().ok());
            match (key_name.as_deref(), mods) {
                (Some(k), Some(m)) => {
                    hotkey::key_name_to_vk(k).map(|vk| (vk, m)).unwrap_or_else(|| {
                        let preset = db
                            .get_setting("hotkey_preset")
                            .ok()
                            .flatten()
                            .unwrap_or_else(|| "cmd-space".to_string());
                        hotkey::preset_to_carbon(&preset)
                    })
                }
                _ => {
                    let preset = db
                        .get_setting("hotkey_preset")
                        .ok()
                        .flatten()
                        .unwrap_or_else(|| "cmd-space".to_string());
                    hotkey::preset_to_carbon(&preset)
                }
            }
        };
        hotkey::register_global_hotkey(hk_code, hk_mods);

        // Poll for hotkey presses from the Carbon event handler and open the window
        cx.spawn(async move |cx: &mut AsyncApp| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(50))
                    .await;

                if hotkey::take_toggle_requested() {
                    let _ = cx.update(|cx: &mut App| {
                        let window_open =
                            cx.global::<WindowOpenGlobal>().0.load(Ordering::Relaxed);
                        if !window_open {
                            let commands = cx.global::<CommandsGlobal>().0.clone();
                            let db = cx.global::<DbGlobal>().0.clone();
                            open_window(commands, db, cx);
                        }
                    });
                }
            }
        })
        .detach();

        match cli {
            CliCommand::None => open_window(commands, db, cx),
            CliCommand::Browser => {
                rover_browser::open_browser_window(cx, Some(db));
            }
            CliCommand::OpenUrl(url) => {
                rover_browser::open_browser_window_with_url(cx, &url, Some(db));
            }
            CliCommand::Search(query) => {
                let encoded = urlencoding::encode(&query);
                let url = format!("https://www.perplexity.ai/search?q={encoded}");
                rover_browser::open_browser_window_with_url(cx, &url, Some(db));
            }
        }
    });
}

fn open_window(commands: Vec<rover_commands::Command>, db: Arc<RoverDb>, cx: &mut App) {
    let window_open_flag = cx.global::<WindowOpenGlobal>().0.clone();
    window_open_flag.store(true, Ordering::Relaxed);

    let bounds = Bounds::centered(None, size(px(680.0), px(420.0)), cx);

    let ai_session = if cx
        .global::<DbGlobal>()
        .0
        .get_setting("ai_enabled")
        .ok()
        .flatten()
        .map(|v| v == "true")
        .unwrap_or(false)
    {
        ai::create_session().map(Arc::new)
    } else {
        None
    };
    let ai_enabled = ai_session.is_some();

    let window = cx
        .open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: None,
                kind: WindowKind::PopUp,
                focus: true,
                is_movable: true,
                is_resizable: false,
                window_background: WindowBackgroundAppearance::Transparent,
                ..Default::default()
            },
            |window, cx| {
                let settings_db = db.clone();
                let dismiss_flag = window_open_flag.clone();
                cx.new(|cx| {
                    let delegate = RoverDelegate::new(commands, db, ai_session, ai_enabled);
                    let picker = cx.new(|cx| Picker::new(delegate, cx));

                    cx.subscribe_in(
                        &picker,
                        window,
                        move |_this: &mut RoverWindow, _picker, event: &PickerEvent, window, cx| {
                            match event {
                                PickerEvent::Dismiss => {
                                    dismiss_flag.store(false, Ordering::Relaxed);
                                    window.remove_window();
                                }
                                PickerEvent::OpenSettings => {
                                    dismiss_flag.store(false, Ordering::Relaxed);
                                    let db = settings_db.clone();
                                    let handle = window.window_handle();
                                    cx.spawn(async move |_entity, cx: &mut AsyncApp| {
                                        let _ = cx.update(|cx| {
                                            settings_window::open_settings_window(db, cx);
                                        });
                                        let _ = handle.update(cx, |_, window, _cx| {
                                            window.remove_window();
                                        });
                                    })
                                    .detach();
                                }
                                PickerEvent::ToggleActions => {}
                                PickerEvent::OpenTerminal => {
                                    dismiss_flag.store(false, Ordering::Relaxed);
                                    window.remove_window();
                                    let _ = std::process::Command::new("open")
                                        .args(["-a", "Terminal"])
                                        .spawn();
                                }
                            }
                        },
                    )
                    .detach();

                    RoverWindow { picker }
                })
            },
        )
        .unwrap();

    window
        .update(cx, |view, window, cx| {
            let input_focus = view.picker.read(cx).text_input.focus_handle(cx);
            window.focus(&input_focus);
            cx.activate(true);
        })
        .unwrap();
}

struct RoverWindow {
    picker: Entity<Picker<RoverDelegate>>,
}

impl Render for RoverWindow {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.picker.clone())
    }
}
