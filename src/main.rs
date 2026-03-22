mod ai;
mod clipboard_monitor;
mod delegate;
mod help_docs;
mod hotkey;
mod settings_window;
mod terminal;
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

actions!(rover, [Toggle, Quit, OpenSettings, ShowAbout, BrowserNewTab, BrowserCloseTab, BrowserReload, BrowserBack, BrowserForward]);

/// Static holding the semantic index, set from a background thread on startup.
static SEMANTIC_INDEX: std::sync::Mutex<Option<Arc<std::sync::Mutex<rover_semantic::SemanticIndex>>>> =
    std::sync::Mutex::new(None);

struct DbGlobal(pub Arc<RoverDb>);
impl Global for DbGlobal {}

struct SemanticGlobal(pub Arc<std::sync::Mutex<rover_semantic::SemanticIndex>>);
impl Global for SemanticGlobal {}

struct CommandsGlobal(Vec<rover_commands::Command>);
impl Global for CommandsGlobal {}

struct ExtensionRuntimesGlobal(
    std::collections::HashMap<String, rover_extensions::runtime::ExtensionRuntimeRef>,
);
impl Global for ExtensionRuntimesGlobal {}

struct WindowOpenGlobal(Arc<AtomicBool>);
impl Global for WindowOpenGlobal {}

struct WindowHandleGlobal(Option<WindowHandle<RoverWindow>>);
impl Global for WindowHandleGlobal {}

/// Continuously tracked last non-Rover frontmost app name.
/// Updated on a background poll so it's always ready when the hotkey fires.
struct LastFrontmostAppGlobal(Arc<std::sync::Mutex<Option<String>>>);
impl Global for LastFrontmostAppGlobal {}

