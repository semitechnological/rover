#![recursion_limit = "512"]

mod webview;

use anyhow::Result;
use gpui::*;
use std::time::{Duration, Instant};

#[cfg(target_os = "macos")]
use webview::WebViewHandle;

pub const CACHED_TIMEOUT: Duration = Duration::from_secs(5 * 60);
pub const FROZEN_TIMEOUT: Duration = Duration::from_secs(30 * 60);
pub const CLOSED_TIMEOUT: Duration = Duration::from_secs(2 * 60 * 60);

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
    #[cfg(target_os = "macos")]
    pub webview_handle: Option<WebViewHandle>,
}

#[derive(Debug, Clone)]
pub enum BrowserAction {
    Navigate(String),
    SwitchToTab(usize),
    CloseTab,
    Back,
    Forward,
    Reload,
    ListTabs,
    NewSearch { engine_name: String, query: String },
}

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

pub fn parse_command(input: &str) -> BrowserAction {
    let trimmed = input.trim();

    if trimmed.is_empty() {
        return BrowserAction::Navigate("about:blank".to_string());
    }

    match trimmed {
        "back" | "<" => return BrowserAction::Back,
        "forward" | ">" => return BrowserAction::Forward,
        "reload" | "r" | "refresh" => return BrowserAction::Reload,
        "close" | "x" => return BrowserAction::CloseTab,
        "tabs" | "ls" | "list" => return BrowserAction::ListTabs,
        _ => {}
    }

    if let Some((prefix, query)) = trimmed.split_once(' ') {
        let prefix_lower = prefix.to_lowercase();
        for engine in SEARCH_ENGINES {
            if prefix_lower == engine.long_prefix
                || engine
                    .short_prefix
                    .is_some_and(|short| prefix_lower == short)
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
        engine_name: "google".to_string(),
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

    let without_path = input.split('/').next().unwrap_or(input);
    let without_port = without_path.split(':').next().unwrap_or(without_path);

    if without_port.contains('.') && !without_port.contains(' ') {
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
    let encoded_fallback = urlencoding::encode(query);
    format!("https://www.google.com/search?q={}", encoded_fallback)
}

pub fn fuzzy_score(query: &str, text: &str) -> f32 {
    if query.is_empty() {
        return 1.0;
    }

    let query_lower = query.to_lowercase();
    let text_lower = text.to_lowercase();

    if text_lower.contains(&query_lower) {
        let position_bonus = if text_lower.starts_with(&query_lower) {
            1.0
        } else {
            0.5
        };
        return position_bonus + (query_lower.len() as f32 / text_lower.len() as f32);
    }

    let mut query_chars = query_lower.chars().peekable();
    let mut score = 0.0f32;

    for character in text_lower.chars() {
        if let Some(&next_query_char) = query_chars.peek() {
            if character == next_query_char {
                query_chars.next();
                score += 1.0;
            }
        }
    }

    if query_chars.peek().is_some() {
        return 0.0;
    }

    score / query_lower.len() as f32 * 0.3
}

pub struct BrowserWindow {
    tabs: Vec<BrowserTab>,
    active_tab: usize,
    command_bar_text: String,
    command_bar_focus: FocusHandle,
}

impl BrowserWindow {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();

        let initial_tab = BrowserTab {
            title: "New Tab".to_string(),
            url: "about:blank".to_string(),
            state: TabState::Active,
            last_active: Instant::now(),
            #[cfg(target_os = "macos")]
            webview_handle: None,
        };

        let mut browser = Self {
            tabs: vec![initial_tab],
            active_tab: 0,
            command_bar_text: String::new(),
            command_bar_focus: focus_handle,
        };

        browser.navigate_current("about:blank", window, cx);
        browser
    }

    fn navigate_current(
        &mut self,
        url: &str,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            tab.url = url.to_string();
            tab.last_active = Instant::now();
            tab.state = TabState::Active;

            #[cfg(target_os = "macos")]
            if let Some(webview) = &tab.webview_handle {
                webview.navigate(url);
            }
        }
    }

    pub fn execute_action(&mut self, action: BrowserAction, window: &mut Window, cx: &mut Context<Self>) {
        match action {
            BrowserAction::Navigate(url) => {
                self.navigate_current(&url, window, cx);
            }
            BrowserAction::NewSearch { engine_name, query } => {
                let url = search_url(&engine_name, &query);
                self.navigate_current(&url, window, cx);
            }
            BrowserAction::Back => {
                #[cfg(target_os = "macos")]
                if let Some(tab) = self.tabs.get(self.active_tab) {
                    if let Some(webview) = &tab.webview_handle {
                        webview.go_back();
                    }
                }
            }
            BrowserAction::Forward => {
                #[cfg(target_os = "macos")]
                if let Some(tab) = self.tabs.get(self.active_tab) {
                    if let Some(webview) = &tab.webview_handle {
                        webview.go_forward();
                    }
                }
            }
            BrowserAction::Reload => {
                #[cfg(target_os = "macos")]
                if let Some(tab) = self.tabs.get(self.active_tab) {
                    if let Some(webview) = &tab.webview_handle {
                        webview.reload();
                    }
                }
            }
            BrowserAction::CloseTab => {
                if self.tabs.len() > 1 {
                    #[cfg(target_os = "macos")]
                    {
                        if let Some(mut tab) = self.tabs.get_mut(self.active_tab) {
                            tab.webview_handle = None;
                        }
                    }
                    self.tabs.remove(self.active_tab);
                    if self.active_tab >= self.tabs.len() {
                        self.active_tab = self.tabs.len().saturating_sub(1);
                    }
                }
            }
            BrowserAction::SwitchToTab(index) => {
                if index < self.tabs.len() {
                    #[cfg(target_os = "macos")]
                    {
                        if let Some(tab) = self.tabs.get(self.active_tab) {
                            if let Some(webview) = &tab.webview_handle {
                                webview.hide();
                            }
                        }
                    }
                    self.active_tab = index;
                    if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                        tab.last_active = Instant::now();
                        tab.state = TabState::Active;
                        #[cfg(target_os = "macos")]
                        if let Some(webview) = &tab.webview_handle {
                            webview.show();
                        }
                    }
                }
            }
            BrowserAction::ListTabs => {}
        }
        cx.notify();
    }
}

