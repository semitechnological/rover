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
use std::sync::Arc;

actions!(rover, [Toggle, Quit, OpenSettings, ShowAbout]);

struct DbGlobal(pub Arc<RoverDb>);
impl Global for DbGlobal {}

fn main() {
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

        // Make db accessible to global action handlers
        cx.set_global(DbGlobal(db.clone()));

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
                MenuItem::action("About Rover", ShowAbout),
                MenuItem::action("Settings...", OpenSettings),
                MenuItem::separator(),
                MenuItem::action("Quit", Quit),
            ],
        }]);

        cx.on_action::<Quit>(|_, cx| cx.quit());

        cx.on_action::<OpenSettings>(|_, cx| {
            let db = cx.global::<DbGlobal>().0.clone();
            settings_window::open_settings_window(db, cx);
        });

        cx.on_action::<ShowAbout>(|_, cx| {
            let db = cx.global::<DbGlobal>().0.clone();
            settings_window::open_settings_window_on_about(db, cx);
        });

        hotkey::register_global_hotkey(cx);

        open_window(commands, db, cx);
    });
}

fn open_window(commands: Vec<rover_commands::Command>, db: Arc<RoverDb>, cx: &mut App) {
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
                cx.new(|cx| {
                    let delegate = RoverDelegate::new(commands, db, ai_session, ai_enabled);
                    let picker = cx.new(|cx| Picker::new(delegate, cx));

                    cx.subscribe_in(
                        &picker,
                        window,
                        move |_this: &mut RoverWindow, _picker, event: &PickerEvent, window, cx| {
                            match event {
                                PickerEvent::Dismiss => {
                                    window.remove_window();
                                }
                                PickerEvent::OpenSettings => {
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
                                PickerEvent::OpenTerminal => {}
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