/// Shared live browser tabs — wrapped in a Global for GPUI access.
struct LiveBrowserTabsGlobal(rover_browser::LiveBrowserTabsRef);
impl Global for LiveBrowserTabsGlobal {}

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

        let loaded_extensions = load_extensions().unwrap_or_default();
        {
            let registry = cx.global_mut::<ThemeRegistry>();
            register_extension_themes(&loaded_extensions, registry);
        }

        let mut cmd_registry = CommandRegistry::new();
        register_builtins(&mut cmd_registry);
        register_browser(&mut cmd_registry);
        register_apps(&mut cmd_registry);

        // Load WASM extension runtimes and register their commands
        let ext_runtimes = rover_extensions::runtime::load_extension_runtimes(&loaded_extensions);
        for (ext_id, _runtime, commands) in &ext_runtimes {
            for cmd_info in commands {
                cmd_registry.register(rover_commands::Command {
                    name: cmd_info.name.clone(),
                    description: cmd_info.description.clone(),
                    icon: None,
                    kind: rover_commands::CommandKind::Extension {
                        ext_id: ext_id.clone(),
                        command: cmd_info.name.clone(),
                    },
                    has_submenu: false,
                });
            }
        }

        let commands: Vec<_> = cmd_registry.commands().to_vec();

        // Store extension runtimes as a global for command execution
        let runtime_map: std::collections::HashMap<String, rover_extensions::runtime::ExtensionRuntimeRef> =
            ext_runtimes.into_iter().map(|(id, rt, _)| (id, rt)).collect();
        cx.set_global(ExtensionRuntimesGlobal(runtime_map));

        let db = Arc::new(RoverDb::open().expect("Failed to open database"));

        // Apply persisted yolo mode before any AI tools can be called
        let yolo = db
            .get_setting("yolo_mode")
            .ok()
            .flatten()
            .map(|v| v == "true")
            .unwrap_or(false);
        tools::set_yolo_mode(yolo);

        cx.set_global(DbGlobal(db.clone()));
        cx.set_global(CommandsGlobal(commands.clone()));
        cx.set_global(WindowOpenGlobal(Arc::new(AtomicBool::new(false))));
        cx.set_global(WindowHandleGlobal(None));
        let last_frontmost = Arc::new(std::sync::Mutex::new(
            delegate::RoverDelegate::detect_frontmost_app(),
        ));
        cx.set_global(LastFrontmostAppGlobal(last_frontmost.clone()));
        let live_browser_tabs: rover_browser::LiveBrowserTabsRef =
            std::sync::Arc::new(std::sync::Mutex::new(rover_browser::LiveBrowserTabs::default()));
        cx.set_global(LiveBrowserTabsGlobal(live_browser_tabs.clone()));

        // Initialize semantic index on a background thread (model download may be slow)
        std::thread::spawn(move || {
            let db_path = dirs::config_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
                .join("rover")
                .join("semantic.db");
            if let Some(parent) = db_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            match rover_semantic::SemanticIndex::new(&db_path) {
                Ok(mut index) => {
                    help_docs::index_help_docs(&mut index);
                    let global = SemanticGlobal(Arc::new(std::sync::Mutex::new(index)));
                    // We can't set_global from a non-main thread, so we stash it in a static.
                    *SEMANTIC_INDEX.lock().unwrap() = Some(global.0);
                    log::info!("Semantic index ready");
                }
                Err(e) => {
                    log::warn!("Semantic index init failed: {e}");
                }
            }
        });

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
            KeyBinding::new("cmd-t", BrowserNewTab, Some("Browser")),
            KeyBinding::new("cmd-w", BrowserCloseTab, Some("Browser")),
            KeyBinding::new("cmd-r", BrowserReload, Some("Browser")),
            KeyBinding::new("cmd-[", BrowserBack, Some("Browser")),
            KeyBinding::new("cmd-]", BrowserForward, Some("Browser")),
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
            if window_open {
                // Dismiss the existing window
                cx.global::<WindowOpenGlobal>().0.store(false, Ordering::Relaxed);
                if let Some(handle) = cx.global::<WindowHandleGlobal>().0 {
                    let _ = handle.update(cx, |_, window, _cx| {
                        window.remove_window();
                    });
                }
                cx.global_mut::<WindowHandleGlobal>().0 = None;
            } else {
                let prev_app = cx.global::<LastFrontmostAppGlobal>().0.lock().unwrap().clone();
                let commands = cx.global::<CommandsGlobal>().0.clone();
                let db = cx.global::<DbGlobal>().0.clone();
                open_window(commands, db, prev_app, cx);
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

        // Browser keybinding actions — push to live_tabs action inbox
        cx.on_action::<BrowserNewTab>(|_, cx| {
            if let Some(live) = cx.try_global::<LiveBrowserTabsGlobal>() {
                live.0.lock().unwrap().action_inbox.push(rover_browser::BrowserAction::NewTab);
            }
        });
        cx.on_action::<BrowserCloseTab>(|_, cx| {
            if let Some(live) = cx.try_global::<LiveBrowserTabsGlobal>() {
                live.0.lock().unwrap().action_inbox.push(rover_browser::BrowserAction::CloseTab);
            }
        });
        cx.on_action::<BrowserReload>(|_, cx| {
            if let Some(live) = cx.try_global::<LiveBrowserTabsGlobal>() {
                live.0.lock().unwrap().action_inbox.push(rover_browser::BrowserAction::Reload);
            }
        });
        cx.on_action::<BrowserBack>(|_, cx| {
            if let Some(live) = cx.try_global::<LiveBrowserTabsGlobal>() {
                live.0.lock().unwrap().action_inbox.push(rover_browser::BrowserAction::Back);
            }
        });
        cx.on_action::<BrowserForward>(|_, cx| {
            if let Some(live) = cx.try_global::<LiveBrowserTabsGlobal>() {
                live.0.lock().unwrap().action_inbox.push(rover_browser::BrowserAction::Forward);
            }
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

        // Poll frontmost app continuously so we always know the last non-Rover app
        {
            let tracker = last_frontmost.clone();
            cx.spawn(async move |cx: &mut AsyncApp| {
                loop {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(500))
                        .await;
                    if let Some(app) = delegate::RoverDelegate::detect_frontmost_app() {
                        if let Ok(mut lock) = tracker.lock() {
                            *lock = Some(app);
                        }
                    }
                }
            })
            .detach();
        }

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
                        if window_open {
                            // Dismiss the existing window
                            cx.global::<WindowOpenGlobal>().0.store(false, Ordering::Relaxed);
                            if let Some(handle) = cx.global::<WindowHandleGlobal>().0 {
                                let _ = handle.update(cx, |_, window, _cx| {
                                    window.remove_window();
                                });
                            }
                            cx.global_mut::<WindowHandleGlobal>().0 = None;
                        } else {
                            // Use the continuously tracked last non-Rover frontmost app
                            let prev_app = cx.global::<LastFrontmostAppGlobal>().0.lock().unwrap().clone();
                            let commands = cx.global::<CommandsGlobal>().0.clone();
                            let db = cx.global::<DbGlobal>().0.clone();
                            open_window(commands, db, prev_app, cx);
                        }
                    });
                }
            }
        })
        .detach();

        match cli {
            CliCommand::None => {
                // Just start the menubar app — picker opens via hotkey
                let commands = cx.global::<CommandsGlobal>().0.clone();
                let db_clone = cx.global::<DbGlobal>().0.clone();
                let prev_app = cx.global::<LastFrontmostAppGlobal>().0.lock().unwrap().clone();
                open_window(commands, db_clone, prev_app, cx);
            }
            CliCommand::Browser => {
                rover_browser::open_browser_window_with_live_tabs(
                    cx, rover_browser::DEFAULT_URL, Some(db), None, Some(live_browser_tabs.clone()),
                );
            }
            CliCommand::OpenUrl(url) => {
                rover_browser::open_browser_window_with_live_tabs(
                    cx, &url, Some(db), None, Some(live_browser_tabs.clone()),
                );
            }
            CliCommand::Search(query) => {
                let encoded = urlencoding::encode(&query);
                let url = format!("https://www.perplexity.ai/search?q={encoded}");
                rover_browser::open_browser_window_with_live_tabs(
                    cx, &url, Some(db), None, Some(live_browser_tabs.clone()),
                );
            }
        }
    });
}

