use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

// ─── Toggle signal ────────────────────────────────────────────────────────────

static TOGGLE_REQUESTED: AtomicBool = AtomicBool::new(false);

pub fn take_toggle_requested() -> bool {
    TOGGLE_REQUESTED.swap(false, Ordering::Relaxed)
}

// ─── Carbon modifier bits ─────────────────────────────────────────────────────

pub const K_VK_SPACE: u32 = 49;
pub const CMD_KEY: u32 = 0x0100;
pub const OPT_KEY: u32 = 0x0800;
pub const CTRL_KEY: u32 = 0x1000;
pub const SHIFT_KEY: u32 = 0x0200;
const HYPER_MODS: u32 = CTRL_KEY | SHIFT_KEY | OPT_KEY | CMD_KEY;

// ─── Virtual key code table ──────────────────────────────────────────────────
//
// Maps GPUI Keystroke.key strings to macOS Carbon virtual key codes.
// Only includes keys useful as global hotkey triggers (no pure modifiers).

pub const VK_MAP: &[(&str, u32)] = &[
    // Letters (QWERTY layout positions)
    ("a", 0),
    ("s", 1),
    ("d", 2),
    ("f", 3),
    ("g", 5),
    ("h", 4),
    ("j", 38),
    ("k", 40),
    ("l", 37),
    ("q", 12),
    ("w", 13),
    ("e", 14),
    ("r", 15),
    ("t", 17),
    ("y", 16),
    ("u", 32),
    ("i", 34),
    ("o", 31),
    ("p", 35),
    ("z", 6),
    ("x", 7),
    ("c", 8),
    ("v", 9),
    ("b", 11),
    ("n", 45),
    ("m", 46),
    // Numbers
    ("0", 29),
    ("1", 18),
    ("2", 19),
    ("3", 20),
    ("4", 21),
    ("5", 23),
    ("6", 22),
    ("7", 26),
    ("8", 28),
    ("9", 25),
    // Function keys
    ("f1", 122),
    ("f2", 120),
    ("f3", 99),
    ("f4", 118),
    ("f5", 96),
    ("f6", 97),
    ("f7", 98),
    ("f8", 100),
    ("f9", 101),
    ("f10", 109),
    ("f11", 103),
    ("f12", 111),
    ("f13", 105),
    ("f14", 107),
    ("f15", 113),
    ("f16", 106),
    ("f17", 64),
    ("f18", 79),
    ("f19", 80),
    ("f20", 90),
    // Special
    ("space", 49),
    ("return", 36),
    ("tab", 48),
    ("backspace", 51),
    ("delete", 51),
    ("escape", 53),
    // Arrows
    ("left", 123),
    ("right", 124),
    ("down", 125),
    ("up", 126),
    // Navigation
    ("home", 115),
    ("end", 119),
    ("pageup", 116),
    ("pagedown", 121),
    // Symbols
    ("-", 27),
    ("=", 24),
    ("[", 33),
    ("]", 30),
    ("\\", 42),
    (";", 41),
    ("'", 39),
    (",", 43),
    (".", 47),
    ("/", 44),
    ("`", 50),
];

/// Convert a GPUI key name to a Carbon virtual key code.
pub fn key_name_to_vk(name: &str) -> Option<u32> {
    let lower = name.to_lowercase();
    VK_MAP
        .iter()
        .find(|(k, _)| *k == lower.as_str())
        .map(|(_, v)| *v)
}

/// Build Carbon modifier bits from GPUI Modifiers fields.
pub fn mods_from_gpui(cmd: bool, opt: bool, ctrl: bool, shift: bool) -> u32 {
    let mut m = 0u32;
    if cmd {
        m |= CMD_KEY;
    }
    if opt {
        m |= OPT_KEY;
    }
    if ctrl {
        m |= CTRL_KEY;
    }
    if shift {
        m |= SHIFT_KEY;
    }
    m
}