impl Render for BrowserWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let command_bar_focus = self.command_bar_focus.clone();
        let current_url = self
            .tabs
            .get(self.active_tab)
            .map(|tab| tab.url.clone())
            .unwrap_or_default();

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x1e1e1e))
            .child(
                div()
                    .flex()
                    .items_center()
                    .px_3()
                    .py_2()
                    .gap_2()
                    .bg(rgb(0x2d2d2d))
                    .border_b_1()
                    .border_color(rgb(0x3d3d3d))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(
                                div()
                                    .w(px(12.0))
                                    .h(px(12.0))
                                    .rounded_full()
                                    .bg(rgb(0xff5f57)),
                            )
                            .child(
                                div()
                                    .w(px(12.0))
                                    .h(px(12.0))
                                    .rounded_full()
                                    .bg(rgb(0xffbd2e)),
                            )
                            .child(
                                div()
                                    .w(px(12.0))
                                    .h(px(12.0))
                                    .rounded_full()
                                    .bg(rgb(0x28c840)),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .bg(rgb(0x1e1e1e))
                            .border_1()
                            .border_color(if command_bar_focus.is_focused(_window) {
                                        rgb(0x5588cc)
                                    } else {
                                        rgb(0x444444)
                                    })
                            .child(
                                div()
                                    .text_color(rgb(0xcccccc))
                                    .text_size(px(13.0))
                                    .child(SharedString::from(current_url)),
                            ),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .relative()
                    .child(WebViewElement {
                        browser: cx.entity().downgrade(),
                    }),
            )
    }
}

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
        let layout_id = window.request_layout(style, [], cx);
        (layout_id, ())
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
                Ok(handle) => handle,
                Err(_) => return,
            };

            let ns_view = match raw_handle.as_raw() {
                raw_window_handle::RawWindowHandle::AppKit(appkit_handle) => {
                    appkit_handle.ns_view.as_ptr() as *mut objc::runtime::Object
                }
                _ => return,
            };

            let scale = window.scale_factor();
            let x = f64::from(bounds.origin.x) / scale as f64;
            let y = f64::from(bounds.origin.y) / scale as f64;
            let width = f64::from(bounds.size.width) / scale as f64;
            let height = f64::from(bounds.size.height) / scale as f64;

            if let Some(browser) = self.browser.upgrade() {
                browser.update(cx, |browser_window, _cx| {
                    if let Some(tab) = browser_window.tabs.get_mut(browser_window.active_tab) {
                        if let Some(webview) = &tab.webview_handle {
                            webview.set_frame(x, y, width, height);
                            webview.show();
                        } else if let Some(webview) =
                            WebViewHandle::create(ns_view, x, y, width, height)
                        {
                            let url = tab.url.clone();
                            webview.navigate(&url);
                            tab.webview_handle = Some(webview);
                        }
                    }
                });
            }
        }
    }
}

pub fn open_browser_window(cx: &mut App) {
    let bounds = Bounds::centered(None, size(px(1200.0), px(800.0)), cx);

    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: None,
            kind: WindowKind::Normal,
            focus: true,
            is_movable: true,
            is_resizable: true,
            window_background: WindowBackgroundAppearance::Opaque,
            ..Default::default()
        },
        |window, cx| cx.new(|cx| BrowserWindow::new(window, cx)),
    )
    .ok();
}

#[cfg(test)]
mod tests {
    use super::{
        BrowserAction, CACHED_TIMEOUT, CLOSED_TIMEOUT, FROZEN_TIMEOUT,
        fuzzy_score, looks_like_url, parse_command, search_url,
    };
    use std::time::Duration;

