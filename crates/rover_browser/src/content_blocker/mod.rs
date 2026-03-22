//! uBOL-derived content blocking for the Rover browser.
//!
//! Implements the three layers that uBOL uses on WebKit-based platforms:
//!
//! 1. **Network blocking** — `WKContentRuleList` compiled from our JSON ruleset
//!    (ad/tracker domains, URL patterns).  The OS caches compiled bytecode to
//!    disk, so compilation only runs on first launch or after rules change.
//!
//! 2. **Cosmetic filtering** — `WKUserScript` injecting a `<style>` node with
//!    `display:none!important` for known ad element selectors (EasyList cosmetic
//!    equivalents).  A `MutationObserver` guards against the style node being
//!    removed by the page.
//!
//! 3. **Scriptlet injection** — `WKUserScript` at document-start that stubs
//!    out tracking SDKs and ad-framework entry-points before page scripts run,
//!    mirroring uBOL's `web_accessible_resources/` scriptlets.
//!
//! Compilation is fire-and-forget: we kick it off early and store the result
//! when the completion handler fires on the run loop.  This avoids blocking
//! GPUI's render/paint cycle with a synchronous NSRunLoop spin.

mod cosmetic;
mod rules;
mod scriptlets;

#[cfg(target_os = "macos")]
mod macos {
    use block::ConcreteBlock;
    use cocoa::base::{id, nil};
    use cocoa::foundation::NSString;
    use objc::*;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    };

    use super::cosmetic::COSMETIC_JS;
    use super::rules::RULES_JSON;
    use super::scriptlets::SCRIPTLETS_JS;

    /// Bump this whenever the rules change materially to force the OS to
    /// recompile the cached bytecode.
    const RULES_ID: &str = "rover-content-blocker-v3";

    /// Wraps a raw ObjC `id` so it can live in a `static Mutex`.
    struct SendId(id);
    unsafe impl Send for SendId {}
    unsafe impl Sync for SendId {}

    /// Compiled WKContentRuleList, available once the async completion fires.
    static CONTENT_RULE_LIST: Mutex<Option<SendId>> = Mutex::new(None);
    /// Guards against starting compilation more than once.
    static COMPILE_STARTED: AtomicBool = AtomicBool::new(false);

    /// Kick off compilation of the content rule list.
    ///
    /// Non-blocking: the completion handler stores the result when the run loop
    /// next processes it.  Safe to call from any main-thread context.
    pub fn prewarm() {
        if COMPILE_STARTED
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
        {
            unsafe { start_async_compile() };
        }
    }

    unsafe fn start_async_compile() {
        let store: id = msg_send![class!(WKContentRuleListStore), defaultStore];
        if store == nil {
            log::warn!("content_blocker: WKContentRuleListStore unavailable");
            return;
        }

        let identifier: id = NSString::alloc(nil).init_str(RULES_ID);
        let json_ns: id = NSString::alloc(nil).init_str(RULES_JSON);

        // The completion handler is called on the main queue by WKContentRuleListStore.
        // We just store the result; no blocking needed.
        let completion = ConcreteBlock::new(move |list: id, error: id| {
            if error != nil {
                let desc: id = msg_send![error, localizedDescription];
                let utf8: *const std::os::raw::c_char = msg_send![desc, UTF8String];
                if !utf8.is_null() {
                    if let Ok(s) = std::ffi::CStr::from_ptr(utf8).to_str() {
                        log::error!("content_blocker: compilation failed: {s}");
                    }
                }
            } else if list != nil {
                let retained: id = msg_send![list, retain];
                if !retained.is_null() {
                    if let Ok(mut guard) = CONTENT_RULE_LIST.lock() {
                        *guard = Some(SendId(retained));
                        log::debug!("content_blocker: rule list ready");
                    }
                }
            }
        });
        let completion = completion.copy();

        // Fire-and-forget: WKContentRuleListStore retains the completion block.
        let _: () = msg_send![
            store,
            compileContentRuleListForIdentifier: identifier
            encodedContentRuleList: json_ns
            completionHandler: &*completion
        ];
        // `completion` drops here but WKContentRuleListStore already copied it.
    }

    const AT_DOCUMENT_START: usize = 0;

    unsafe fn add_user_script(ucc: id, source: &str, injection_time: usize) {
        let src_ns: id = NSString::alloc(nil).init_str(source);
        let script: id = msg_send![class!(WKUserScript), alloc];
        let script: id = msg_send![
            script,
            initWithSource: src_ns
            injectionTime: injection_time
            forMainFrameOnly: false
        ];
        if script != nil {
            let _: () = msg_send![ucc, addUserScript: script];
        }
    }

    /// Apply all three layers of content blocking to a `WKWebViewConfiguration`.
    ///
    /// Network blocking is applied only if the rule list has already compiled.
    /// Cosmetic filtering and scriptlets are always applied immediately.
    ///
    /// # Safety
    /// `config` must be a valid `WKWebViewConfiguration *` on the main thread.
    pub unsafe fn apply_to_config(config: id) {
        if config == nil {
            return;
        }

        let ucc: id = msg_send![config, userContentController];

        // Layer 1 — network blocking via WKContentRuleList (if compiled yet).
        if let Ok(guard) = CONTENT_RULE_LIST.lock() {
            if let Some(ref rl) = *guard {
                if !rl.0.is_null() {
                    let _: () = msg_send![ucc, addContentRuleList: rl.0];
                    log::debug!("content_blocker: network rule list applied");
                }
            }
        }

        // Layer 2 — cosmetic filtering.
        add_user_script(ucc, COSMETIC_JS, AT_DOCUMENT_START);

        // Layer 3 — scriptlets.
        add_user_script(ucc, SCRIPTLETS_JS, AT_DOCUMENT_START);
    }
}

#[cfg(target_os = "macos")]
pub use macos::{apply_to_config, prewarm};

#[cfg(not(target_os = "macos"))]
pub fn prewarm() {}
