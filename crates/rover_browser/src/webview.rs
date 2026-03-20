#[cfg(target_os = "macos")]
mod macos {
    use cocoa::base::{id, nil, YES};
    use cocoa::foundation::{NSPoint, NSRect, NSSize, NSString};
    use objc::runtime::Object;
    use objc::*;

    pub struct WebViewHandle {
        raw: id,
    }

    unsafe impl Send for WebViewHandle {}

    impl WebViewHandle {
        pub fn create(
            parent: *mut Object,
            x: f64,
            y: f64,
            width: f64,
            height: f64,
        ) -> Option<Self> {
            unsafe {
                let config: id = msg_send![class!(WKWebViewConfiguration), new];
                let frame = NSRect::new(NSPoint::new(x, y), NSSize::new(width, height));
                let webview: id = msg_send![class!(WKWebView), alloc];
                let webview: id =
                    msg_send![webview, initWithFrame:frame configuration:config];
                if webview == nil {
                    return None;
                }
                let _: () = msg_send![parent, addSubview: webview];
                Some(Self { raw: webview })
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