    #[test]
    fn test_parse_command_navigation_commands() {
        assert!(matches!(parse_command("back"), BrowserAction::Back));
        assert!(matches!(parse_command("<"), BrowserAction::Back));
        assert!(matches!(parse_command("forward"), BrowserAction::Forward));
        assert!(matches!(parse_command(">"), BrowserAction::Forward));
        assert!(matches!(parse_command("reload"), BrowserAction::Reload));
        assert!(matches!(parse_command("r"), BrowserAction::Reload));
        assert!(matches!(parse_command("refresh"), BrowserAction::Reload));
        assert!(matches!(parse_command("close"), BrowserAction::CloseTab));
        assert!(matches!(parse_command("x"), BrowserAction::CloseTab));
        assert!(matches!(parse_command("tabs"), BrowserAction::ListTabs));
        assert!(matches!(parse_command("ls"), BrowserAction::ListTabs));
        assert!(matches!(parse_command("list"), BrowserAction::ListTabs));
    }

    #[test]
    fn test_parse_command_search_engines() {
        let engines_and_prefixes: &[(&str, &[&str])] = &[
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

        for (expected_engine, prefixes) in engines_and_prefixes {
            for prefix in *prefixes {
                let input = format!("{} rust programming", prefix);
                match parse_command(&input) {
                    BrowserAction::NewSearch { engine_name, query } => {
                        assert_eq!(
                            engine_name, *expected_engine,
                            "Expected engine '{}' for prefix '{}', got '{}'",
                            expected_engine, prefix, engine_name
                        );
                        assert_eq!(query, "rust programming");
                    }
                    other => panic!(
                        "Expected NewSearch for prefix '{}', got {:?}",
                        prefix, other
                    ),
                }
            }
        }
    }

    #[test]
    fn test_parse_command_urls() {
        match parse_command("https://example.com") {
            BrowserAction::Navigate(url) => assert_eq!(url, "https://example.com"),
            other => panic!("Expected Navigate, got {:?}", other),
        }

        match parse_command("http://example.com") {
            BrowserAction::Navigate(url) => assert_eq!(url, "http://example.com"),
            other => panic!("Expected Navigate, got {:?}", other),
        }

        match parse_command("example.com") {
            BrowserAction::Navigate(url) => assert_eq!(url, "https://example.com"),
            other => panic!("Expected Navigate, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_command_fallback_to_google() {
        match parse_command("rust programming language") {
            BrowserAction::NewSearch { engine_name, query } => {
                assert_eq!(engine_name, "google");
                assert_eq!(query, "rust programming language");
            }
            other => panic!("Expected NewSearch (google fallback), got {:?}", other),
        }

        match parse_command("hello world") {
            BrowserAction::NewSearch { engine_name, query } => {
                assert_eq!(engine_name, "google");
                assert_eq!(query, "hello world");
            }
            other => panic!("Expected NewSearch (google fallback), got {:?}", other),
        }
    }

    #[test]
    fn test_parse_command_empty_input() {
        match parse_command("") {
            BrowserAction::Navigate(url) => assert_eq!(url, "about:blank"),
            other => panic!("Expected Navigate(about:blank), got {:?}", other),
        }

        match parse_command("   ") {
            BrowserAction::Navigate(url) => assert_eq!(url, "about:blank"),
            other => panic!("Expected Navigate(about:blank), got {:?}", other),
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
        assert!(looks_like_url("file:///home/user/file.html"));

        assert!(!looks_like_url("hello world"));
        assert!(!looks_like_url("rust programming"));
        assert!(!looks_like_url("plain text query"));
        assert!(!looks_like_url("no dots here"));
    }

    #[test]
    fn test_search_url_generation() {
        let google_url = search_url("google", "rust");
        assert!(google_url.contains("google.com/search"));
        assert!(google_url.contains("rust"));

        let youtube_url = search_url("youtube", "gpui tutorial");
        assert!(youtube_url.contains("youtube.com"));
        assert!(youtube_url.contains("gpui"));

        let github_url = search_url("github", "gpui");
        assert!(github_url.contains("github.com/search"));

        let maps_url = search_url("maps", "coffee shop");
        assert!(maps_url.contains("maps.apple.com"));
        assert!(maps_url.contains("coffee"));

        let fallback_url = search_url("nonexistent_engine", "test");
        assert!(fallback_url.contains("google.com/search"));
    }

    #[test]
    fn test_fuzzy_score() {
        assert!(fuzzy_score("rust", "rust programming") > 0.0);
        assert!(fuzzy_score("rust", "rust programming") > fuzzy_score("rust", "programming in rust"));
        assert_eq!(fuzzy_score("", "anything"), 1.0);
        assert_eq!(fuzzy_score("xyz", "hello world"), 0.0);
        assert!(fuzzy_score("rp", "rust programming") > 0.0);
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
