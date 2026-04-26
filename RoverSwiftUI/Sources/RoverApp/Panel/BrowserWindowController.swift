import AppKit
import WebKit

// MARK: - BrowserWindowController

/// Rover's built-in WebKit browser window.
///
/// Mirrors rover_browser's core features: address bar, navigation buttons,
/// title tracking. One shared window — reused across invocations.
@MainActor
final class BrowserWindowController: NSWindowController {

    // MARK: - Singleton

    static let shared = BrowserWindowController()

    // MARK: - UI

    private let webView: WKWebView
    private let addressField = NSTextField()
    private let backButton    = NSButton()
    private let forwardButton = NSButton()
    private let reloadButton  = NSButton()
    private let progressBar   = NSProgressIndicator()

    // MARK: - Init

    private init() {
        let config = WKWebViewConfiguration()
        webView = WKWebView(frame: .zero, configuration: config)

        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 1100, height: 720),
            styleMask: [.titled, .closable, .resizable, .miniaturizable, .fullSizeContentView],
            backing: .buffered,
            defer: false
        )
        window.title = "Rover Browser"
        window.center()
        window.setFrameAutosaveName("RoverBrowserWindow")

        super.init(window: window)

        buildUI()
    }

    required init?(coder: NSCoder) { nil }

    // MARK: - Public API

    /// Open the browser and navigate to `url`. Pass `nil` to show the last page.
    func navigate(to url: URL?) {
        showWindow(nil)
        window?.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)

        guard let url else { return }
        // Blank placeholder — show empty address bar ready to type.
        if url.absoluteString == "https://" {
            addressField.stringValue = ""
            addressField.selectText(nil)
            window?.makeFirstResponder(addressField)
            return
        }
        load(url)
    }

    // MARK: - Private

    private func load(_ url: URL) {
        addressField.stringValue = url.absoluteString
        webView.load(URLRequest(url: url))
    }

    private func buildUI() {
        guard let window else { return }

        // ── Toolbar ──────────────────────────────────────────────────────────
        let toolbar = NSToolbar(identifier: "RoverBrowserToolbar")
        toolbar.delegate = self
        toolbar.displayMode = .iconOnly
        toolbar.allowsUserCustomization = false
        window.toolbar = toolbar
        window.titleVisibility = .hidden
        window.titlebarAppearsTransparent = false

        // ── Progress bar (under toolbar) ─────────────────────────────────────
        progressBar.style = .bar
        progressBar.isIndeterminate = false
        progressBar.minValue = 0
        progressBar.maxValue = 1
        progressBar.isHidden = true

        // ── WebView fills the window ──────────────────────────────────────────
        webView.navigationDelegate = self
        webView.translatesAutoresizingMaskIntoConstraints = false

        let container = NSView()
        container.addSubview(progressBar)
        container.addSubview(webView)
        progressBar.translatesAutoresizingMaskIntoConstraints = false

        NSLayoutConstraint.activate([
            progressBar.topAnchor.constraint(equalTo: container.topAnchor),
            progressBar.leadingAnchor.constraint(equalTo: container.leadingAnchor),
            progressBar.trailingAnchor.constraint(equalTo: container.trailingAnchor),
            progressBar.heightAnchor.constraint(equalToConstant: 2),

            webView.topAnchor.constraint(equalTo: progressBar.bottomAnchor),
            webView.leadingAnchor.constraint(equalTo: container.leadingAnchor),
            webView.trailingAnchor.constraint(equalTo: container.trailingAnchor),
            webView.bottomAnchor.constraint(equalTo: container.bottomAnchor),
        ])

        window.contentView = container

        // ── KVO for title and load progress ──────────────────────────────────
        webView.addObserver(self, forKeyPath: #keyPath(WKWebView.title), options: .new, context: nil)
        webView.addObserver(self, forKeyPath: #keyPath(WKWebView.estimatedProgress), options: .new, context: nil)
        webView.addObserver(self, forKeyPath: #keyPath(WKWebView.canGoBack), options: .new, context: nil)
        webView.addObserver(self, forKeyPath: #keyPath(WKWebView.canGoForward), options: .new, context: nil)
    }

    override func observeValue(
        forKeyPath keyPath: String?,
        of object: Any?,
        change: [NSKeyValueChangeKey: Any]?,
        context: UnsafeMutableRawPointer?
    ) {
        switch keyPath {
        case #keyPath(WKWebView.title):
            window?.title = webView.title ?? "Rover Browser"
            if let url = webView.url {
                addressField.stringValue = url.absoluteString
            }
        case #keyPath(WKWebView.estimatedProgress):
            let p = webView.estimatedProgress
            progressBar.doubleValue = p
            progressBar.isHidden = p >= 1.0
        case #keyPath(WKWebView.canGoBack):
            backButton.isEnabled = webView.canGoBack
        case #keyPath(WKWebView.canGoForward):
            forwardButton.isEnabled = webView.canGoForward
        default:
            super.observeValue(forKeyPath: keyPath, of: object, change: change, context: context)
        }
    }

    // MARK: - Actions

    @objc private func goBack()    { webView.goBack() }
    @objc private func goForward() { webView.goForward() }

    @objc private func reloadOrStop() {
        if webView.isLoading {
            webView.stopLoading()
        } else {
            webView.reload()
        }
    }

    @objc private func commitAddress() {
        var raw = addressField.stringValue.trimmingCharacters(in: .whitespaces)
        guard !raw.isEmpty else { return }
        // Add scheme if missing.
        if !raw.hasPrefix("http://") && !raw.hasPrefix("https://") {
            // Looks like a search query — send to DuckDuckGo.
            if !raw.contains(".") || raw.contains(" ") {
                let encoded = raw.addingPercentEncoding(withAllowedCharacters: .urlQueryAllowed) ?? raw
                raw = "https://duckduckgo.com/?q=\(encoded)"
            } else {
                raw = "https://\(raw)"
            }
        }
        if let url = URL(string: raw) { load(url) }
    }
}

// MARK: - WKNavigationDelegate

extension BrowserWindowController: WKNavigationDelegate {
    nonisolated func webView(_ webView: WKWebView, didStartProvisionalNavigation _: WKNavigation!) {
        Task { @MainActor in
            self.reloadButton.image = NSImage(systemSymbolName: "xmark", accessibilityDescription: "Stop")
        }
    }

    nonisolated func webView(_ webView: WKWebView, didFinish _: WKNavigation!) {
        Task { @MainActor in
            self.reloadButton.image = NSImage(systemSymbolName: "arrow.clockwise", accessibilityDescription: "Reload")
        }
    }

    nonisolated func webView(_ webView: WKWebView, didFail _: WKNavigation!, withError _: Error) {
        Task { @MainActor in
            self.reloadButton.image = NSImage(systemSymbolName: "arrow.clockwise", accessibilityDescription: "Reload")
        }
    }
}

// MARK: - NSToolbarDelegate

extension BrowserWindowController: NSToolbarDelegate {

    private static let backID    = NSToolbarItem.Identifier("back")
    private static let forwardID = NSToolbarItem.Identifier("forward")
    private static let reloadID  = NSToolbarItem.Identifier("reload")
    private static let addressID = NSToolbarItem.Identifier("address")

    func toolbar(
        _ toolbar: NSToolbar,
        itemForItemIdentifier identifier: NSToolbarItem.Identifier,
        willBeInsertedIntoToolbar _: Bool
    ) -> NSToolbarItem? {
        switch identifier {
        case Self.backID:
            backButton.image = NSImage(systemSymbolName: "chevron.left", accessibilityDescription: "Back")
            backButton.bezelStyle = .toolbar
            backButton.isBordered = false
            backButton.target = self
            backButton.action = #selector(goBack)
            backButton.isEnabled = false
            let item = NSToolbarItem(itemIdentifier: identifier)
            item.view = backButton
            item.label = "Back"
            return item

        case Self.forwardID:
            forwardButton.image = NSImage(systemSymbolName: "chevron.right", accessibilityDescription: "Forward")
            forwardButton.bezelStyle = .toolbar
            forwardButton.isBordered = false
            forwardButton.target = self
            forwardButton.action = #selector(goForward)
            forwardButton.isEnabled = false
            let item = NSToolbarItem(itemIdentifier: identifier)
            item.view = forwardButton
            item.label = "Forward"
            return item

        case Self.reloadID:
            reloadButton.image = NSImage(systemSymbolName: "arrow.clockwise", accessibilityDescription: "Reload")
            reloadButton.bezelStyle = .toolbar
            reloadButton.isBordered = false
            reloadButton.target = self
            reloadButton.action = #selector(reloadOrStop)
            let item = NSToolbarItem(itemIdentifier: identifier)
            item.view = reloadButton
            item.label = "Reload"
            return item

        case Self.addressID:
            addressField.placeholderString = "Search or enter address"
            addressField.bezelStyle = .roundedBezel
            addressField.target = self
            addressField.action = #selector(commitAddress)
            addressField.setContentHuggingPriority(.defaultLow, for: .horizontal)
            let item = NSToolbarItem(itemIdentifier: identifier)
            item.view = addressField
            item.label = "Address"
            item.visibilityPriority = .high
            addressField.widthAnchor.constraint(greaterThanOrEqualToConstant: 300).isActive = true
            return item

        default:
            return nil
        }
    }

    func toolbarDefaultItemIdentifiers(_ toolbar: NSToolbar) -> [NSToolbarItem.Identifier] {
        [Self.backID, Self.forwardID, Self.reloadID, .flexibleSpace, Self.addressID, .flexibleSpace]
    }

    func toolbarAllowedItemIdentifiers(_ toolbar: NSToolbar) -> [NSToolbarItem.Identifier] {
        [Self.backID, Self.forwardID, Self.reloadID, Self.addressID, .flexibleSpace, .space]
    }
}
