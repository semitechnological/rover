#![recursion_limit = "4096"]

mod webview;
#[cfg(target_os = "macos")]
mod content_blocker;
mod nav_events;
mod shared_config;
#[cfg(target_os = "macos")]
mod navigation_delegate;
#[cfg(target_os = "macos")]
mod ui_delegate;
#[cfg(target_os = "macos")]
mod download_manager;
mod extensions;
mod history;
mod bookmarks;
mod dns_prefetch;
#[cfg(target_os = "macos")]
mod resource_hints;
#[cfg(target_os = "macos")]
mod warmup_pool;
pub mod transport;

use gpui::*;
use std::sync::Arc;
use std::time::{Duration, Instant};

use nav_events::{NavEvent, NavEventQueue};
use history::HistoryManager;
use bookmarks::BookmarkManager;

#[cfg(target_os = "macos")]
use webview::WebViewHandle;

// ─── Constants ────────────────────────────────────────────────────────────────

pub const DEFAULT_URL: &str = "https://www.perplexity.ai";
const FALLBACK_ENGINE: &str = "perplexity";
pub const CACHED_TIMEOUT: Duration = Duration::from_secs(5 * 60);
pub const FROZEN_TIMEOUT: Duration = Duration::from_secs(30 * 60);
pub const CLOSED_TIMEOUT: Duration = Duration::from_secs(2 * 60 * 60);
const LIFECYCLE_INTERVAL: Duration = Duration::from_secs(30);
const EVENT_POLL_INTERVAL: Duration = Duration::from_millis(500);

actions!(rover_browser, [OpenTransport, ShareCurrentUrl]);

/// Shared live browser tab state so the picker can show context about open tabs.
/// Also contains a URL inbox for navigation requests from the picker.
pub struct LiveBrowserTabs {
    pub tabs: Vec<(String, String, TabState, u64)>, // (title, url, state, last_active_epoch_ms)
    pub active_tab: usize,
    /// URLs pushed by the picker for the browser to open as new tabs.
    pub url_inbox: Vec<String>,
    /// Tab indices to close (pushed by picker, consumed by browser).
    pub close_inbox: Vec<usize>,
}
impl Default for LiveBrowserTabs {
    fn default() -> Self {
        Self { tabs: Vec::new(), active_tab: 0, url_inbox: Vec::new(), close_inbox: Vec::new() }
    }
}

/// Thread-safe shared reference to live browser tab state.
pub type LiveBrowserTabsRef = Arc<std::sync::Mutex<LiveBrowserTabs>>;

// ─── Search engines ───────────────────────────────────────────────────────────

pub struct SearchEngine {
    pub name: &'static str,
    pub long_prefix: &'static str,
    pub short_prefix: Option<&'static str>,
    pub url_template: &'static str,
}

pub const SEARCH_ENGINES: &[SearchEngine] = &[
    SearchEngine {
        name: "google",
        long_prefix: "google",
        short_prefix: Some("g"),
        url_template: "https://www.google.com/search?q={query}",
    },
    SearchEngine {
        name: "youtube",
        long_prefix: "youtube",
        short_prefix: Some("yt"),
        url_template: "https://www.youtube.com/results?search_query={query}",
    },
    SearchEngine {
        name: "wikipedia",
        long_prefix: "wikipedia",
        short_prefix: Some("wiki"),
        url_template: "https://en.wikipedia.org/wiki/Special:Search?search={query}",
    },
    SearchEngine {
        name: "perplexity",
        long_prefix: "perplexity",
        short_prefix: Some("pp"),
        url_template: "https://www.perplexity.ai/search?q={query}",
    },
    SearchEngine {
        name: "github",
        long_prefix: "github",
        short_prefix: Some("gh"),
        url_template: "https://github.com/search?q={query}",
    },
    SearchEngine {
        name: "reddit",
        long_prefix: "reddit",
        short_prefix: Some("r"),
        url_template: "https://www.reddit.com/search?q={query}",
    },
    SearchEngine {
        name: "stackoverflow",
        long_prefix: "stackoverflow",
        short_prefix: Some("so"),
        url_template: "https://stackoverflow.com/search?q={query}",
    },
    SearchEngine {
        name: "duckduckgo",
        long_prefix: "duckduckgo",
        short_prefix: Some("ddg"),
        url_template: "https://duckduckgo.com/?q={query}",
    },
    SearchEngine {
        name: "amazon",
        long_prefix: "amazon",
        short_prefix: Some("amz"),
        url_template: "https://www.amazon.com/s?k={query}",
    },
    SearchEngine {
        name: "twitter",
        long_prefix: "twitter",
        short_prefix: Some("x"),
        url_template: "https://twitter.com/search?q={query}",
    },
    SearchEngine {
        name: "maps",
        long_prefix: "maps",
        short_prefix: None,
        url_template: "https://maps.apple.com/?q={query}",
    },
];

// ─── Types ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum TabState {
    Active,
    Cached,
    Frozen,
}

pub struct BrowserTab {
    pub title: String,
    pub url: String,
    pub state: TabState,
    pub last_active: Instant,
    pub events: NavEventQueue,
    #[cfg(target_os = "macos")]
    pub webview_handle: Option<WebViewHandle>,
}

#[derive(Debug, Clone)]
pub enum BrowserAction {
    Navigate(String),
    SwitchToTab(usize),
    CloseTab,
    NewTab,
    Back,
    Forward,
    Reload,
    ListTabs,
    NewSearch { engine_name: String, query: String },
    Find(String),
    AddBookmark,
    RemoveBookmark(i64),
    ShowHistory,
    SearchHistory(String),
    SearchBookmarks(String),
    Share,
}