/// Get screen dimensions (width, height) from macOS.
#[cfg(target_os = "macos")]
pub(crate) fn screen_size() -> (f32, f32) {
    use objc::{class, msg_send, sel, sel_impl};
    use cocoa::foundation::NSRect;
    unsafe {
        let main: *mut objc::runtime::Object = msg_send![class!(NSScreen), mainScreen];
        let frame: NSRect = msg_send![main, frame];
        (frame.size.width as f32, frame.size.height as f32)
    }
}

/// Get current cursor position in GPUI coords (top-left origin).
#[cfg(target_os = "macos")]
fn cursor_position() -> (f32, f32) {
    use objc::{class, msg_send, sel, sel_impl};
    use cocoa::foundation::{NSPoint, NSRect};
    unsafe {
        let loc: NSPoint = msg_send![class!(NSEvent), mouseLocation];
        let main: *mut objc::runtime::Object = msg_send![class!(NSScreen), mainScreen];
        let frame: NSRect = msg_send![main, frame];
        let sh = frame.size.height as f32;
        (loc.x as f32, sh - loc.y as f32)
    }
}

/// Make the NSPanel truly borderless — no shadow, clear background, non-opaque.
/// Also configures it as an overlay panel (like Raycast) so Rover doesn't become
/// the frontmost app — the previous app remains active in the system.
#[cfg(target_os = "macos")]
fn configure_borderless_panel(window: &WindowHandle<RoverWindow>, cx: &mut App) {
    use objc::{class, msg_send, sel, sel_impl};
    use raw_window_handle::HasWindowHandle;

    let _ = window.update(cx, |_, window, _cx| {
        if let Ok(handle) = window.window_handle() {
            if let raw_window_handle::RawWindowHandle::AppKit(appkit) = handle.as_ref() {
                let ns_view = appkit.ns_view.as_ptr() as *mut objc::runtime::Object;
                unsafe {
                    let ns_window: *mut objc::runtime::Object = msg_send![ns_view, window];
                    if !ns_window.is_null() {
                        let _: () = msg_send![ns_window, setHasShadow: false];
                        let _: () = msg_send![ns_window, setOpaque: false];
                        let clear: *mut objc::runtime::Object =
                            msg_send![class!(NSColor), clearColor];
                        let _: () = msg_send![ns_window, setBackgroundColor: clear];
                        // Float above other windows without activating the app
                        // NSFloatingWindowLevel = 3 (CGWindowLevelForKey(kCGFloatingWindowLevelKey))
                        let _: () = msg_send![ns_window, setLevel: 3i64];
                        // NSNonactivatingPanelMask = 1 << 7 = 128
                        let style: u64 = msg_send![ns_window, styleMask];
                        let _: () = msg_send![ns_window, setStyleMask: style | 128u64];
                        // Allow the panel to become key (receive keyboard) without activating the app
                        let _: () = msg_send![ns_window, setFloatingPanel: true];
                        let _: () = msg_send![ns_window, setWorksWhenModal: true];
                        let _: () = msg_send![ns_window, setHidesOnDeactivate: false];
                    }
                }
            }
        }
    });
}

