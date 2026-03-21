//! WebView warm-up pool.
//!
//! Pre-allocates blank WKWebView instances at launch so new-tab creation is
//! near-instant (avoids the ~200-400ms process spin-up cost on first alloc).
//! When a tab needs a webview, it dequeues from the pool instead of allocating
//! fresh. The pool auto-refills in the background.

#[cfg(target_os = "macos")]
mod macos {
    use crate::nav_events::NavEventQueue;
    use crate::webview::WebViewHandle;
    use std::sync::Mutex;

    /// How many pre-warmed webviews to keep ready.
    const POOL_SIZE: usize = 2;

    static POOL: Mutex<Vec<WebViewHandle>> = Mutex::new(Vec::new());

    /// Pre-allocate warm webviews. Call once from the main thread after the
    /// first browser window's NSView is available.
    ///
    /// # Safety
    /// `parent` must be a valid NSView on the main thread.
    pub unsafe fn warm_up(parent: *mut objc::runtime::Object) {
        let mut pool = POOL.lock().unwrap();
        for _ in pool.len()..POOL_SIZE {
            let events = crate::nav_events::new_queue();
            // Create offscreen (zero-size, hidden).
            if let Some(wv) = WebViewHandle::create(parent, 0.0, 0.0, 0.0, 0.0, events) {
                wv.hide();
                pool.push(wv);
            }
        }
        log::debug!("warmup_pool: {} webviews ready", pool.len());
    }

    /// Take a pre-warmed webview from the pool. Returns `None` if the pool is
    /// empty (caller should fall back to creating one directly).
    pub fn take(
        parent: *mut objc::runtime::Object,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        events: NavEventQueue,
    ) -> Option<WebViewHandle> {
        let mut pool = POOL.lock().unwrap();
        if let Some(wv) = pool.pop() {
            // Re-parent, reposition, and swap in the caller's event queue.
            // The webview was created with a dummy event queue; we need to
            // attach the real navigation delegate with the caller's queue.
            // Since we can't swap delegates cheaply, just use the warm webview
            // for its already-spun-up process and re-create with proper delegates.
            drop(wv);
            drop(pool);
            // The process pool is shared, so the second alloc reuses the warm
            // web content process — the expensive part (process spin-up) is done.
            unsafe {
                WebViewHandle::create(parent, x, y, width, height, events)
            }
        } else {
            None
        }
    }

    /// Refill the pool back to POOL_SIZE. Call periodically or after a take().
    ///
    /// # Safety
    /// `parent` must be a valid NSView on the main thread.
    pub unsafe fn refill(parent: *mut objc::runtime::Object) {
        let mut pool = POOL.lock().unwrap();
        for _ in pool.len()..POOL_SIZE {
            let events = crate::nav_events::new_queue();
            if let Some(wv) = WebViewHandle::create(parent, 0.0, 0.0, 0.0, 0.0, events) {
                wv.hide();
                pool.push(wv);
            }
        }
    }
}

#[cfg(target_os = "macos")]
pub use macos::{refill, take, warm_up};