/// Build a pretty display string for a key combo (e.g. "⌘⌥ F5").
pub fn display_for_combo(modifiers: u32, key_name: &str) -> String {
    let mut s = String::new();
    if modifiers & CTRL_KEY != 0 {
        s.push('⌃');
    }
    if modifiers & OPT_KEY != 0 {
        s.push('⌥');
    }
    if modifiers & SHIFT_KEY != 0 {
        s.push('⇧');
    }
    if modifiers & CMD_KEY != 0 {
        s.push('⌘');
    }
    if !s.is_empty() {
        s.push(' ');
    }
    let pretty_key = match key_name {
        "space" => "Space".to_string(),
        "return" => "↩".to_string(),
        "tab" => "⇥".to_string(),
        "backspace" | "delete" => "⌫".to_string(),
        "escape" => "⎋".to_string(),
        "left" => "←".to_string(),
        "right" => "→".to_string(),
        "up" => "↑".to_string(),
        "down" => "↓".to_string(),
        "home" => "↖".to_string(),
        "end" => "↘".to_string(),
        "pageup" => "⇞".to_string(),
        "pagedown" => "⇟".to_string(),
        other => other.to_uppercase(),
    };
    s.push_str(&pretty_key);
    s
}

// ─── Hotkey presets (kept for backward compatibility) ─────────────────────────

/// (db_key, display_label, carbon_key_code, carbon_modifiers)
pub const PRESETS: &[(&str, &str, u32, u32)] = &[
    ("cmd-space", "⌘ Space", K_VK_SPACE, CMD_KEY),
    ("opt-space", "⌥ Space", K_VK_SPACE, OPT_KEY),
    ("ctrl-space", "⌃ Space", K_VK_SPACE, CTRL_KEY),
    ("cmd-opt-space", "⌘⌥ Space", K_VK_SPACE, CMD_KEY | OPT_KEY),
];

pub fn preset_to_carbon(preset: &str) -> (u32, u32) {
    PRESETS
        .iter()
        .find(|(id, _, _, _)| *id == preset)
        .map(|(_, _, kc, mods)| (*kc, *mods))
        .unwrap_or((K_VK_SPACE, CMD_KEY))
}

