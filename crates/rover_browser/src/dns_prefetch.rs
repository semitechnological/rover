//! Background DNS prefetching for likely navigation targets.
//!
//! Resolves hostnames ahead of time so the system DNS cache is warm when the
//! user actually navigates. Since WKWebView shares the system DNS cache, these
//! pre-resolved entries benefit subsequent page loads.

use std::collections::HashSet;
use std::sync::Mutex;

/// Hostnames we've already prefetched in this session (avoid duplicate work).
static RESOLVED: Mutex<Option<HashSet<String>>> = Mutex::new(None);

/// Prefetch DNS for a hostname on a background thread.
/// No-op if already resolved in this session.
pub fn prefetch(hostname: &str) {
    let hostname = hostname.to_string();

    let mut resolved = RESOLVED.lock().unwrap();
    let set = resolved.get_or_insert_with(HashSet::new);
    if !set.insert(hostname.clone()) {
        return; // already resolved
    }
    drop(resolved);

    std::thread::spawn(move || {
        // getaddrinfo populates the system DNS cache.
        use std::net::ToSocketAddrs;
        let addr = format!("{hostname}:443");
        match addr.to_socket_addrs() {
            Ok(_) => log::debug!("dns_prefetch: resolved {hostname}"),
            Err(e) => log::debug!("dns_prefetch: failed {hostname}: {e}"),
        }
    });
}

/// Prefetch DNS for all hostnames extracted from a URL.
pub fn prefetch_url(url: &str) {
    if let Some(host) = extract_host(url) {
        prefetch(&host);
    }
}

/// Extract hostname from a URL string.
fn extract_host(url: &str) -> Option<String> {
    let stripped = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let host = stripped.split('/').next()?;
    let host = host.split(':').next()?; // strip port
    if host.is_empty() {
        None
    } else {
        Some(host.to_string())
    }
}

/// Common CDN/service hostnames to pre-resolve at startup.
const COMMON_HOSTS: &[&str] = &[
    "fonts.googleapis.com",
    "fonts.gstatic.com",
    "cdn.jsdelivr.net",
    "cdnjs.cloudflare.com",
    "unpkg.com",
    "ajax.googleapis.com",
];

/// Pre-resolve common CDN hostnames. Call once at startup.
pub fn prefetch_common() {
    for host in COMMON_HOSTS {
        prefetch(host);
    }
}