// ─── Command parsing ──────────────────────────────────────────────────────────

pub fn parse_command(input: &str) -> BrowserAction {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return BrowserAction::Navigate("about:blank".to_string());
    }

    if trimmed.starts_with('/') {
        let query = &trimmed[1..];
        return BrowserAction::Find(query.to_string());
    }

    if trimmed.starts_with("h ") {
        let query = trimmed[2..].trim();
        return BrowserAction::SearchHistory(query.to_string());
    }

    if trimmed == "bk!" || trimmed.starts_with("bk!") {
        return BrowserAction::AddBookmark;
    }

    if trimmed.starts_with("bk ") {
        let query = trimmed[3..].trim();
        return BrowserAction::SearchBookmarks(query.to_string());
    }

    if trimmed == "history" {
        return BrowserAction::ShowHistory;
    }

    match trimmed {
        "back" | "<" => return BrowserAction::Back,
        "forward" | ">" => return BrowserAction::Forward,
        "reload" | "refresh" => return BrowserAction::Reload,
        "close" => return BrowserAction::CloseTab,
        "new tab" | "newtab" => return BrowserAction::NewTab,
        "tabs" | "ls" | "list" => return BrowserAction::ListTabs,
        "share" => return BrowserAction::Share,
        _ => {}
    }

    if let Some((prefix, query)) = trimmed.split_once(' ') {
        let prefix_lower = prefix.to_lowercase();
        for engine in SEARCH_ENGINES {
            if prefix_lower == engine.long_prefix
                || engine.short_prefix.is_some_and(|s| prefix_lower == s)
            {
                return BrowserAction::NewSearch {
                    engine_name: engine.name.to_string(),
                    query: query.to_string(),
                };
            }
        }
    }

    if looks_like_url(trimmed) {
        let url = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
            trimmed.to_string()
        } else {
            format!("https://{}", trimmed)
        };
        return BrowserAction::Navigate(url);
    }

    BrowserAction::NewSearch {
        engine_name: FALLBACK_ENGINE.to_string(),
        query: trimmed.to_string(),
    }
}

pub fn looks_like_url(input: &str) -> bool {
    if input.starts_with("http://") || input.starts_with("https://") {
        return true;
    }
    if input.starts_with("about:") || input.starts_with("file://") {
        return true;
    }
    if input.contains(' ') {
        return false;
    }
    let without_path = input.split('/').next().unwrap_or(input);
    let without_port = without_path.split(':').next().unwrap_or(without_path);
    if without_port.contains('.') {
        let parts: Vec<&str> = without_port.splitn(2, '.').collect();
        if parts.len() == 2 && !parts[1].is_empty() {
            return true;
        }
    }
    false
}

pub fn search_url(engine_name: &str, query: &str) -> String {
    let encoded = urlencoding::encode(query);
    for engine in SEARCH_ENGINES {
        if engine.name == engine_name {
            return engine.url_template.replace("{query}", &encoded);
        }
    }
    format!(
        "https://www.perplexity.ai/search?q={}",
        urlencoding::encode(query)
    )
}

pub fn fuzzy_score(query: &str, text: &str) -> f32 {
    if query.is_empty() {
        return 1.0;
    }
    let q = query.to_lowercase();
    let t = text.to_lowercase();

    if t.contains(&q) {
        let bonus = if t.starts_with(&q) { 1.0 } else { 0.5 };
        return bonus + (q.len() as f32 / t.len() as f32);
    }

    let mut score = 0.0f32;
    let mut qchars = q.chars().peekable();
    for ch in t.chars() {
        if let Some(&next) = qchars.peek() {
            if ch == next {
                qchars.next();
                score += 1.0;
            }
        }
    }
    if qchars.peek().is_some() {
        return 0.0;
    }
    score / q.len() as f32 * 0.3
}

/// Extracts the bare domain from a URL for display (e.g. "perplexity.ai").
pub fn domain_from_url(url: &str) -> &str {
    let stripped = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);
    stripped
        .split('/')
        .next()
        .unwrap_or(stripped)
        .trim_start_matches("www.")
}

// ─── BrowserWindow ────────────────────────────────────────────────────────────
//
// Pure WebView host — no embedded chrome. All UI lives in the floating transport.

pub struct BrowserWindow {
    pub tabs: Vec<BrowserTab>,
    pub active_tab: usize,
    /// Last ephemeral hint (download completion, search result, etc.).
    /// The transport reads and clears this.
    pub overlay_hint: Option<String>,
    pub history_manager: Option<HistoryManager>,
    pub bookmark_manager: Option<BookmarkManager>,
    pub semantic_index: Option<Arc<std::sync::Mutex<rover_semantic::SemanticIndex>>>,
    db: Option<Arc<rover_db::RoverDb>>,
    pub live_tabs: Option<LiveBrowserTabsRef>,
    focus: FocusHandle,
    _lifecycle_task: Task<()>,
    _event_task: Task<()>,
}

