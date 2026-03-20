#![recursion_limit = "4096"]

mod webview;

use gpui::*;
use std::time::{Duration, Instant};

#[cfg(target_os = "macos")]
use webview::WebViewHandle;

// ─── Constants ────────────────────────────────────────────────────────────────

pub const DEFAULT_URL: &str = "https://www.perplexity.ai";
const FALLBACK_ENGINE: &str = "perplexity";
pub const CACHED_TIMEOUT: Duration = Duration::from_secs(5 * 60);
pub const FROZEN_TIMEOUT: Duration = Duration::from_secs(30 * 60);
pub const CLOSED_TIMEOUT: Duration = Duration::from_secs(2 * 60 * 60);
const LIFECYCLE_INTERVAL: Duration = Duration::from_secs(30);
const CHROME_HEIGHT: f32 = 44.0;

actions!(rover_browser, [FocusCommandBar]);

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
}

// ─── Command parsing ──────────────────────────────────────────────────────────

pub fn parse_command(input: &str) -> BrowserAction {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return BrowserAction::Navigate("about:blank".to_string());
    }

    match trimmed {
        "back" | "<" => return BrowserAction::Back,
        "forward" | ">" => return BrowserAction::Forward,
        "reload" | "refresh" => return BrowserAction::Reload,
        "close" => return BrowserAction::CloseTab,
        "new tab" | "newtab" => return BrowserAction::NewTab,
        "tabs" | "ls" | "list" => return BrowserAction::ListTabs,
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
    // Fallback to perplexity
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
fn domain_from_url(url: &str) -> &str {
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

pub struct BrowserWindow {
    tabs: Vec<BrowserTab>,
    active_tab: usize,
    command_bar_text: String,
    command_bar_active: bool,
    command_bar_focus: FocusHandle,
    /// Hover state for the title area (hover shows full URL, default shows page title).
    title_hovered: bool,
    _lifecycle_task: Task<()>,
}

impl BrowserWindow {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();

        let lifecycle_task = cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(LIFECYCLE_INTERVAL).await;
            if this.update(cx, |b, cx| b.run_lifecycle(cx)).is_err() {
                break;
            }
        });

        Self {
            tabs: vec![BrowserTab {
                title: "Perplexity".to_string(),
                url: DEFAULT_URL.to_string(),
                state: TabState::Active,
                last_active: Instant::now(),
                #[cfg(target_os = "macos")]
                webview_handle: None,
            }],
            active_tab: 0,
            command_bar_text: String::new(),
            command_bar_active: false,
            command_bar_focus: focus,
            title_hovered: false,
            _lifecycle_task: lifecycle_task,
        }
    }

    // ── Lifecycle ──────────────────────────────────────────────────────────────

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

        cx.notify();
    }

    // ── Navigation ─────────────────────────────────────────────────────────────

    fn navigate_current(&mut self, url: &str, _window: &mut Window, _cx: &mut Context<Self>) {
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            tab.url = url.to_string();
            tab.last_active = Instant::now();
            tab.state = TabState::Active;
            #[cfg(target_os = "macos")]
            if let Some(wv) = &tab.webview_handle {
                wv.navigate(url);
            }
        }
    }

    fn open_tab(&mut self, url: &str, _cx: &mut Context<Self>) {
        // Cache / hide current tab's webview
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
            #[cfg(target_os = "macos")]
            webview_handle: None,
        });
        self.active_tab = self.tabs.len() - 1;
    }

    fn switch_tab(&mut self, index: usize, _window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.tabs.len() || index == self.active_tab {
            self.command_bar_active = false;
            cx.notify();
            return;
        }

        // Hide old tab's webview
        #[cfg(target_os = "macos")]
        if let Some(tab) = self.tabs.get(self.active_tab) {
            if let Some(wv) = &tab.webview_handle {
                wv.hide();
            }
        }

        self.active_tab = index;
        let tab = &mut self.tabs[index];
        tab.last_active = Instant::now();
        tab.state = TabState::Active;

        // Show new tab's webview (if already loaded)
        #[cfg(target_os = "macos")]
        if let Some(wv) = &tab.webview_handle {
            wv.show();
        }

        self.command_bar_active = false;
        cx.notify();
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

    fn execute_action(&mut self, action: BrowserAction, window: &mut Window, cx: &mut Context<Self>) {
        match action {
            BrowserAction::Navigate(url) => self.navigate_current(&url, window, cx),
            BrowserAction::NewSearch { engine_name, query } => {
                let url = search_url(&engine_name, &query);
                self.navigate_current(&url, window, cx);
            }
            BrowserAction::SwitchToTab(i) => {
                self.switch_tab(i, window, cx);
                return; // switch_tab calls notify
            }
            BrowserAction::NewTab => self.open_tab(DEFAULT_URL, cx),
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
            BrowserAction::ListTabs => {}
        }
        self.command_bar_active = false;
        cx.notify();
    }

    // ── Command bar ────────────────────────────────────────────────────────────

    fn activate_command_bar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.command_bar_active = true;
        self.command_bar_text.clear();
        window.focus(&self.command_bar_focus);
        cx.notify();
    }

    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = std::mem::take(&mut self.command_bar_text);
        self.command_bar_active = false;
        // Tabs take priority over URL/search parsing
        if let Some((index, _)) = self.best_tab_match(&text) {
            self.switch_tab(index, window, cx);
        } else {
            let action = parse_command(&text);
            self.execute_action(action, window, cx);
        }
    }

    fn handle_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = event.keystroke.key.as_str();
        let mods = &event.keystroke.modifiers;

        // Global cmd shortcuts always available
        if mods.platform {
            match key {
                "t" => {
                    self.open_tab(DEFAULT_URL, cx);
                    cx.notify();
                    return;
                }
                "w" => {
                    let i = self.active_tab;
                    self.close_tab(i, cx);
                    cx.notify();
                    return;
                }
                "[" => {
                    #[cfg(target_os = "macos")]
                    if let Some(wv) = self.tabs.get(self.active_tab).and_then(|t| t.webview_handle.as_ref()) {
                        wv.go_back();
                    }
                    return;
                }
                "]" => {
                    #[cfg(target_os = "macos")]
                    if let Some(wv) = self.tabs.get(self.active_tab).and_then(|t| t.webview_handle.as_ref()) {
                        wv.go_forward();
                    }
                    return;
                }
                "r" => {
                    #[cfg(target_os = "macos")]
                    if let Some(wv) = self.tabs.get(self.active_tab).and_then(|t| t.webview_handle.as_ref()) {
                        wv.reload();
                    }
                    return;
                }
                _ => {}
            }
        }

        if !self.command_bar_active {
            return;
        }

        match key {
            "escape" => {
                self.command_bar_active = false;
                self.command_bar_text.clear();
                cx.notify();
            }
            "return" | "enter" => {
                self.submit(window, cx);
            }
            "backspace" => {
                self.command_bar_text.pop();
                cx.notify();
            }
            key if key.chars().count() == 1
                && key.chars().next().map_or(false, |c| !c.is_control())
                && !mods.platform
                && !mods.control =>
            {
                self.command_bar_text.push_str(key);
                cx.notify();
            }
            _ => {}
        }
    }

    fn best_tab_match(&self, query: &str) -> Option<(usize, &str)> {
        if query.is_empty() {
            return None;
        }
        let mut best_score = 0.4f32;
        let mut best = None;
        for (i, tab) in self.tabs.iter().enumerate() {
            let score = fuzzy_score(query, &tab.title).max(fuzzy_score(query, &tab.url));
            if score > best_score {
                best_score = score;
                best = Some((i, tab.title.as_str()));
            }
        }
        best
    }
}

