//! WKUIDelegate implementation for Rover.
//!
//! Handles JavaScript dialog panels (alert, confirm, prompt), new-window
//! requests, and media capture permission for the embedded WKWebView.

#[cfg(target_os = "macos")]
mod macos {
    use crate::nav_events::{NavEvent, NavEventQueue};
    use objc2::define_class;
    use objc2::rc::Retained;
    use objc2::runtime::NSObjectProtocol;
    use objc2::{msg_send, DefinedClass, MainThreadMarker, MainThreadOnly};
    use objc2_app_kit::{NSAlert, NSAlertFirstButtonReturn, NSTextField};
    use objc2_foundation::{ns_string, NSObject, NSRect, NSString};
    use objc2_web_kit::{
        WKFrameInfo, WKMediaCaptureType, WKNavigationAction, WKPermissionDecision,
        WKSecurityOrigin, WKUIDelegate, WKWebView, WKWebViewConfiguration, WKWindowFeatures,
    };

    // ── Ivars ─────────────────────────────────────────────────────────────────

    pub struct UiDelegateIvars {
        pub events: NavEventQueue,
    }

    // ── Class definition ──────────────────────────────────────────────────────

    define_class!(
        // SAFETY: NSObject superclass has no subclassing requirements.
        // We do not implement Drop.
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[ivars = UiDelegateIvars]
        pub struct RoverUIDelegate;

        unsafe impl NSObjectProtocol for RoverUIDelegate {}

        unsafe impl WKUIDelegate for RoverUIDelegate {
            /// Called when JavaScript opens a popup window. We push a
            /// NewTabRequested event so BrowserWindow can open a new tab,
            /// and return nil so WebKit cancels its own window creation.
            #[unsafe(method_id(webView:createWebViewWithConfiguration:forNavigationAction:windowFeatures:))]
            unsafe fn create_web_view(
                &self,
                _web_view: &WKWebView,
                _configuration: &WKWebViewConfiguration,
                navigation_action: &WKNavigationAction,
                _window_features: &WKWindowFeatures,
            ) -> Option<Retained<WKWebView>> {
                // Extract the requested URL from the navigation action.
                let url_str: String = {
                    let req_url_opt = navigation_action.request().URL();
                    match req_url_opt {
                        Some(u) => match u.absoluteString() {
                            Some(s) => s.to_string(),
                            None => String::new(),
                        },
                        None => String::new(),
                    }
                };

                if !url_str.is_empty() {
                    self.ivars()
                        .events
                        .lock()
                        .unwrap()
                        .push(NavEvent::NewTabRequested { url: url_str });
                }
                // Return nil — BrowserWindow handles opening the tab.
                None
            }

            /// Show an NSAlert for JavaScript alert() calls.
            #[unsafe(method(webView:runJavaScriptAlertPanelWithMessage:initiatedByFrame:completionHandler:))]
            unsafe fn run_js_alert(
                &self,
                _web_view: &WKWebView,
                message: &NSString,
                _frame: &WKFrameInfo,
                completion_handler: &block2::DynBlock<dyn Fn()>,
            ) {
                let mtm = MainThreadMarker::new_unchecked();
                let alert = NSAlert::new(mtm);
                alert.setMessageText(message);
                alert.addButtonWithTitle(ns_string!("OK"));
                alert.runModal();
                completion_handler.call(());
            }

            /// Show an NSAlert with OK/Cancel for JavaScript confirm() calls.
            #[unsafe(method(webView:runJavaScriptConfirmPanelWithMessage:initiatedByFrame:completionHandler:))]
            unsafe fn run_js_confirm(
                &self,
                _web_view: &WKWebView,
                message: &NSString,
                _frame: &WKFrameInfo,
                completion_handler: &block2::DynBlock<dyn Fn(objc2::runtime::Bool)>,
            ) {
                let mtm = MainThreadMarker::new_unchecked();
                let alert = NSAlert::new(mtm);
                alert.setMessageText(message);
                alert.addButtonWithTitle(ns_string!("OK"));
                alert.addButtonWithTitle(ns_string!("Cancel"));
                let response = alert.runModal();
                let confirmed = response == NSAlertFirstButtonReturn;
                completion_handler.call((objc2::runtime::Bool::new(confirmed),));
            }

            /// Show an NSAlert with a text field for JavaScript prompt() calls.
            #[unsafe(method(webView:runJavaScriptTextInputPanelWithPrompt:defaultText:initiatedByFrame:completionHandler:))]
            unsafe fn run_js_prompt(
                &self,
                _web_view: &WKWebView,
                prompt: &NSString,
                default_text: Option<&NSString>,
                _frame: &WKFrameInfo,
                completion_handler: &block2::DynBlock<dyn Fn(*mut NSString)>,
            ) {
                let mtm = MainThreadMarker::new_unchecked();
                let alert = NSAlert::new(mtm);
                alert.setMessageText(prompt);
                alert.addButtonWithTitle(ns_string!("OK"));
                alert.addButtonWithTitle(ns_string!("Cancel"));

                // Create a text field as the accessory view.
                let frame = NSRect::new(
                    objc2_foundation::NSPoint::new(0.0, 0.0),
                    objc2_foundation::NSSize::new(300.0, 22.0),
                );
                let text_field = NSTextField::initWithFrame(NSTextField::alloc(mtm), frame);
                if let Some(default) = default_text {
                    let _: () = msg_send![&*text_field, setStringValue: default];
                }
                // setAccessoryView: requires the NSView feature
                let _: () = msg_send![&*alert, setAccessoryView: &*text_field];

                let response = alert.runModal();
                if response == NSAlertFirstButtonReturn {
                    // Return the text field's content.
                    let value: *mut NSString = msg_send![&*text_field, stringValue];
                    completion_handler.call((value,));
                } else {
                    completion_handler.call((std::ptr::null_mut(),));
                }
            }

            /// Grant media capture (camera/microphone) permission automatically.
            /// This is a permissive policy — a production app would prompt the user.
            #[unsafe(method(webView:requestMediaCapturePermissionForOrigin:initiatedByFrame:type:decisionHandler:))]
            unsafe fn request_media_capture_permission(
                &self,
                _web_view: &WKWebView,
                _origin: &WKSecurityOrigin,
                _frame: &WKFrameInfo,
                _capture_type: WKMediaCaptureType,
                decision_handler: &block2::DynBlock<dyn Fn(WKPermissionDecision)>,
            ) {
                decision_handler.call((WKPermissionDecision::Grant,));
            }
        }
    );

    impl RoverUIDelegate {
        /// Create a new UI delegate with the given event queue.
        pub fn new(events: NavEventQueue, mtm: MainThreadMarker) -> Retained<Self> {
            let this = Self::alloc(mtm).set_ivars(UiDelegateIvars { events });
            unsafe { objc2::msg_send![super(this), init] }
        }
    }
}

#[cfg(target_os = "macos")]
pub use macos::RoverUIDelegate;