impl BrowserWindow {
    pub fn new(
        _window: &mut Window,
        cx: &mut Context<Self>,
        db: Option<Arc<rover_db::RoverDb>>,
    ) -> Self {
        let focus = cx.focus_handle();

        let history_manager = db.as_ref().map(|d| HistoryManager::new(d.clone()));
        let bookmark_manager = db.as_ref().map(|d| BookmarkManager::new(d.clone()));

        // Pre-resolve common CDN hostnames in the background.
        dns_prefetch::prefetch_common();

        let lifecycle_task = cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(LIFECYCLE_INTERVAL).await;
            if this.update(cx, |b, cx| b.run_lifecycle(cx)).is_err() {
                break;
            }
        });

        let event_task = cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(EVENT_POLL_INTERVAL).await;
            if this.update(cx, |b, cx| b.drain_events(cx)).is_err() {
                break;
            }
        });

        // Restore previous session if available.
        let (tabs, active_tab) = match db.as_ref().and_then(|d| d.load_session().ok()) {
            Some(session) if !session.is_empty() => {
                let mut active = 0;
                let tabs: Vec<BrowserTab> = session
                    .into_iter()
                    .enumerate()
                    .map(|(i, (url, title, is_active))| {
                        if is_active {
                            active = i;
                        }
                        BrowserTab {
                            title,
                            url,
                            state: if is_active { TabState::Active } else { TabState::Frozen },
                            last_active: Instant::now(),
                            events: nav_events::new_queue(),
                            #[cfg(target_os = "macos")]
                            webview_handle: None,
                        }
                    })
                    .collect();
                log::info!("session: restored {} tabs", tabs.len());
                (tabs, active)
            }
            _ => {
                let tabs = vec![BrowserTab {
                    title: "Perplexity".to_string(),
                    url: DEFAULT_URL.to_string(),
                    state: TabState::Active,
                    last_active: Instant::now(),
                    events: nav_events::new_queue(),
                    #[cfg(target_os = "macos")]
                    webview_handle: None,
                }];
                (tabs, 0)
            }
        };

        Self {
            semantic_index: None,
            tabs,
            active_tab,
            overlay_hint: None,
            history_manager,
            bookmark_manager,
            db: db.clone(),
            live_tabs: None,
            focus,
            _lifecycle_task: lifecycle_task,
            _event_task: event_task,
        }
    }

    // ── Public API for the transport ───────────────────────────────────────────

    /// Returns (title, tab_state) pairs for all tabs plus the active index.
    pub fn tab_infos(&self) -> (Vec<(String, TabState)>, usize) {
        let infos = self
            .tabs
            .iter()
            .map(|t| (t.title.clone(), t.state.clone()))
            .collect();
        (infos, self.active_tab)
    }

    /// Active tab's URL for display in transport.
    pub fn active_url(&self) -> String {
        self.tabs
            .get(self.active_tab)
            .map(|t| t.url.clone())
            .unwrap_or_default()
    }

    /// Active tab's title.
    pub fn active_title(&self) -> String {
        self.tabs
            .get(self.active_tab)
            .map(|t| t.title.clone())
            .unwrap_or_default()
    }

    /// Returns Some(bookmark_id) if the active tab is bookmarked.
    pub fn active_tab_bookmark_id(&self) -> Option<i64> {
        let url = self.tabs.get(self.active_tab)?.url.clone();
        self.bookmark_manager.as_ref()?.find_by_url(&url)
    }

    /// Switch tab — callable from transport without a Window ref.
    pub fn switch_tab_external(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.tabs.len() || index == self.active_tab {
            cx.notify();
            return;
        }

        #[cfg(target_os = "macos")]
        if let Some(tab) = self.tabs.get(self.active_tab) {
            if let Some(wv) = &tab.webview_handle {
                wv.hide();
                wv.suspend_media();
            }
        }

        self.active_tab = index;
        let tab = &mut self.tabs[index];
        tab.last_active = Instant::now();
        tab.state = TabState::Active;

        #[cfg(target_os = "macos")]
        if let Some(wv) = &tab.webview_handle {
            wv.show();
            wv.resume_media();
        }

        cx.notify();
    }

    /// Execute an action — callable from transport without a Window ref.
    pub fn execute_action_external(&mut self, action: BrowserAction, cx: &mut Context<Self>) {
        match action {
            BrowserAction::Navigate(url) => {
                dns_prefetch::prefetch_url(&url);
                if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                    tab.url = url.clone();
                    tab.last_active = Instant::now();
                    tab.state = TabState::Active;
                    #[cfg(target_os = "macos")]
                    if let Some(wv) = &tab.webview_handle {
                        wv.navigate(&url);
                    }
                }
            }
            BrowserAction::NewSearch { engine_name, query } => {
                let url = search_url(&engine_name, &query);
                self.execute_action_external(BrowserAction::Navigate(url), cx);
                return;
            }
            BrowserAction::SwitchToTab(i) => {
                self.switch_tab_external(i, cx);
                return;
            }
            BrowserAction::NewTab => {
                self.open_tab_external(DEFAULT_URL, cx);
            }
            BrowserAction::CloseTab => {
                let i = self.active_tab;
                self.close_tab(i, cx);
            }
            BrowserAction::Back => {
                #[cfg(target_os = "macos")]
                if let Some(wv) = self.tabs.get(self.active_tab).and_then(|t| t.webview_handle.as_ref()) {
                    wv.go_back();
                }
            }
            BrowserAction::Forward => {
                #[cfg(target_os = "macos")]
                if let Some(wv) = self.tabs.get(self.active_tab).and_then(|t| t.webview_handle.as_ref()) {
                    wv.go_forward();
                }
            }
            BrowserAction::Reload => {
                #[cfg(target_os = "macos")]
                if let Some(wv) = self.tabs.get(self.active_tab).and_then(|t| t.webview_handle.as_ref()) {
                    wv.reload();
                }
            }
            BrowserAction::Find(query) => {
                #[cfg(target_os = "macos")]
                if let Some(wv) = self.tabs.get(self.active_tab).and_then(|t| t.webview_handle.as_ref()) {
                    wv.find_text(&query);
                }
            }
            BrowserAction::AddBookmark => {
                if let Some(tab) = self.tabs.get(self.active_tab) {
                    if let Some(bm) = &self.bookmark_manager {
                        let _ = bm.add(&tab.url, &tab.title);
                        self.overlay_hint = Some(format!("Bookmarked: {}", tab.title));
                    }
                }
            }
            BrowserAction::RemoveBookmark(id) => {
                if let Some(bm) = &self.bookmark_manager {
                    let _ = bm.remove(id);
                    self.overlay_hint = Some("Bookmark removed".to_string());
                }
            }
            BrowserAction::ShowHistory => {
                if let Some(hm) = &self.history_manager {
                    let recent = hm.recent(10);
                    if recent.is_empty() {
                        self.overlay_hint = Some("No history".to_string());
                    } else {
                        let lines: Vec<String> = recent
                            .iter()
                            .map(|e| format!("{} — {}", e.title, e.url))
                            .collect();
                        self.overlay_hint = Some(lines.join("\n"));
                    }
                }
            }
            BrowserAction::SearchHistory(query) => {
                if let Some(hm) = &self.history_manager {
                    let results = hm.search(&query, 5);
                    if results.is_empty() {
                        self.overlay_hint = Some(format!("No history matching '{query}'"));
                    } else {
                        let lines: Vec<String> = results
                            .iter()
                            .map(|e| format!("{} — {}", e.title, e.url))
                            .collect();
                        self.overlay_hint = Some(lines.join("\n"));
                    }
                }
            }
            BrowserAction::SearchBookmarks(query) => {
                if let Some(bm) = &self.bookmark_manager {
                    let results = bm.search(&query);
                    if results.is_empty() {
                        self.overlay_hint = Some(format!("No bookmarks matching '{query}'"));
                    } else {
                        let lines: Vec<String> = results
                            .iter()
                            .map(|b| format!("{} — {}", b.title, b.url))
                            .collect();
                        self.overlay_hint = Some(lines.join("\n"));
                    }
                }
            }
            BrowserAction::Share => {
                if let Some(tab) = self.tabs.get(self.active_tab) {
                    let url = tab.url.clone();
                    use std::io::Write;
                    if let Ok(mut child) = std::process::Command::new("pbcopy")
                        .stdin(std::process::Stdio::piped())
                        .spawn()
                    {
                        if let Some(stdin) = child.stdin.as_mut() {
                            let _ = stdin.write_all(url.as_bytes());
                        }
                        let _ = child.wait();
                        self.overlay_hint = Some(format!("Copied: {url}"));
                    }
                }
            }
            BrowserAction::ListTabs => {}
        }
        cx.notify();
    }

    // ── Internal helpers ───────────────────────────────────────────────────────

    pub fn open_tab_external(&mut self, url: &str, _cx: &mut Context<Self>) {
        #[cfg(target_os = "macos")]
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            if let Some(wv) = &tab.webview_handle {
                wv.hide();
            }
            tab.state = TabState::Cached;
            tab.last_active = Instant::now();
        }
        #[cfg(not(target_os = "macos"))]
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            tab.state = TabState::Cached;
            tab.last_active = Instant::now();
        }

        self.tabs.push(BrowserTab {
            title: domain_from_url(url).to_string(),
            url: url.to_string(),
            state: TabState::Active,
            last_active: Instant::now(),
            events: nav_events::new_queue(),
            #[cfg(target_os = "macos")]
            webview_handle: None,
        });
        self.active_tab = self.tabs.len() - 1;
    }

    fn close_tab(&mut self, index: usize, _cx: &mut Context<Self>) {
        if self.tabs.len() <= 1 {
            return;
        }
        #[cfg(target_os = "macos")]
        {
            self.tabs[index].webview_handle = None;
        }
        self.tabs.remove(index);
        if self.active_tab >= self.tabs.len() {
            self.active_tab = self.tabs.len() - 1;
        }
        #[cfg(target_os = "macos")]
        if let Some(wv) = &self.tabs[self.active_tab].webview_handle {
            wv.show();
        }
        self.tabs[self.active_tab].state = TabState::Active;
        self.tabs[self.active_tab].last_active = Instant::now();
    }

    fn save_session(&self) {
        if let Some(db) = &self.db {
            let tabs: Vec<(String, String, bool)> = self
                .tabs
                .iter()
                .enumerate()
                .map(|(i, t)| (t.url.clone(), t.title.clone(), i == self.active_tab))
                .collect();
            if let Err(e) = db.save_session(&tabs) {
                log::warn!("session save failed: {e}");
            }
        }
    }

    fn drain_events(&mut self, cx: &mut Context<Self>) {
        // Check URL inbox for navigation requests from the picker
        if let Some(ref live) = self.live_tabs {
            let (urls, close_indices) = {
                let mut lock = live.lock().unwrap();
                (
                    std::mem::take(&mut lock.url_inbox),
                    std::mem::take(&mut lock.close_inbox),
                )
            };
            if !urls.is_empty() {
                for url in urls {
                    self.open_tab_external(&url, cx);
                }
                self.save_session();
                self.update_live_tabs();
                cx.notify();
            }
            if !close_indices.is_empty() {
                // Close tabs by index (sorted descending to avoid index shift issues)
                let mut indices = close_indices;
                indices.sort_unstable();
                indices.dedup();
                for &ix in indices.iter().rev() {
                    if ix < self.tabs.len() {
                        self.close_tab(ix, cx);
                    }
                }
                self.save_session();
                self.update_live_tabs();
                cx.notify();
            }
        }

        let mut changed = false;

        for (i, tab) in self.tabs.iter_mut().enumerate() {
            let events: Vec<NavEvent> = {
                let mut queue = tab.events.lock().unwrap();
                std::mem::take(&mut *queue)
            };
            if events.is_empty() {
                continue;
            }
            changed = true;
            for event in events {
                match event {
                    NavEvent::LoadStarted { url } => {
                        if !url.is_empty() {
                            tab.url = url;
                        }
                    }
                    NavEvent::LoadCommitted { url } => {
                        if !url.is_empty() {
                            tab.url = url;
                        }
                    }
                    NavEvent::LoadFinished { url, title } => {
                        if !url.is_empty() {
                            tab.url = url.clone();
                        }
                        if !title.is_empty() {
                            tab.title = title.clone();
                        }
                        if i == self.active_tab {
                            if let Some(hm) = &self.history_manager {
                                hm.record(&url, &title);
                            }
                        }
                        // Index page content for semantic search
                        #[cfg(target_os = "macos")]
                        if let Some(semantic) = &self.semantic_index {
                            if let Some(wv) = &tab.webview_handle {
                                let sem = semantic.clone();
                                let page_url = url.clone();
                                let page_title = title.clone();
                                wv.evaluate_javascript(
                                    "document.body ? document.body.innerText.substring(0, 5000) : ''",
                                    move |text| {
                                        if let Some(content) = text {
                                            if content.len() > 50 {
                                                let doc_id = format!("page:{page_url}");
                                                let doc_text = format!("{page_title}\n\n{content}");
                                                std::thread::spawn(move || {
                                                    if let Ok(mut idx) = sem.lock() {
                                                        let meta = serde_json::json!({
                                                            "url": page_url,
                                                            "title": page_title,
                                                        });
                                                        if let Err(e) = idx.upsert(&doc_id, "pages", &doc_text, meta) {
                                                            log::warn!("semantic index page failed: {e}");
                                                        } else {
                                                            log::debug!("semantic: indexed page {page_url}");
                                                        }
                                                    }
                                                });
                                            }
                                        }
                                    },
                                );
                            }
                        }
                    }
                    NavEvent::LoadFailed { error } => {
                        log::warn!("browser: tab {i} load failed: {error}");
                    }
                    NavEvent::TitleChanged { title } => {
                        if !title.is_empty() {
                            tab.title = title;
                        }
                    }
                    NavEvent::NewTabRequested { url } => {
                        cx.spawn({
                            let url = url.clone();
                            async move |this, cx: &mut AsyncApp| {
                                let _ = this.update(cx, |bw: &mut BrowserWindow, cx| {
                                    bw.open_tab_external(&url, cx);
                                    cx.notify();
                                });
                            }
                        })
                        .detach();
                    }
                    NavEvent::DownloadStarted { suggested_filename, destination } => {
                        self.overlay_hint =
                            Some(format!("Downloaded {suggested_filename} to {destination}"));
                    }
                    NavEvent::FindResult { found, match_count } => {
                        if found {
                            self.overlay_hint = Some(format!("{match_count} matches"));
                        } else {
                            self.overlay_hint = Some("No matches".to_string());
                        }
                    }
                    NavEvent::ProcessTerminated => {
                        log::warn!("browser: tab {i} web content process terminated, reloading");
                        #[cfg(target_os = "macos")]
                        if let Some(wv) = &tab.webview_handle {
                            wv.navigate(&tab.url);
                        }
                        self.overlay_hint = Some("Page crashed — reloading".to_string());
                    }
                }
            }
        }

        if changed {
            self.save_session();
            self.update_live_tabs();
            cx.notify();
        }
    }

    /// Push current tab state to the shared LiveBrowserTabs so the picker can show it.
    fn update_live_tabs(&self) {
        if let Some(ref live) = self.live_tabs {
            if let Ok(mut lt) = live.lock() {
                lt.tabs = self.tabs.iter()
                    .map(|t| {
                        let epoch_ms = t.last_active.elapsed().as_millis() as u64;
                        // Store as "millis ago" so larger = older. The picker can sort ascending.
                        (t.title.clone(), t.url.clone(), t.state.clone(), epoch_ms)
                    })
                    .collect();
                lt.active_tab = self.active_tab;
            }
        }
    }

    fn run_lifecycle(&mut self, cx: &mut Context<Self>) {
        let now = Instant::now();
        let mut to_remove: Vec<usize> = Vec::new();

        for (i, tab) in self.tabs.iter_mut().enumerate() {
            if i == self.active_tab {
                continue;
            }
            let elapsed = now.duration_since(tab.last_active);
            match tab.state {
                TabState::Active => {
                    if elapsed >= CACHED_TIMEOUT {
                        tab.state = TabState::Cached;
                        #[cfg(target_os = "macos")]
                        if let Some(wv) = &tab.webview_handle {
                            wv.hide();
                            wv.suspend_media();
                        }
                    }
                }
                TabState::Cached => {
                    if elapsed >= FROZEN_TIMEOUT {
                        tab.state = TabState::Frozen;
                        #[cfg(target_os = "macos")]
                        {
                            if let Some(wv) = &tab.webview_handle {
                                if let Some(url) = wv.url() {
                                    tab.url = url;
                                }
                                if let Some(title) = wv.title() {
                                    if !title.is_empty() {
                                        tab.title = title;
                                    }
                                }
                            }
                            tab.webview_handle = None;
                        }
                    }
                }
                TabState::Frozen => {
                    if elapsed >= CLOSED_TIMEOUT {
                        to_remove.push(i);
                    }
                }
            }
        }

        for i in to_remove.into_iter().rev() {
            self.tabs.remove(i);
            if i < self.active_tab {
                self.active_tab -= 1;
            } else if i == self.active_tab {
                self.active_tab = self.active_tab.min(self.tabs.len().saturating_sub(1));
            }
        }

        // Persist session state for restore on next launch.
        self.save_session();

        cx.notify();
    }
}

