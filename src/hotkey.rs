use gpui::App;

/// Register a global Cmd+Space hotkey on macOS.
///
/// Uses the Carbon Events API (RegisterEventHotKey) which works even when
/// the app is not focused. This is the same approach Raycast/Alfred use.
///
/// Note: This requires accessibility permissions on modern macOS.
pub fn register_global_hotkey(_cx: &mut App) {
    #[cfg(target_os = "macos")]
    {
        use std::os::raw::c_void;

        // Carbon key codes
        const K_VK_SPACE: u32 = 49;
        const CMD_KEY: u32 = 0x0100; // cmdKey

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
            // TODO: Send Toggle action to the app via a channel or callback.
            // For now this is a placeholder; full integration requires
            // bridging back into GPUI's event loop.
            log::info!("Global hotkey Cmd+Space pressed");
            0 // noErr
        }

        unsafe {
            let target = GetApplicationEventTarget();

            // kEventClassKeyboard = 'keyb' = 0x6B657962
            // kEventHotKeyPressed = 5
            let event_type = EventTypeSpec {
                event_class: 0x6B657962,
                event_kind: 5,
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

            let hotkey_id = EventHotKeyID {
                signature: u32::from_be_bytes(*b"ROVR"),
                id: 1,
            };

            let mut hotkey_ref: *mut c_void = std::ptr::null_mut();
            RegisterEventHotKey(K_VK_SPACE, CMD_KEY, hotkey_id, target, 0, &mut hotkey_ref);
        }
    }
}
