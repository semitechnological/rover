//! WKNavigationDelegate implementation for Rover.
//!
//! Bridges navigation events from WKWebView into the NavEventQueue,
//! which BrowserWindow polls to update its GPUI state.

#[cfg(target_os = "macos")]
mod macos {
    use crate::nav_events::{NavEvent, NavEventQueue};
    use objc2::define_class;
    use objc2::rc::Retained;
    use objc2::runtime::NSObjectProtocol;
    use objc2::{DefinedClass, MainThreadOnly, MainThreadMarker};
    use objc2_foundation::{NSError, NSObject, NSString, NSURL};
    use objc2_web_kit::{
        WKNavigation, WKNavigationAction, WKNavigationActionPolicy, WKNavigationDelegate,
        WKNavigationResponse, WKNavigationResponsePolicy, WKWebView,
    };

    // ── Ivars ─────────────────────────────────────────────────────────────────

    pub struct NavDelegateIvars {
        pub events: NavEventQueue,
    }

    // ── Class definition ──────────────────────────────────────────────────────

    define_class!(
        // SAFETY: NSObject superclass has no subclassing requirements.
        // We do not implement Drop.
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[ivars = NavDelegateIvars]
        pub struct RoverNavDelegate;

        unsafe impl NSObjectProtocol for RoverNavDelegate {}

        unsafe impl WKNavigationDelegate for RoverNavDelegate {
            // Fired when the webview begins loading a provisional navigation
            // (before any response is received).
            #[unsafe(method(webView:didStartProvisionalNavigation:))]
            unsafe fn webview_did_start_provisional_navigation(
                &self,
                web_view: &WKWebView,
                _nav: Option<&WKNavigation>,
            ) {
                let url = url_from_webview(web_view);
                self.ivars()
                    .events
                    .lock()
                    .unwrap()
                    .push(NavEvent::LoadStarted { url });
            }

            // Fired when content starts arriving for the main frame.
            #[unsafe(method(webView:didCommitNavigation:))]
            unsafe fn webview_did_commit_navigation(
                &self,
                web_view: &WKWebView,
                _nav: Option<&WKNavigation>,
            ) {
                let url = url_from_webview(web_view);
                self.ivars()
                    .events
                    .lock()
                    .unwrap()
                    .push(NavEvent::LoadCommitted { url });
            }

            // Fired when a main frame navigation completes successfully.
            #[unsafe(method(webView:didFinishNavigation:))]
            unsafe fn webview_did_finish_navigation(
                &self,
                web_view: &WKWebView,
                _nav: Option<&WKNavigation>,
            ) {
                let url = url_from_webview(web_view);
                let title = title_from_webview(web_view);
                self.ivars()
                    .events
                    .lock()
                    .unwrap()
                    .push(NavEvent::LoadFinished { url, title });
            }

            // Fired when a committed navigation fails.
            #[unsafe(method(webView:didFailNavigation:withError:))]
            unsafe fn webview_did_fail_navigation(
                &self,
                _web_view: &WKWebView,
                _nav: Option<&WKNavigation>,
                error: &NSError,
            ) {
                let desc = error.localizedDescription().to_string();
                self.ivars()
                    .events
                    .lock()
                    .unwrap()
                    .push(NavEvent::LoadFailed { error: desc });
            }

            // Fired when the provisional navigation fails (e.g. DNS error).
            #[unsafe(method(webView:didFailProvisionalNavigation:withError:))]
            unsafe fn webview_did_fail_provisional_navigation(
                &self,
                _web_view: &WKWebView,
                _nav: Option<&WKNavigation>,
                error: &NSError,
            ) {
                let desc = error.localizedDescription().to_string();
                self.ivars()
                    .events
                    .lock()
                    .unwrap()
                    .push(NavEvent::LoadFailed { error: desc });
            }

            // Fired when the web content process crashes or is terminated by the OS.
            // We push a ProcessTerminated event so the tab auto-reloads.
            #[unsafe(method(webViewWebContentProcessDidTerminate:))]
            unsafe fn webview_web_content_process_did_terminate(
                &self,
                web_view: &WKWebView,
            ) {
                let url = url_from_webview(web_view);
                log::warn!("content process terminated for {url}");
                self.ivars()
                    .events
                    .lock()
                    .unwrap()
                    .push(NavEvent::ProcessTerminated);
            }

            // Decide whether to allow or cancel a navigation action.
            // We allow all normal navigations; mailto: and tel: links are
            // handled by the OS via the "allow" path (WKWebView forwards
            // unrenderable schemes to NSWorkspace automatically on macOS).
            #[unsafe(method(webView:decidePolicyForNavigationAction:decisionHandler:))]
            unsafe fn webview_decide_policy_for_navigation_action(
                &self,
                _web_view: &WKWebView,
                _navigation_action: &WKNavigationAction,
                decision_handler: &block2::DynBlock<dyn Fn(WKNavigationActionPolicy)>,
            ) {
                decision_handler.call((WKNavigationActionPolicy::Allow,));
            }

            // Decide whether to allow or download a response.
            // If the server signals an attachment (Content-Disposition: attachment),
            // WebKit will set `canShowMIMEType` to false; we forward to download.
            #[unsafe(method(webView:decidePolicyForNavigationResponse:decisionHandler:))]
            unsafe fn webview_decide_policy_for_navigation_response(
                &self,
                _web_view: &WKWebView,
                navigation_response: &WKNavigationResponse,
                decision_handler: &block2::DynBlock<dyn Fn(WKNavigationResponsePolicy)>,
            ) {
                if navigation_response.canShowMIMEType() {
                    decision_handler.call((WKNavigationResponsePolicy::Allow,));
                } else {
                    decision_handler.call((WKNavigationResponsePolicy::Download,));
                }
            }
        }
    );

    impl RoverNavDelegate {
        /// Create a new navigation delegate with the given event queue.
        pub fn new(events: NavEventQueue, mtm: MainThreadMarker) -> Retained<Self> {
            let this = Self::alloc(mtm).set_ivars(NavDelegateIvars { events });
            unsafe { objc2::msg_send![super(this), init] }
        }
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    unsafe fn url_from_webview(wv: &WKWebView) -> String {
        match wv.URL() {
            Some(u) => match u.absoluteString() {
                Some(s) => s.to_string(),
                None => String::new(),
            },
            None => String::new(),
        }
    }

    unsafe fn title_from_webview(wv: &WKWebView) -> String {
        match wv.title() {
            Some(s) => s.to_string(),
            None => String::new(),
        }
    }
}

#[cfg(target_os = "macos")]
pub use macos::RoverNavDelegate;
