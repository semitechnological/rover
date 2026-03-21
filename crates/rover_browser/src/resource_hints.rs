//! Resource hint injection via WKUserScript.
//!
//! Injects `<link rel="preconnect">` and `<link rel="dns-prefetch">` tags at
//! document-start for common CDN origins. This tells WebKit to establish
//! connections early, shaving 100-300ms off first resource fetches.

#[cfg(target_os = "macos")]
mod macos {
    use cocoa::base::{id, nil};
    use cocoa::foundation::NSString;
    use objc::*;

    /// Common origins worth pre-connecting to (DNS + TCP + TLS handshake).
    const PRECONNECT_ORIGINS: &[&str] = &[
        "https://fonts.googleapis.com",
        "https://fonts.gstatic.com",
        "https://cdn.jsdelivr.net",
        "https://cdnjs.cloudflare.com",
    ];

    /// JavaScript that injects preconnect link tags into the document head.
    fn build_hint_script() -> String {
        let links: Vec<String> = PRECONNECT_ORIGINS
            .iter()
            .map(|origin| {
                format!(
                    "h.appendChild(Object.assign(document.createElement('link'),\
                     {{rel:'preconnect',href:'{origin}',crossOrigin:'anonymous'}}));\
                     h.appendChild(Object.assign(document.createElement('link'),\
                     {{rel:'dns-prefetch',href:'{origin}'}}));"
                )
            })
            .collect();

        format!(
            "(function(){{var h=document.head;if(!h)return;{}}})();",
            links.join("")
        )
    }

    /// Inject resource hint `<link>` tags into a WKWebViewConfiguration's
    /// user content controller. Call before `initWithFrame:configuration:`.
    ///
    /// # Safety
    /// `config` must be a valid `WKWebViewConfiguration *` on the main thread.
    pub unsafe fn apply_to_config(config: id) {
        if config == nil {
            return;
        }

        let ucc: id = msg_send![config, userContentController];
        let source = build_hint_script();
        let src_ns: id = NSString::alloc(nil).init_str(&source);

        const AT_DOCUMENT_END: usize = 1;
        let script: id = msg_send![class!(WKUserScript), alloc];
        let script: id = msg_send![
            script,
            initWithSource: src_ns
            injectionTime: AT_DOCUMENT_END
            forMainFrameOnly: true
        ];
        if script != nil {
            let _: () = msg_send![ucc, addUserScript: script];
        }
        log::debug!("resource_hints: preconnect script injected");
    }
}

#[cfg(target_os = "macos")]
pub use macos::apply_to_config;