fn open_window(commands: Vec<rover_commands::Command>, db: Arc<RoverDb>, previous_app: Option<String>, cx: &mut App) {

    let window_open_flag = cx.global::<WindowOpenGlobal>().0.clone();
    window_open_flag.store(true, Ordering::Relaxed);

    let w = 440.0f32;
    let h = 300.0f32;

    let picker_position = db
        .get_setting("picker_position")
        .ok()
        .flatten()
        .unwrap_or_else(|| "center".to_string());

    #[cfg(target_os = "macos")]
    let bounds = {
        let (sw, sh) = screen_size();
        match picker_position.as_str() {
            "cursor" => {
                let (cx_pos, cy_pos) = cursor_position();
                // Position window so the input bar (at bottom) sits just above the cursor,
                // offset slightly to the right so the cursor isn't hidden.
                let x = (cx_pos - w * 0.35).max(8.0).min(sw - w - 8.0);
                let y = (cy_pos - h - 8.0).max(8.0).min(sh - h - 8.0);
                Bounds {
                    origin: point(px(x), px(y)),
                    size: size(px(w), px(h)),
                }
            }
            "top" => {
                let x = (sw - w) / 2.0;
                Bounds {
                    origin: point(px(x), px(40.0)),
                    size: size(px(w), px(h)),
                }
            }
            "bottom" => {
                let x = (sw - w) / 2.0;
                let y = sh - h - 40.0;
                Bounds {
                    origin: point(px(x), px(y)),
                    size: size(px(w), px(h)),
                }
            }
            _ => {
                // "center" (default)
                let x = (sw - w) / 2.0;
                let y = (sh - h) / 2.0;
                Bounds {
                    origin: point(px(x), px(y)),
                    size: size(px(w), px(h)),
                }
            }
        }
    };
    #[cfg(not(target_os = "macos"))]
    let bounds = Bounds::centered(None, size(px(w), px(h)), cx);

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

    let ext_runtimes = cx.global::<ExtensionRuntimesGlobal>().0.clone();
    // Read live browser context (populated by browser window when open)
    let live_tabs_ref = cx.global::<LiveBrowserTabsGlobal>().0.clone();
    let live_browser_ctx = {
        let lt = live_tabs_ref.lock().unwrap();
        if lt.tabs.is_empty() {
            None
        } else {
            Some(delegate::BrowserContext {
                tabs: lt.tabs.iter().map(|(t, u, s, la)| (t.clone(), u.clone(), s.clone(), *la)).collect(),
                active_tab: lt.active_tab,
            })
        }
    };

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
                    let semantic = SEMANTIC_INDEX.lock().unwrap().clone();
                    let compact = settings_db
                        .get_setting("compact_mode")
                        .ok()
                        .flatten()
                        .map(|v| v == "true")
                        .unwrap_or(false);
                    // Browser tabs only show if the browser window is actually open (live tabs exist)
                    let browser_focused = live_browser_ctx.as_ref().map(|c| !c.tabs.is_empty()).unwrap_or(false);
                    let terminal_focused = previous_app.as_ref().map(|a| a == "Ghostty").unwrap_or(false);
                    let mut delegate = RoverDelegate::new(commands, db, ai_session, ai_enabled, semantic, live_browser_ctx, Some(live_tabs_ref), ext_runtimes);
                    delegate.previous_frontmost_app = previous_app.clone();
                    delegate.browser_was_focused = browser_focused;
                    delegate.terminal_was_focused = terminal_focused;
                    let picker = cx.new(|cx| {
                        let mut p = Picker::new(delegate, cx);
                        p.compact = compact;
                        p
                    });

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
                                PickerEvent::OpenTerminal { cmd } => {
                                    dismiss_flag.store(false, Ordering::Relaxed);
                                    window.remove_window();
                                    terminal::open_ghostty(cmd.as_deref(), cx);
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

    cx.global_mut::<WindowHandleGlobal>().0 = Some(window);

    // Make the NSPanel truly borderless and overlay (no macOS window shadow/frame)
    // Must happen BEFORE focusing so the panel is already configured as non-activating
    #[cfg(target_os = "macos")]
    configure_borderless_panel(&window, cx);

    window
        .update(cx, |view, window, cx| {
            let input_focus = view.picker.read(cx).text_input.focus_handle(cx);
            window.focus(&input_focus);
            // Don't call cx.activate(true) — the panel is configured as a floating
            // overlay that receives keyboard input without activating Rover as the
            // frontmost app. This keeps the previous app as "frontmost" in System Events.
        })
        .unwrap();

    // Auto-dismiss when the panel loses key status (user clicked elsewhere or switched apps)
    let dismiss_flag = window_open_flag;
    cx.spawn(async move |cx: &mut AsyncApp| {
        cx.background_executor()
            .timer(std::time::Duration::from_millis(200))
            .await;
        loop {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(50))
                .await;
            let is_key = window
                .update(cx, |_, window, _cx| {
                    // Check if this window is still the key window (receives keyboard input)
                    window.is_window_active()
                })
                .unwrap_or(false);
            if !is_key {
                dismiss_flag.store(false, Ordering::Relaxed);
                let _ = window.update(cx, |_, window, _cx| {
                    window.remove_window();
                });
                break;
            }
        }
    })
    .detach();
}

struct RoverWindow {
    picker: Entity<Picker<RoverDelegate>>,
}

impl Render for RoverWindow {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(self.picker.clone())
    }
}