// ─── Focusable ────────────────────────────────────────────────────────────────

impl Focusable for BrowserWindow {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

// ─── Render ───────────────────────────────────────────────────────────────────
//
// The browser window is a pure full-screen WebView. All chrome lives in the
// floating transport popup (transport.rs).

impl Render for BrowserWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let browser_weak = cx.entity().downgrade();
        div()
            .key_context("BrowserWindow")
            .track_focus(&self.focus)
            .on_action(cx.listener(move |_this, _: &OpenTransport, _window, cx| {
                transport::open_browser_transport(browser_weak.clone(), cx);
            }))
            .on_action(cx.listener(|this, _: &ShareCurrentUrl, _window, cx| {
                this.execute_action_external(BrowserAction::Share, cx);
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| {
                let key = event.keystroke.key.as_str();
                let mods = &event.keystroke.modifiers;
                if mods.platform {
                    match key {
                        "t" => { this.open_tab_external(DEFAULT_URL, cx); cx.notify(); }
                        "w" => {
                            let i = this.active_tab;
                            this.close_tab(i, cx);
                            cx.notify();
                        }
                        "[" => {
                            #[cfg(target_os = "macos")]
                            if let Some(wv) = this.tabs.get(this.active_tab).and_then(|t| t.webview_handle.as_ref()) {
                                wv.go_back();
                            }
                        }
                        "]" => {
                            #[cfg(target_os = "macos")]
                            if let Some(wv) = this.tabs.get(this.active_tab).and_then(|t| t.webview_handle.as_ref()) {
                                wv.go_forward();
                            }
                        }
                        "r" => {
                            #[cfg(target_os = "macos")]
                            if let Some(wv) = this.tabs.get(this.active_tab).and_then(|t| t.webview_handle.as_ref()) {
                                wv.reload();
                            }
                        }
                        _ => {}
                    }
                }
            }))
            .size_full()
            .child(WebViewElement { browser: cx.entity().downgrade() })
    }
}

