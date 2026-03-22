#[cfg(target_os = "macos")]
mod macos {
    use crate::nav_events::NavEventQueue;
    use cocoa::base::{id, nil, YES};
    use cocoa::foundation::{NSPoint, NSRect, NSSize, NSString};
    use objc::runtime::Object;
    use objc::*;

    /// A wrapper that forces a `Retained<T>` to be `Send+Sync` by lying about it.
    /// Safety: All ObjC delegate calls happen on the main thread. We hold these
    /// alive purely for their retain count; we never send them to another thread.
    struct SendableRetained<T: objc2::Message>(objc2::rc::Retained<T>);
    unsafe impl<T: objc2::Message> Send for SendableRetained<T> {}
    unsafe impl<T: objc2::Message> Sync for SendableRetained<T> {}

    pub struct WebViewHandle {
        pub(crate) raw: id,
        #[allow(dead_code)]
        _nav_delegate: Box<dyn std::any::Any + Send + Sync>,
        #[allow(dead_code)]
        _ui_delegate: Box<dyn std::any::Any + Send + Sync>,
    }

    unsafe impl Send for WebViewHandle {}

    impl WebViewHandle {
        pub fn create(
            parent: *mut Object,
            x: f64,
            y: f64,
            width: f64,
            height: f64,
            events: NavEventQueue,
        ) -> Option<Self> {
            log::info!("WebViewHandle::create: start ({x:.0},{y:.0},{width:.0},{height:.0})");
            unsafe {
                let config: id = msg_send![class!(WKWebViewConfiguration), new];
                log::info!("WebViewHandle::create: got WKWebViewConfiguration");

                // Shared process pool (cross-tab session sharing).
                let pool = crate::shared_config::shared_process_pool();
                let _: () = msg_send![config, setProcessPool: pool];

                // Persistent data store (cookies, localStorage, cache).
                let ds = crate::shared_config::default_data_store();
                let _: () = msg_send![config, setWebsiteDataStore: ds];

                // Load any installed WKWebExtensions.
                if let Some(controller) = crate::extensions::load_extensions() {
                    let _: () = msg_send![config, setWebExtensionController: controller];
                }

                log::info!("WebViewHandle::create: applying content blocker");
                // Apply uBOL-derived content blocking rules to this configuration.
                crate::content_blocker::apply_to_config(config);
                log::info!("WebViewHandle::create: content blocker applied");

                log::info!("WebViewHandle::create: applying resource hints");
                // Inject preconnect/dns-prefetch hints for common CDN origins.
                crate::resource_hints::apply_to_config(config);
                log::info!("WebViewHandle::create: resource hints applied");

                let frame = NSRect::new(NSPoint::new(x, y), NSSize::new(width, height));
                log::info!("WebViewHandle::create: alloc+init WKWebView");
                let webview: id = msg_send![class!(WKWebView), alloc];
                let webview: id =
                    msg_send![webview, initWithFrame:frame configuration:config];
                if webview == nil {
                    log::error!("WebViewHandle::create: WKWebView init returned nil!");
                    return None;
                }
                log::info!("WebViewHandle::create: adding subview to parent NSView");
                let _: () = msg_send![parent, addSubview: webview];
                log::info!("WebViewHandle::create: subview added");

                // Create and attach the navigation delegate (objc2 world).
                use objc2::MainThreadMarker;
                let mtm = MainThreadMarker::new_unchecked();

                let nav_delegate =
                    crate::navigation_delegate::RoverNavDelegate::new(events.clone(), mtm);
                // Bridge Retained<> pointer into the objc 0.2 world for setNavigationDelegate:.
                let nav_ptr: *const _ = objc2::rc::Retained::as_ptr(&nav_delegate);
                let _: () = msg_send![webview, setNavigationDelegate: nav_ptr];

                // Create and attach the UI delegate.
                let ui_delegate =
                    crate::ui_delegate::RoverUIDelegate::new(events.clone(), mtm);
                let ui_ptr: *const _ = objc2::rc::Retained::as_ptr(&ui_delegate);
                let _: () = msg_send![webview, setUIDelegate: ui_ptr];

                log::info!("WebViewHandle::create: success");
                Some(Self {
                    raw: webview,
                    _nav_delegate: Box::new(SendableRetained(nav_delegate)),
                    _ui_delegate: Box::new(SendableRetained(ui_delegate)),
                })
            }
        }

        pub fn navigate(&self, url: &str) {
            unsafe {
                let ns_url_class = class!(NSURL);
                let url_string: id =
                    NSString::alloc(nil).init_str(url);
                let ns_url: id = msg_send![ns_url_class, URLWithString: url_string];
                if ns_url == nil {
                    return;
                }
                let request: id = msg_send![class!(NSURLRequest), requestWithURL: ns_url];
                let _: id = msg_send![self.raw, loadRequest: request];
            }
        }

        pub fn go_back(&self) {
            unsafe {
                let can_go_back: bool = msg_send![self.raw, canGoBack];
                if can_go_back {
                    let _: id = msg_send![self.raw, goBack];
                }
            }
        }

