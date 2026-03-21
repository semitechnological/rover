//! WKDownloadDelegate + plain-Rust DownloadManager for Rover.
//!
//! The ObjC delegate saves downloads to ~/Downloads/<suggestedFilename>
//! and pushes NavEvent::DownloadStarted when complete.

#[cfg(target_os = "macos")]
mod macos {
    use crate::nav_events::{NavEvent, NavEventQueue};
    use objc2::define_class;
    use objc2::rc::Retained;
    use objc2::runtime::NSObjectProtocol;
    use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly};
    use objc2_foundation::{NSData, NSError, NSObject, NSString, NSURL, NSURLResponse};
    use objc2_web_kit::{WKDownload, WKDownloadDelegate};

    // ── Ivars ─────────────────────────────────────────────────────────────────

    pub struct DownloadDelegateIvars {
        pub events: NavEventQueue,
    }

    // ── Class definition ──────────────────────────────────────────────────────

    define_class!(
        // SAFETY: NSObject superclass has no subclassing requirements.
        // We do not implement Drop.
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[ivars = DownloadDelegateIvars]
        pub struct RoverDownloadDelegate;

        unsafe impl NSObjectProtocol for RoverDownloadDelegate {}

        unsafe impl WKDownloadDelegate for RoverDownloadDelegate {
            /// Required: pick the destination path for the download.
            /// We place files in ~/Downloads/<suggestedFilename>.
            #[unsafe(method(download:decideDestinationUsingResponse:suggestedFilename:completionHandler:))]
            unsafe fn decide_destination(
                &self,
                _download: &WKDownload,
                _response: &NSURLResponse,
                suggested_filename: &NSString,
                completion_handler: &block2::DynBlock<dyn Fn(*mut NSURL)>,
            ) {
                let filename = suggested_filename.to_string();
                let destination: Option<std::path::PathBuf> = dirs::download_dir()
                    .map(|d| d.join(&filename));

                if let Some(dest) = destination {
                    // Ensure ~/Downloads exists.
                    if let Some(parent) = dest.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    let dest_str = dest.to_string_lossy();
                    // Build file:// NSURL for the destination.
                    use cocoa::base::nil;
                    use cocoa::foundation::NSString as CocoaNSString;
                    use objc::msg_send as omsg;
                    use objc::*;
                    let ns_path: cocoa::base::id =
                        CocoaNSString::alloc(nil).init_str(&dest_str);
                    let ns_url: cocoa::base::id =
                        omsg![class!(NSURL), fileURLWithPath: ns_path];
                    let typed_url = ns_url as *mut NSURL;
                    completion_handler.call((typed_url,));
                } else {
                    completion_handler.call((std::ptr::null_mut(),));
                }
            }

            /// Called when the download finishes successfully.
            #[unsafe(method(downloadDidFinish:))]
            unsafe fn download_did_finish(&self, download: &WKDownload) {
                // Try to get the filename from the original request URL.
                let filename: String = {
                    let mut name = String::from("download");
                    if let Some(req) = download.originalRequest() {
                        if let Some(url) = req.URL() {
                            if let Some(component) = url.lastPathComponent() {
                                let s = component.to_string();
                                if !s.is_empty() {
                                    name = s;
                                }
                            }
                        }
                    }
                    name
                };

                let destination = dirs::download_dir()
                    .map(|d| d.join(&filename).to_string_lossy().to_string())
                    .unwrap_or_else(|| format!("~/Downloads/{filename}"));

                self.ivars()
                    .events
                    .lock()
                    .unwrap()
                    .push(NavEvent::DownloadStarted {
                        suggested_filename: filename,
                        destination,
                    });
            }

            /// Called when a download fails.
            #[unsafe(method(download:didFailWithError:resumeData:))]
            unsafe fn download_did_fail(
                &self,
                _download: &WKDownload,
                error: &NSError,
                _resume_data: Option<&NSData>,
            ) {
                let desc = error.localizedDescription().to_string();
                log::error!("download_manager: {desc}");
            }
        }
    );

    impl RoverDownloadDelegate {
        /// Create a new download delegate with the given event queue.
        pub fn new(events: NavEventQueue, mtm: MainThreadMarker) -> Retained<Self> {
            let this = Self::alloc(mtm).set_ivars(DownloadDelegateIvars { events });
            unsafe { objc2::msg_send![super(this), init] }
        }
    }
}

#[cfg(target_os = "macos")]
pub use macos::RoverDownloadDelegate;

// ─── Plain-Rust DownloadManager ────────────────────────────────────────────

/// Tracks in-progress downloads by filename.
pub struct DownloadManager {
    in_progress: std::collections::HashMap<String, String>,
}

impl DownloadManager {
    pub fn new() -> Self {
        Self {
            in_progress: std::collections::HashMap::new(),
        }
    }

    pub fn start(&mut self, filename: &str, destination: &str) {
        self.in_progress
            .insert(filename.to_string(), destination.to_string());
    }

    pub fn finish(&mut self, filename: &str) -> Option<String> {
        self.in_progress.remove(filename)
    }

    pub fn in_progress(&self) -> &std::collections::HashMap<String, String> {
        &self.in_progress
    }
}

impl Default for DownloadManager {
    fn default() -> Self {
        Self::new()
    }
}