// ─── WebViewElement ───────────────────────────────────────────────────────────

struct WebViewElement {
    browser: WeakEntity<BrowserWindow>,
}

impl IntoElement for WebViewElement {
    type Element = WebViewElement;
    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for WebViewElement {
    type RequestLayoutState = ();
    type PrepaintState = Option<Bounds<Pixels>>;

    fn id(&self) -> Option<ElementId> {
        Some("webview".into())
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.flex_grow = 1.0;
        style.size = Size {
            width: relative(1.0).into(),
            height: relative(1.0).into(),
        };
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
        Some(bounds)
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        #[cfg(target_os = "macos")]
        {
            let Some(bounds) = prepaint else { return };

            use raw_window_handle::HasWindowHandle;
            let raw_handle = match window.window_handle() {
                Ok(h) => h,
                Err(_) => return,
            };
            let ns_view = match raw_handle.as_raw() {
                raw_window_handle::RawWindowHandle::AppKit(h) => {
                    h.ns_view.as_ptr() as *mut objc::runtime::Object
                }
                _ => return,
            };

            // GPUI Pixels are logical pixels = NSPoints.
            // viewport_size() is already in logical pixels — no scale conversion needed.
            // Only Y-axis needs flipping (NSView origin is bottom-left, GPUI is top-left).
            let viewport_h = f64::from(window.viewport_size().height);
            let x = f64::from(bounds.origin.x);
            let y_top = f64::from(bounds.origin.y);
            let width = f64::from(bounds.size.width);
            let height = f64::from(bounds.size.height);
            let y = viewport_h - y_top - height;

            if let Some(browser) = self.browser.upgrade() {
                browser.update(cx, |bw, _cx| {
                    if let Some(tab) = bw.tabs.get_mut(bw.active_tab) {
                        if let Some(wv) = &tab.webview_handle {
                            wv.set_frame(x, y, width, height);
                            wv.show();
                            if let Some(title) = wv.title() {
                                if !title.is_empty() {
                                    tab.title = title;
                                }
                            }
                        } else {
                            let events = tab.events.clone();
                            // Try the warm-up pool first (pre-spun process), fall back to fresh.
                            let wv = warmup_pool::take(ns_view, x, y, width, height, events.clone())
                                .or_else(|| WebViewHandle::create(ns_view, x, y, width, height, events));
                            if let Some(wv) = wv {
                                wv.navigate(&tab.url);
                                tab.webview_handle = Some(wv);
                                // Refill the pool in the background for next time.
                                unsafe { warmup_pool::refill(ns_view); }
                            }
                        }
                    }
                });
            }
        }
    }
}

// ─── Entry points ─────────────────────────────────────────────────────────────

pub fn open_browser_window(cx: &mut App, db: Option<Arc<rover_db::RoverDb>>) {
    open_browser_window_inner(cx, DEFAULT_URL, db, None, None);
}

pub fn open_browser_window_with_url(cx: &mut App, url: &str, db: Option<Arc<rover_db::RoverDb>>) {
    open_browser_window_inner(cx, url, db, None, None);
}

pub fn open_browser_window_with_semantic(
    cx: &mut App,
    url: &str,
    db: Option<Arc<rover_db::RoverDb>>,
    semantic: Option<Arc<std::sync::Mutex<rover_semantic::SemanticIndex>>>,
) {
    open_browser_window_inner(cx, url, db, semantic, None);
}

pub fn open_browser_window_with_live_tabs(
    cx: &mut App,
    url: &str,
    db: Option<Arc<rover_db::RoverDb>>,
    semantic: Option<Arc<std::sync::Mutex<rover_semantic::SemanticIndex>>>,
    live_tabs: Option<LiveBrowserTabsRef>,
) {
    open_browser_window_inner(cx, url, db, semantic, live_tabs);
}

/// Navigate to a URL in the existing browser window (new tab) if one exists,
/// otherwise open a new browser window.
pub fn navigate_or_open(
    cx: &mut App,
    url: &str,
    db: Option<Arc<rover_db::RoverDb>>,
    semantic: Option<Arc<std::sync::Mutex<rover_semantic::SemanticIndex>>>,
    live_tabs: Option<LiveBrowserTabsRef>,
) {
    // Try to push URL to existing browser window's inbox
    if let Some(ref lt) = live_tabs {
        let has_window = {
            let lock = lt.lock().unwrap();
            !lock.tabs.is_empty()
        };
        if has_window {
            lt.lock().unwrap().url_inbox.push(url.to_string());
            return;
        }
    }
    // No existing window — open a new one
    open_browser_window_inner(cx, url, db, semantic, live_tabs);
}

fn open_browser_window_inner(
    cx: &mut App,
    url: &str,
    db: Option<Arc<rover_db::RoverDb>>,
    semantic: Option<Arc<std::sync::Mutex<rover_semantic::SemanticIndex>>>,
    live_tabs: Option<LiveBrowserTabsRef>,
) {

    // Cmd+\ opens the floating transport from anywhere in the browser window.
    // Cmd+Shift+C copies the current URL directly (no transport needed).
    cx.bind_keys([
        KeyBinding::new("cmd-\\", OpenTransport, Some("BrowserWindow")),
        KeyBinding::new("cmd-shift-c", ShareCurrentUrl, Some("BrowserWindow")),
    ]);

    let url = url.to_string();
    let window_handle = cx.open_window(
        WindowOptions {
            window_bounds: None, // fullscreen / maximised by OS
            titlebar: None,
            kind: WindowKind::Normal,
            focus: true,
            is_movable: true,
            is_resizable: true,
            window_background: WindowBackgroundAppearance::Opaque,
            ..Default::default()
        },
        move |window, cx| {
            cx.new(|cx| {
                let mut bw = BrowserWindow::new(window, cx, db);
                bw.semantic_index = semantic;
                bw.live_tabs = live_tabs;
                if let Some(tab) = bw.tabs.get_mut(0) {
                    tab.url = url.clone();
                    tab.title = looks_like_url(&url)
                        .then(|| {
                            url.strip_prefix("https://")
                                .or_else(|| url.strip_prefix("http://"))
                                .unwrap_or(&url)
                                .split('/')
                                .next()
                                .unwrap_or(&url)
                                .trim_start_matches("www.")
                                .to_string()
                        })
                        .unwrap_or_else(|| url.clone());
                }
                bw
            })
        },
    )
    .ok();

    // Install option+drag window move handler
    #[cfg(target_os = "macos")]
    install_option_drag_handler();
}

/// Installs a local NSEvent monitor that listens for Option+leftMouseDragged
/// and calls `performWindowDragWithEvent:` on the key window so the user can
/// drag the browser window by holding Option and clicking anywhere.
#[cfg(target_os = "macos")]
fn install_option_drag_handler() {
    use cocoa::base::{id, nil};
    use objc::*;

    unsafe {
        // NSEventMaskLeftMouseDragged = 1 << 6 = 64
        let mask: u64 = 1 << 6;
        let block = block::ConcreteBlock::new(move |event: id| -> id {
            let modifier_flags: u64 = msg_send![event, modifierFlags];
            // NSEventModifierFlagOption = 1 << 19 = 524288
            let option_held = modifier_flags & (1 << 19) != 0;
            if option_held {
                let app: id = msg_send![class!(NSApplication), sharedApplication];
                let key_window: id = msg_send![app, keyWindow];
                if key_window != nil {
                    let _: () = msg_send![key_window, performWindowDragWithEvent: event];
                }
            }
            event
        });
        let block = block.copy();
        let _: id = msg_send![
            class!(NSEvent),
            addLocalMonitorForEventsMatchingMask: mask
            handler: &*block
        ];
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::{
        fuzzy_score, looks_like_url, parse_command, search_url, BrowserAction, CACHED_TIMEOUT,
        CLOSED_TIMEOUT, FROZEN_TIMEOUT,
    };
    use std::time::Duration;

    #[test]
    fn test_parse_command_navigation_commands() {
        assert!(matches!(parse_command("back"), BrowserAction::Back));
        assert!(matches!(parse_command("<"), BrowserAction::Back));
        assert!(matches!(parse_command("forward"), BrowserAction::Forward));
        assert!(matches!(parse_command(">"), BrowserAction::Forward));
        assert!(matches!(parse_command("reload"), BrowserAction::Reload));
        assert!(matches!(parse_command("refresh"), BrowserAction::Reload));
        assert!(matches!(parse_command("close"), BrowserAction::CloseTab));
        assert!(matches!(parse_command("tabs"), BrowserAction::ListTabs));
        assert!(matches!(parse_command("ls"), BrowserAction::ListTabs));
        assert!(matches!(parse_command("list"), BrowserAction::ListTabs));
        assert!(matches!(parse_command("new tab"), BrowserAction::NewTab));
    }

    #[test]
    fn test_parse_command_search_engines() {
        let cases: &[(&str, &[&str])] = &[
            ("google", &["google", "g"]),
            ("youtube", &["youtube", "yt"]),
            ("wikipedia", &["wikipedia", "wiki"]),
            ("perplexity", &["perplexity", "pp"]),
            ("github", &["github", "gh"]),
            ("reddit", &["reddit", "r"]),
            ("stackoverflow", &["stackoverflow", "so"]),
            ("duckduckgo", &["duckduckgo", "ddg"]),
            ("amazon", &["amazon", "amz"]),
            ("twitter", &["twitter", "x"]),
            ("maps", &["maps"]),
        ];
        for (engine, prefixes) in cases {
            for prefix in *prefixes {
                let input = format!("{prefix} rust");
                match parse_command(&input) {
                    BrowserAction::NewSearch { engine_name, query } => {
                        assert_eq!(engine_name, *engine, "prefix '{prefix}'");
                        assert_eq!(query, "rust");
                    }
                    other => panic!("Expected NewSearch for '{prefix}', got {other:?}"),
                }
            }
        }
    }

    #[test]
    fn test_parse_command_urls() {
        match parse_command("https://example.com") {
            BrowserAction::Navigate(url) => assert_eq!(url, "https://example.com"),
            other => panic!("Expected Navigate, got {other:?}"),
        }
        match parse_command("example.com") {
            BrowserAction::Navigate(url) => assert_eq!(url, "https://example.com"),
            other => panic!("Expected Navigate, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_command_fallback_to_perplexity() {
        match parse_command("rust programming language") {
            BrowserAction::NewSearch { engine_name, query } => {
                assert_eq!(engine_name, "perplexity");
                assert_eq!(query, "rust programming language");
            }
            other => panic!("Expected perplexity fallback, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_command_empty_input() {
        match parse_command("") {
            BrowserAction::Navigate(url) => assert_eq!(url, "about:blank"),
            other => panic!("Expected Navigate(about:blank), got {other:?}"),
        }
    }

    #[test]
    fn test_parse_command_find() {
        match parse_command("/hello world") {
            BrowserAction::Find(q) => assert_eq!(q, "hello world"),
            other => panic!("Expected Find, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_command_history_search() {
        match parse_command("h rust") {
            BrowserAction::SearchHistory(q) => assert_eq!(q, "rust"),
            other => panic!("Expected SearchHistory, got {other:?}"),
        }
    }

    #[test]
    fn test_parse_command_bookmark() {
        assert!(matches!(parse_command("bk!"), BrowserAction::AddBookmark));
        match parse_command("bk rust") {
            BrowserAction::SearchBookmarks(q) => assert_eq!(q, "rust"),
            other => panic!("Expected SearchBookmarks, got {other:?}"),
        }
    }

    #[test]
    fn test_looks_like_url() {
        assert!(looks_like_url("https://example.com"));
        assert!(looks_like_url("http://example.com"));
        assert!(looks_like_url("example.com"));
        assert!(looks_like_url("www.google.com"));
        assert!(looks_like_url("github.com/rust-lang/rust"));
        assert!(looks_like_url("about:blank"));

        assert!(!looks_like_url("hello world"));
        assert!(!looks_like_url("rust programming"));
        assert!(!looks_like_url("no dots here"));
    }

    #[test]
    fn test_search_url_generation() {
        let url = search_url("google", "rust");
        assert!(url.contains("google.com/search") && url.contains("rust"));

        let url = search_url("youtube", "gpui");
        assert!(url.contains("youtube.com") && url.contains("gpui"));

        let url = search_url("nonexistent", "test");
        assert!(url.contains("perplexity.ai"));
    }

    #[test]
    fn test_fuzzy_score() {
        assert!(fuzzy_score("rust", "rust programming") > 0.0);
        assert!(fuzzy_score("rust", "rust programming") > fuzzy_score("rust", "programming in rust"));
        assert_eq!(fuzzy_score("", "anything"), 1.0);
        assert_eq!(fuzzy_score("xyz", "hello world"), 0.0);
    }

    #[test]
    fn test_tab_lifecycle_constants() {
        assert!(CACHED_TIMEOUT < FROZEN_TIMEOUT);
        assert!(FROZEN_TIMEOUT < CLOSED_TIMEOUT);
        assert_eq!(CACHED_TIMEOUT, Duration::from_secs(5 * 60));
        assert_eq!(FROZEN_TIMEOUT, Duration::from_secs(30 * 60));
        assert_eq!(CLOSED_TIMEOUT, Duration::from_secs(2 * 60 * 60));
    }
}