        pub fn go_forward(&self) {
            unsafe {
                let can_go_forward: bool = msg_send![self.raw, canGoForward];
                if can_go_forward {
                    let _: id = msg_send![self.raw, goForward];
                }
            }
        }

        pub fn reload(&self) {
            unsafe {
                let _: id = msg_send![self.raw, reload];
            }
        }

        pub fn set_frame(&self, x: f64, y: f64, width: f64, height: f64) {
            unsafe {
                let frame = NSRect::new(NSPoint::new(x, y), NSSize::new(width, height));
                let _: () = msg_send![self.raw, setFrame: frame];
            }
        }

        pub fn show(&self) {
            unsafe {
                let _: () = msg_send![self.raw, setHidden: false];
            }
        }

        pub fn hide(&self) {
            unsafe {
                let _: () = msg_send![self.raw, setHidden: YES];
            }
        }

        pub fn title(&self) -> Option<String> {
            unsafe {
                let title: id = msg_send![self.raw, title];
                if title == nil {
                    return None;
                }
                let utf8: *const std::os::raw::c_char = msg_send![title, UTF8String];
                if utf8.is_null() {
                    return None;
                }
                std::ffi::CStr::from_ptr(utf8)
                    .to_str()
                    .ok()
                    .map(|s| s.to_string())
            }
        }

        pub fn url(&self) -> Option<String> {
            unsafe {
                let url: id = msg_send![self.raw, URL];
                if url == nil {
                    return None;
                }
                let absolute_string: id = msg_send![url, absoluteString];
                if absolute_string == nil {
                    return None;
                }
                let utf8: *const std::os::raw::c_char =
                    msg_send![absolute_string, UTF8String];
                if utf8.is_null() {
                    return None;
                }
                std::ffi::CStr::from_ptr(utf8)
                    .to_str()
                    .ok()
                    .map(|s| s.to_string())
            }
        }

        /// Invoke WKWebView's find-in-page with the given query.
        /// Calls `_findString:options:maxCount:` (private API fallback) or
        /// the public `findString:withConfiguration:completionHandler:` on macOS 16+.
        /// We use the older raw msg_send! approach for maximum compatibility.
        pub fn find_text(&self, query: &str) {
            unsafe {
                let ns_string: id = NSString::alloc(nil).init_str(query);
                // WKFindConfiguration (public API, macOS 13+)
                let cfg: id = msg_send![class!(WKFindConfiguration), new];
                // findString:withConfiguration:completionHandler: — no-op block
                use block::ConcreteBlock;
                let done_block = ConcreteBlock::new(|_result: id| {}).copy();
                let _: () = msg_send![self.raw,
                    findString: ns_string
                    withConfiguration: cfg
                    completionHandler: &*done_block
                ];
            }
        }

        /// Evaluates JavaScript in the web view and calls the callback with the result string.
        /// The callback fires on the main thread.
        pub fn evaluate_javascript<F>(&self, script: &str, callback: F)
        where
            F: FnOnce(Option<String>) + Send + 'static,
        {
            unsafe {
                use block::ConcreteBlock;
                use std::sync::Mutex;
                let ns_script: id = NSString::alloc(nil).init_str(script);

                // Wrap FnOnce in Mutex<Option<>> so the block is Fn-compatible
                let cb = Mutex::new(Some(callback));
                let completion = ConcreteBlock::new(move |result: id, error: id| {
                    let cb = cb.lock().unwrap().take();
                    let Some(cb) = cb else { return };
                    if error != nil || result == nil {
                        cb(None);
                        return;
                    }
                    let desc: id = msg_send![result, description];
                    if desc == nil {
                        cb(None);
                        return;
                    }
                    let utf8: *const std::os::raw::c_char = msg_send![desc, UTF8String];
                    if utf8.is_null() {
                        cb(None);
                        return;
                    }
                    let s = std::ffi::CStr::from_ptr(utf8)
                        .to_str()
                        .ok()
                        .map(|s| s.to_string());
                    cb(s);
                });
                let completion = completion.copy();

                let _: () = msg_send![self.raw,
                    evaluateJavaScript: ns_script
                    completionHandler: &*completion
                ];
            }
        }

        /// Suspend all media playback (audio/video) in this webview.
        /// Call when the tab goes to background to save CPU/battery.
        pub fn suspend_media(&self) {
            unsafe {
                let _: () = msg_send![self.raw, setAllMediaPlaybackSuspended: true];
            }
        }

        /// Resume media playback when the tab becomes active again.
        pub fn resume_media(&self) {
            unsafe {
                let _: () = msg_send![self.raw, setAllMediaPlaybackSuspended: false];
            }
        }

        pub fn remove_from_superview(&self) {
            unsafe {
                let _: () = msg_send![self.raw, removeFromSuperview];
            }
        }
    }

    impl Drop for WebViewHandle {
        fn drop(&mut self) {
            self.remove_from_superview();
        }
    }
}

#[cfg(target_os = "macos")]
pub use macos::WebViewHandle;
