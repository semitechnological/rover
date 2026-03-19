mod delegate;
mod hotkey;

use delegate::RoverDelegate;
use gpui::*;
use rover_commands::{register_apps, CommandRegistry};
use rover_extensions::{load_extensions, register_extension_themes};
use rover_picker::{bind_picker_keys, Picker, PickerEvent};
use rover_theme::{self, ThemeRegistry};

actions!(rover, [Toggle, Quit]);

fn main() {
    Application::new().run(|cx| {
        rover_theme::init(cx);

        if let Ok(extensions) = load_extensions() {
            let registry = cx.global_mut::<ThemeRegistry>();
            register_extension_themes(&extensions, registry);
        }

        let mut cmd_registry = CommandRegistry::new();
        register_apps(&mut cmd_registry);
        let commands: Vec<_> = cmd_registry.commands().to_vec();

        bind_picker_keys(cx);
        cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);

        cx.set_menus(vec![Menu {
            name: "Rover".into(),
            items: vec![MenuItem::action("Quit", Quit)],
        }]);

        cx.on_action::<Quit>(|_, cx| cx.quit());

        hotkey::register_global_hotkey(cx);

        open_window(commands, cx);
    });
}

fn open_window(commands: Vec<rover_commands::Command>, cx: &mut App) {
    let bounds = Bounds::centered(None, size(px(680.0), px(420.0)), cx);

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
            |_window, cx| {
                cx.new(|cx| {
                    let delegate = RoverDelegate::new(commands);
                    let picker = cx.new(|cx| Picker::new(delegate, cx));

                    cx.subscribe(
                        &picker,
                        |_this: &mut RoverWindow, _picker, event: &PickerEvent, cx| match event {
                            PickerEvent::Dismiss => {
                                cx.hide();
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