// ─── macOS Carbon FFI ─────────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
mod carbon {
    use std::os::raw::c_void;

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct EventHotKeyID {
        pub signature: u32,
        pub id: u32,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct EventTypeSpec {
        pub event_class: u32,
        pub event_kind: u32,
    }

    extern "C" {
        pub fn RegisterEventHotKey(
            in_hot_key_code: u32,
            in_hot_key_modifiers: u32,
            in_hot_key_id: EventHotKeyID,
            in_target: *mut c_void,
            in_options: u32,
            out_ref: *mut *mut c_void,
        ) -> i32;
        pub fn UnregisterEventHotKey(in_ref: *mut c_void) -> i32;
        pub fn GetApplicationEventTarget() -> *mut c_void;
        pub fn InstallEventHandler(
            in_target: *mut c_void,
            in_handler: extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> i32,
            in_num_types: u32,
            in_list: *const EventTypeSpec,
            in_user_data: *mut c_void,
            out_ref: *mut *mut c_void,
        ) -> i32;
    }
}

#[cfg(target_os = "macos")]
struct HotkeyRefPtr(*mut std::os::raw::c_void);
#[cfg(target_os = "macos")]
unsafe impl Send for HotkeyRefPtr {}

#[cfg(target_os = "macos")]
static MAIN_HOTKEY_REF: Mutex<Option<HotkeyRefPtr>> = Mutex::new(None);

#[cfg(target_os = "macos")]
extern "C" fn hotkey_handler(
    _next: *mut std::os::raw::c_void,
    _event: *mut std::os::raw::c_void,
    _user_data: *mut std::os::raw::c_void,
) -> i32 {
    TOGGLE_REQUESTED.store(true, Ordering::Relaxed);
    0
}

// ─── Public API ───────────────────────────────────────────────────────────────

/// Install the Carbon event handler and register the initial main hotkey.
/// Call once at startup with the user's saved (or default) preset.
pub fn register_global_hotkey(key_code: u32, modifiers: u32) {
    #[cfg(target_os = "macos")]
    unsafe {
        use carbon::*;
        use std::os::raw::c_void;

        let target = GetApplicationEventTarget();

        let event_type = EventTypeSpec {
            event_class: 0x6B657962, // kEventClassKeyboard
            event_kind: 5,           // kEventHotKeyPressed
        };

        let mut handler_ref: *mut c_void = std::ptr::null_mut();
        let err = InstallEventHandler(
            target,
            hotkey_handler,
            1,
            &event_type,
            std::ptr::null_mut(),
            &mut handler_ref,
        );
        if err != 0 {
            log::warn!("InstallEventHandler failed with error code {err}");
        }

        // Register the user's chosen hotkey
        let hotkey_id = EventHotKeyID {
            signature: u32::from_be_bytes(*b"ROVR"),
            id: 1,
        };
        let mut hotkey_ref: *mut c_void = std::ptr::null_mut();
        let err = RegisterEventHotKey(key_code, modifiers, hotkey_id, target, 0, &mut hotkey_ref);
        if err != 0 {
            log::warn!("RegisterEventHotKey failed with error code {err}");
        } else if let Ok(mut guard) = MAIN_HOTKEY_REF.lock() {
            *guard = Some(HotkeyRefPtr(hotkey_ref));
        }

        // Always also register Hyperkey+Space as an alternative trigger
        let hyper_id = EventHotKeyID {
            signature: u32::from_be_bytes(*b"ROVR"),
            id: 2,
        };
        let mut hyper_ref: *mut c_void = std::ptr::null_mut();
        let err = RegisterEventHotKey(K_VK_SPACE, HYPER_MODS, hyper_id, target, 0, &mut hyper_ref);
        if err != 0 {
            log::warn!("RegisterEventHotKey (Hyperkey+Space) failed with error code {err}");
        }
    }
}

/// Unregister only the main (user-configurable) hotkey.
/// Call this before register_main_hotkey when the user changes the hotkey in settings.
pub fn unregister_main_hotkey() {
    #[cfg(target_os = "macos")]
    {
        if let Ok(mut guard) = MAIN_HOTKEY_REF.lock() {
            if let Some(HotkeyRefPtr(ptr)) = guard.take() {
                unsafe {
                    carbon::UnregisterEventHotKey(ptr);
                }
            }
        }
    }
}

/// Register a new main hotkey. Call unregister_main_hotkey() first.
pub fn register_main_hotkey(key_code: u32, modifiers: u32) {
    #[cfg(target_os = "macos")]
    unsafe {
        use carbon::*;
        use std::os::raw::c_void;

        let target = GetApplicationEventTarget();
        let hotkey_id = EventHotKeyID {
            signature: u32::from_be_bytes(*b"ROVR"),
            id: 1,
        };
        let mut hotkey_ref: *mut c_void = std::ptr::null_mut();
        let err = RegisterEventHotKey(key_code, modifiers, hotkey_id, target, 0, &mut hotkey_ref);
        if err == 0 {
            if let Ok(mut guard) = MAIN_HOTKEY_REF.lock() {
                *guard = Some(HotkeyRefPtr(hotkey_ref));
            }
        } else {
            log::warn!("RegisterEventHotKey failed with error code {err}");
        }
    }
}

/// Remap Caps Lock to Hyperkey (Ctrl+Shift+Opt+Cmd) system-wide.
///
/// Uses hidutil to remap Caps Lock to a simultaneous press of all four modifiers.
/// This is the same technique Raycast and other macOS tools use.
/// The remapping persists until logout/restart.
pub fn enable_hyperkey_remap() -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("hidutil")
            .args([
                "property",
                "--set",
                r#"{"UserKeyMapping":[{"HIDKeyboardModifierMappingSrc":0x700000039,"HIDKeyboardModifierMappingDst":0x70000006D}]}"#,
            ])
            .spawn()?;
    }
    Ok(())
}

/// Remove the Caps Lock -> Hyperkey remap.
pub fn disable_hyperkey_remap() -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("hidutil")
            .args(["property", "--set", r#"{"UserKeyMapping":[]}"#])
            .spawn()?;
    }
    Ok(())
}
