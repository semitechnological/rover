//! WKWebExtension loader for Rover.
//!
//! Scans `~/.config/rover/extensions/` for subdirectories and tries to load each
//! as a WKWebExtension. Returns a WKWebExtensionController id if any extensions
//! were loaded, or None on failure / unsupported OS version.
//!
//! WKWebExtension API requires macOS 15.4+. We gate on this at runtime.

#[cfg(target_os = "macos")]
mod macos {
    use cocoa::base::{id, nil};
    use objc::*;

    /// Minimum macOS version required for WKWebExtension APIs (15.4).
    const MIN_MAJOR: i64 = 15;
    const MIN_MINOR: i64 = 4;

    unsafe fn macos_version_ok() -> bool {
        let process_info: id = msg_send![class!(NSProcessInfo), processInfo];
        if process_info == nil {
            return false;
        }
        // NSOperatingSystemVersion is a struct { major, minor, patch } — 3 NSIntegers.
        // We call operatingSystemVersion and read major/minor.
        // objc 0.2 returns the struct as (i64, i64, i64) on 64-bit.
        let (major, minor, _patch): (i64, i64, i64) =
            msg_send![process_info, operatingSystemVersion];
        major > MIN_MAJOR || (major == MIN_MAJOR && minor >= MIN_MINOR)
    }

    /// Load all WKWebExtensions found under `~/.config/rover/extensions/`.
    ///
    /// Returns `Some(controller_id)` if at least one extension was loaded successfully,
    /// `None` if none were found or the OS version is too old.
    ///
    /// # Safety
    /// Must be called from the main thread.
    pub unsafe fn load_extensions() -> Option<id> {
        log::info!("extensions: load_extensions called");
        if !macos_version_ok() {
            log::info!("extensions: macOS < 15.4, WKWebExtension API unavailable, skipping");
            return None;
        }
        log::info!("extensions: macOS version ok, checking extensions dir");

        let extensions_dir = dirs::config_dir()?.join("rover").join("extensions");
        log::info!("extensions: dir = {:?}, exists={}", extensions_dir, extensions_dir.exists());
        if !extensions_dir.exists() {
            log::info!("extensions: no extensions dir, skipping");
            return None;
        }

        let entries = match std::fs::read_dir(&extensions_dir) {
            Ok(e) => e,
            Err(e) => {
                log::warn!("extensions: could not read {:?}: {e}", extensions_dir);
                return None;
            }
        };

        // WKWebExtensionController (default non-persistent controller)
        let controller: id = msg_send![class!(WKWebExtensionController), new];
        if controller == nil {
            return None;
        }

        let mut loaded = 0usize;

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let path_str = match path.to_str() {
                Some(s) => s,
                None => continue,
            };

            // Build NSURL for the extension directory.
            use cocoa::foundation::NSString;
            let ns_path: id = NSString::alloc(nil).init_str(path_str);
            let ns_url: id = msg_send![class!(NSURL), fileURLWithPath: ns_path];
            if ns_url == nil {
                continue;
            }

            // WKWebExtension *ext = [WKWebExtension extensionWithResourceBaseURL:ns_url error:NULL];
            let ext: id = msg_send![class!(WKWebExtension), extensionWithResourceBaseURL: ns_url error: nil as *mut id];
            if ext == nil {
                log::warn!("extensions: failed to load extension at {:?}", path);
                continue;
            }

            // WKWebExtensionContext *ctx = [WKWebExtensionContext contextForExtension:ext];
            let ctx: id = msg_send![class!(WKWebExtensionContext), contextForExtension: ext];
            if ctx == nil {
                continue;
            }

            // [controller addExtensionContext:ctx error:NULL]
            let ok: bool = msg_send![controller, addExtensionContext: ctx error: nil as *mut id];
            if ok {
                log::info!("extensions: loaded {:?}", path.file_name().unwrap_or_default());
                loaded += 1;
            } else {
                log::warn!("extensions: could not add context for {:?}", path);
            }
        }

        if loaded == 0 {
            None
        } else {
            Some(controller)
        }
    }
}

#[cfg(target_os = "macos")]
pub use macos::load_extensions;

// Stub for non-macOS targets so the module compiles.
#[cfg(not(target_os = "macos"))]
pub fn load_extensions() -> Option<()> {
    None
}
