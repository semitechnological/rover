#[cfg(target_os = "macos")]
mod macos {
    use cocoa::base::{id, nil};
    use objc::*;
    use std::sync::OnceLock;

    struct SendId(id);
    unsafe impl Send for SendId {}
    unsafe impl Sync for SendId {}

    static PROCESS_POOL: OnceLock<SendId> = OnceLock::new();

    /// Returns the shared WKProcessPool for all browser tabs.
    /// Sharing a pool allows cross-tab cookie/session sharing.
    pub unsafe fn shared_process_pool() -> id {
        PROCESS_POOL
            .get_or_init(|| {
                let pool: id = msg_send![class!(WKProcessPool), new];
                SendId(pool)
            })
            .0
    }

    /// Returns WKWebsiteDataStore.defaultDataStore — persists cookies,
    /// localStorage, and HTTP cache across app restarts.
    pub unsafe fn default_data_store() -> id {
        msg_send![class!(WKWebsiteDataStore), defaultDataStore]
    }
}

#[cfg(target_os = "macos")]
pub use macos::{default_data_store, shared_process_pool};