// ─── Focusable ────────────────────────────────────────────────────────────────

impl Focusable for BrowserWindow {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.command_bar_focus.clone()
    }
}

// ─── Render ───────────────────────────────────────────────────────────────────

impl Render for BrowserWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let is_active = self.command_bar_active;

        // Root: key bindings + focus tracking always active so cmd+\ fires globally
        let mut root = div()
            .key_context("BrowserWindow")
            .track_focus(&self.command_bar_focus)
            .on_action(cx.listener(|this, _: &FocusCommandBar, window, cx| {
                this.activate_command_bar(window, cx);
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                this.handle_key_down(event, window, cx);
            }))
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x1e1e1e));

        // Chrome bar only visible when cmd+\ is open
        if is_active {
            let bar_text = self.command_bar_text.clone();
            let is_bar_focused = self.command_bar_focus.is_focused(window);

            // Current page info for the title row
            let active_tab = self.tabs.get(self.active_tab);
            let page_title = active_tab.map(|t| t.title.clone()).unwrap_or_else(|| "New Tab".to_string());
            let page_url = active_tab.map(|t| t.url.clone()).unwrap_or_default();
            let page_domain = domain_from_url(&page_url).to_string();

            // Hint: what pressing ↵ will do
            let hint: Option<String> = if !bar_text.is_empty() {
                if let Some((_, title)) = self.best_tab_match(&bar_text) {
                    Some(format!("↵  switch to: {title}"))
                } else {
                    match parse_command(&bar_text) {
                        BrowserAction::Navigate(url) => Some(format!("↵  navigate to {url}")),
                        BrowserAction::NewSearch { engine_name, query } => Some(format!("↵  {engine_name} — {query}")),
                        BrowserAction::Back => Some("↵  go back".to_string()),
                        BrowserAction::Forward => Some("↵  go forward".to_string()),
                        BrowserAction::Reload => Some("↵  reload".to_string()),
                        BrowserAction::CloseTab => Some("↵  close tab".to_string()),
                        BrowserAction::NewTab => Some("↵  new tab".to_string()),
                        _ => None,
                    }
                }
            } else {
                None
            };

            // Tab pills
            let mut tab_pills = div().flex().flex_row().items_center().gap_1();
            for (i, tab) in self.tabs.iter().enumerate() {
                let is_current = i == self.active_tab;
                let dot_color = match tab.state {
                    TabState::Active => rgb(0x28c840u32),
                    TabState::Cached => rgb(0xffbd2eu32),
                    TabState::Frozen => rgb(0x666666u32),
                };
                let short_title = if tab.title.len() > 18 {
                    format!("{}…", &tab.title[..15])
                } else {
                    tab.title.clone()
                };
                tab_pills = tab_pills.child(
                    div()
                        .id(ElementId::NamedInteger("tab-pill".into(), i as u64))
                        .flex().items_center().gap_1()
                        .px_2().py_1().rounded_md()
                        .bg(if is_current { rgb(0x383838u32) } else { rgb(0x252525u32) })
                        .border_1()
                        .border_color(if is_current { rgb(0x5588ccu32) } else { rgb(0x353535u32) })
                        .cursor_pointer()
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.switch_tab(i, window, cx);
                        }))
                        .child(div().w(px(6.0)).h(px(6.0)).rounded_full().bg(dot_color))
                        .child(
                            div()
                                .text_color(if is_current { rgb(0xffffffu32) } else { rgb(0x888888u32) })
                                .text_size(px(11.0))
                                .child(SharedString::from(short_title)),
                        ),
                );
            }

            // Title row: page title (hover → domain) + tab pills
            let title_row = div()
                .flex().flex_row().items_center().gap_2()
                .px_3().h(px(CHROME_HEIGHT))
                .child(
                    div()
                        .id("page-title")
                        .flex().flex_1().items_center()
                        .px_2().py_1().rounded_md()
                        .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                            this.title_hovered = *hovered;
                            cx.notify();
                        }))
                        .child(
                            div()
                                .text_color(if self.title_hovered { rgb(0x7aabccu32) } else { rgb(0x999999u32) })
                                .text_size(px(12.0))
                                .child(SharedString::from(if self.title_hovered { page_domain } else { page_title })),
                        ),
                )
                .child(tab_pills);

            // Input row: text field with cursor
            let input_row = div()
                .flex().flex_row().items_center()
                .px_3().h(px(CHROME_HEIGHT))
                .child(
                    div()
                        .flex_1().px_3().py_1().rounded_md()
                        .bg(rgb(0x1a1a1a))
                        .border_1()
                        .border_color(if is_bar_focused { rgb(0x5588cc) } else { rgb(0x404040) })
                        .child(
                            div()
                                .text_color(rgb(0xffffff))
                                .text_size(px(13.0))
                                .child(SharedString::from(format!("{bar_text}▌"))),
                        ),
                );

            // Build chrome: title row + input row + optional hint
            let mut chrome = div()
                .flex().flex_col().w_full()
                .bg(rgb(0x2a2a2a))
                .border_b_1()
                .border_color(rgb(0x383838))
                .child(title_row)
                .child(input_row);

            if let Some(hint_text) = hint {
                chrome = chrome.child(
                    div()
                        .px_3().pb_2()
                        .text_color(rgb(0x5588cc))
                        .text_size(px(11.0))
                        .child(SharedString::from(hint_text)),
                );
            }

            root = root.child(chrome);
        }

        root.child(
            div()
                .flex_1()
                .w_full()
                .child(WebViewElement { browser: cx.entity().downgrade() }),
        )
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

            let scale = window.scale_factor() as f64;
            // viewport_size is in physical pixels; divide by scale to get logical points
            let viewport_h = f64::from(window.viewport_size().height) / scale;

            // bounds are in physical pixels, top-left origin (GPUI convention)
            let gpui_x = f64::from(bounds.origin.x);
            let gpui_y = f64::from(bounds.origin.y);
            let gpui_w = f64::from(bounds.size.width);
            let gpui_h = f64::from(bounds.size.height);

            // Convert to NSView logical points with flipped Y axis (bottom-left origin)
            let x = gpui_x / scale;
            let width = gpui_w / scale;
            let height = gpui_h / scale;
            let y = viewport_h - (gpui_y / scale) - height; // flip

            if let Some(browser) = self.browser.upgrade() {
                browser.update(cx, |bw, _cx| {
                    if let Some(tab) = bw.tabs.get_mut(bw.active_tab) {
                        if let Some(wv) = &tab.webview_handle {
                            wv.set_frame(x, y, width, height);
                            wv.show();
                            // Opportunistic title sync from WKWebView
                            if let Some(title) = wv.title() {
                                if !title.is_empty() {
                                    tab.title = title;
                                }
                            }
                        } else if let Some(wv) = WebViewHandle::create(ns_view, x, y, width, height) {
                            wv.navigate(&tab.url);
                            tab.webview_handle = Some(wv);
                        }
                    }
                });
            }
        }
    }
}

// ─── Entry point ──────────────────────────────────────────────────────────────

pub fn open_browser_window(cx: &mut App) {
    cx.bind_keys([KeyBinding::new("cmd-\\", FocusCommandBar, Some("BrowserWindow"))]);

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

        // Unknown engine falls back to perplexity
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
