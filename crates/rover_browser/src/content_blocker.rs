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

#[cfg(target_os = "macos")]
mod macos {
    use block::ConcreteBlock;
    use cocoa::base::{id, nil};
    use cocoa::foundation::NSString;
    use objc::*;
    use once_cell::sync::Lazy;

    // -------------------------------------------------------------------------
    // Bundled resources (compiled into the binary)
    // -------------------------------------------------------------------------

    /// Network-blocking rules in WKContentRuleList JSON format.
    static RULES_JSON: &str = include_str!("../resources/content_rules.json");

    /// CSS element-hiding script (cosmetic filtering).
    static COSMETIC_JS: &str = include_str!("../resources/cosmetic.js");

    /// Ad-framework and tracker stubs (scriptlet injection).
    static SCRIPTLETS_JS: &str = include_str!("../resources/scriptlets.js");

    /// Bump this whenever `content_rules.json` changes materially to force
    /// the OS to recompile the cached bytecode.
    const RULES_ID: &str = "rover-content-blocker-v3";

    // -------------------------------------------------------------------------
    // Thread-safe id wrapper
    // -------------------------------------------------------------------------

    struct RuleList(id);
    unsafe impl Send for RuleList {}
    unsafe impl Sync for RuleList {}

    // -------------------------------------------------------------------------
    // Global compiled rule-list (initialised once per process)
    // -------------------------------------------------------------------------

    static CONTENT_RULE_LIST: Lazy<Option<RuleList>> =
        Lazy::new(|| unsafe { compile_rules() }.map(RuleList));

    // -------------------------------------------------------------------------
    // Rule compilation
    // -------------------------------------------------------------------------

    /// Compile `RULES_JSON` via `WKContentRuleListStore`.
    ///
    /// The store caches compiled bytecode on disk keyed by `RULES_ID`, so the
    /// JSON→bytecode pass only runs on first launch (or when the ID changes).
    ///
    /// # Safety
    /// Must be called from the main thread; NSRunLoop is spun to drive the
    /// async completion handler back synchronously.
    unsafe fn compile_rules() -> Option<id> {
        let store: id = msg_send![class!(WKContentRuleListStore), defaultStore];
        if store == nil {
            log::warn!("content_blocker: WKContentRuleListStore unavailable");
            return None;
        }

        let identifier: id = NSString::alloc(nil).init_str(RULES_ID);
        let json_ns: id = NSString::alloc(nil).init_str(RULES_JSON);

        use std::sync::{
            atomic::{AtomicBool, Ordering},
            Arc, Mutex,
        };
        let done = Arc::new(AtomicBool::new(false));
        let result: Arc<Mutex<Option<id>>> = Arc::new(Mutex::new(None));
        let done_c = done.clone();
        let result_c = result.clone();

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
                *result_c.lock().unwrap() = Some(list);
            }
            done_c.store(true, Ordering::Release);
        });
        let completion = completion.copy();

        let _: () = msg_send![
            store,
            compileContentRuleListForIdentifier: identifier
            encodedContentRuleList: json_ns
            completionHandler: &*completion
        ];

        // Spin the main-thread RunLoop until the completion fires.
        // This is the standard pattern for synchronising async ObjC callbacks
        // when you must stay on the main thread.
        let runloop: id = msg_send![class!(NSRunLoop), currentRunLoop];
        while !done.load(Ordering::Acquire) {
            let interval: std::os::raw::c_double = 0.01;
            let date: id =
                msg_send![class!(NSDate), dateWithTimeIntervalSinceNow: interval];
            let _: () = msg_send![runloop, runUntilDate: date];
        }

        let list = result.lock().unwrap().take();
        if list.is_none() {
            log::warn!("content_blocker: compiled list is nil, network blocking disabled");
        }
        list
    }

    // -------------------------------------------------------------------------
    // User-script helpers
    // -------------------------------------------------------------------------

    /// `WKUserScriptInjectionTime` values (from WebKit headers).
    const AT_DOCUMENT_START: usize = 0;

    /// Create a `WKUserScript` and add it to the config's userContentController.
    ///
    /// `for_main_frame_only = false` means the script also runs inside iframes,
    /// matching uBOL's `"allFrames": true` content-script setting.
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

    // -------------------------------------------------------------------------
    // Public API
    // -------------------------------------------------------------------------

    /// Apply all three layers of content blocking to a `WKWebViewConfiguration`.
    ///
    /// Call this **before** `initWithFrame:configuration:` so every new WebView
    /// has blocking active from its very first request / paint.
    ///
    /// Layers applied:
    /// - Network rule-list (WKContentRuleList) — blocks ad/tracker requests
    /// - Cosmetic script  (WKUserScript, document-start) — hides ad elements
    /// - Scriptlet script (WKUserScript, document-start) — stubs tracking SDKs
    ///
    /// # Safety
    /// `config` must be a valid `WKWebViewConfiguration *` on the main thread.
    pub unsafe fn apply_to_config(config: id) {
        if config == nil {
            return;
        }

        let ucc: id = msg_send![config, userContentController];

        // Layer 1 — network blocking via WKContentRuleList.
        // `Lazy` ensures compile_rules() runs only once per process.
        if let Some(rule_list) = CONTENT_RULE_LIST.as_ref().map(|rl| rl.0) {
            let _: () = msg_send![ucc, addContentRuleList: rule_list];
            log::debug!("content_blocker: network rule list applied");
        }

        // Layer 2 — cosmetic filtering (CSS element hiding + MutationObserver guard).
        add_user_script(ucc, COSMETIC_JS, AT_DOCUMENT_START);
        log::debug!("content_blocker: cosmetic script injected");

        // Layer 3 — scriptlets (ad-framework stubs, tracker no-ops).
        add_user_script(ucc, SCRIPTLETS_JS, AT_DOCUMENT_START);
        log::debug!("content_blocker: scriptlet script injected");
    }
}

#[cfg(target_os = "macos")]
pub use macos::apply_to_config;

// On non-macOS targets this module is a no-op; callers are already gated by
// `#[cfg(target_os = "macos")]`.
