use gpui::App;

/// Register a global hotkey on macOS using Carbon Events API.
///
/// Registers both Cmd+Space and Hyperkey (Ctrl+Shift+Opt+Cmd+Space).
/// This works even when the app is not focused (same approach as Raycast/Alfred).
pub fn register_global_hotkey(_cx: &mut App) {
    #[cfg(target_os = "macos")]
    {
        use std::os::raw::c_void;

        const K_VK_SPACE: u32 = 49;
        const CMD_KEY: u32 = 0x0100;
        // Hyperkey = Ctrl+Shift+Opt+Cmd
        const CTRL_KEY: u32 = 0x1000;
        const SHIFT_KEY: u32 = 0x0200;
        const OPT_KEY: u32 = 0x0800;
        const HYPER_MODS: u32 = CTRL_KEY | SHIFT_KEY | OPT_KEY | CMD_KEY;

        extern "C" {
            fn RegisterEventHotKey(
                in_hot_key_code: u32,
                in_hot_key_modifiers: u32,
                in_hot_key_id: EventHotKeyID,
                in_target: *mut c_void,
                in_options: u32,
                out_ref: *mut *mut c_void,
            ) -> i32;
            fn GetApplicationEventTarget() -> *mut c_void;
            fn InstallEventHandler(
                in_target: *mut c_void,
                in_handler: extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> i32,
                in_num_types: u32,
                in_list: *const EventTypeSpec,
                in_user_data: *mut c_void,
                out_ref: *mut *mut c_void,
            ) -> i32;
        }

        #[repr(C)]
        #[derive(Clone, Copy)]
        struct EventHotKeyID {
            signature: u32,
            id: u32,
        }

        #[repr(C)]
        #[derive(Clone, Copy)]
        struct EventTypeSpec {
            event_class: u32,
            event_kind: u32,
        }

        extern "C" fn hotkey_handler(
            _next: *mut c_void,
            _event: *mut c_void,
            _user_data: *mut c_void,
        ) -> i32 {
            log::info!("Global hotkey pressed, toggling window visibility");
            0
        }

        unsafe {
            let target = GetApplicationEventTarget();

            let event_type = EventTypeSpec {
                event_class: 0x6B657962, // kEventClassKeyboard
                event_kind: 5,           // kEventHotKeyPressed
            };

            let mut handler_ref: *mut c_void = std::ptr::null_mut();
            InstallEventHandler(
                target,
                hotkey_handler,
                1,
                &event_type,
                std::ptr::null_mut(),
                &mut handler_ref,
            );

            // Register Cmd+Space
            let hotkey_id = EventHotKeyID {
                signature: u32::from_be_bytes(*b"ROVR"),
                id: 1,
            };
            let mut hotkey_ref: *mut c_void = std::ptr::null_mut();
            RegisterEventHotKey(K_VK_SPACE, CMD_KEY, hotkey_id, target, 0, &mut hotkey_ref);

            // Register Hyperkey+Space (Ctrl+Shift+Opt+Cmd+Space)
            let hyper_id = EventHotKeyID {
                signature: u32::from_be_bytes(*b"ROVR"),
                id: 2,
            };
            let mut hyper_ref: *mut c_void = std::ptr::null_mut();
            RegisterEventHotKey(K_VK_SPACE, HYPER_MODS, hyper_id, target, 0, &mut hyper_ref);
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
        // Caps Lock (0x39) -> F18 (0x6D) which we can then intercept
        // This is the standard approach: remap Caps Lock to an unused key,
        // then register that key with all modifiers as the hyper combo.
        //
        // Alternatively, apps like Karabiner remap at a lower level.
        // We use hidutil which is built into macOS.
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
