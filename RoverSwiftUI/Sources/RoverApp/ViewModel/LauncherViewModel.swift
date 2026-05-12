import AppKit
import RoverCore
import SwiftUI

/// Drives the launcher panel. All state lives here; views are stateless renderers.
@MainActor
@Observable
final class LauncherViewModel {
    var query: String = "" { didSet { scheduleQuery() } }
    var items: [PickerItem] = []
    var cards: [InlineCard] = []
    var selectedIndex: Int = 0

    private let registry: PluginRegistry

    // ── Debounce ──────────────────────────────────────────────────────────────
    // Mirrors the main Rover app's input coalescing: fire immediately on empty
    // query (instant clear), otherwise wait 60 ms so rapid keystrokes coalesce
    // into one query pass — particularly important when AI streaming is active.
    private var debounceTask: Task<Void, Never>?
    private static let debounceNs: UInt64 = 60_000_000  // 60 ms

    init(registry: PluginRegistry) {
        self.registry = registry

        // AI streaming update — only re-query AI + Semantic plugins (not all 10).
        for name in [Notification.Name.roverAIUpdate, .roverSemanticUpdate] {
            NotificationCenter.default.addObserver(
                forName: name, object: nil, queue: .main
            ) { [weak self] _ in
                MainActor.assumeIsolated {
                    self?.runQuery(pluginIDs: ["builtin.ai", "builtin.semantic"])
                }
            }
        }

        // File search result ready — only re-query file search plugin.
        NotificationCenter.default.addObserver(
            forName: .roverFileSearchUpdate, object: nil, queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated {
                self?.runQuery(pluginIDs: ["builtin.file_search"])
            }
        }
    }

    // MARK: - Query

    private func scheduleQuery() {
        // Empty query → execute immediately so the panel clears without lag.
        if query.isEmpty {
            debounceTask?.cancel()
            runQuery()
            return
        }
        debounceTask?.cancel()
        debounceTask = Task { [weak self] in
            try? await Task.sleep(nanoseconds: Self.debounceNs)
            guard !Task.isCancelled else { return }
            self?.runQuery()
        }
    }

    /// Run all plugins against the current query and update results.
    /// - Parameter pluginIDs: If non-nil, only re-run the listed plugin IDs and
    ///   merge their fresh results into the existing list (cheaper partial refresh).
    func runQuery(pluginIDs: [String]? = nil) {
        if let ids = pluginIDs {
            // Partial refresh: keep results from other plugins, replace these ones.
            let (freshItems, freshCards) = registry.query(query, pluginIDs: ids)
            let kept = items.filter { !ids.contains($0.pluginID) }
            let merged = (kept + freshItems).sorted { $0.score > $1.score }
            items = merged
            // Replace cards from the specified plugins.
            let keptCards = cards.filter { card in
                !freshCards.contains { $0.kind == card.kind }
            }
            cards = keptCards + freshCards
        } else {
            let (newItems, newCards) = registry.query(query)
            items = newItems
            cards = newCards
            selectedIndex = 0
        }
    }

    // MARK: - Selection

    var selectedItem: PickerItem? {
        items.indices.contains(selectedIndex) ? items[selectedIndex] : nil
    }

    func moveDown() {
        guard !items.isEmpty else { return }
        selectedIndex = min(selectedIndex + 1, items.count - 1)
    }

    func moveUp() {
        selectedIndex = max(selectedIndex - 1, 0)
    }

    // MARK: - Confirm

    @discardableResult
    func confirm(secondary: Bool = false) -> ConfirmAction {
        guard let item = selectedItem else { return .dismiss }
        let action = registry.confirm(
            pluginID: item.pluginID,
            itemID: item.id,
            query: query,
            secondary: secondary
        )
        execute(action)
        return action
    }

    // MARK: - Action execution

    private func execute(_ action: ConfirmAction) {
        switch action {
        case .dismiss:
            dismiss()

        case .openURL(let url):
            NSWorkspace.shared.open(url)
            dismiss()

        case .openBrowser(let url):
            NotificationCenter.default.post(name: .roverOpenBrowser, object: url)
            dismiss()

        case .launchApp(let name):
            if let url = appURL(named: name) {
                NSWorkspace.shared.openApplication(at: url, configuration: .init())
            }
            dismiss()

        case .runShell(let cmd):
            let proc = Process()
            proc.executableURL = URL(fileURLWithPath: "/bin/sh")
            proc.arguments = ["-c", cmd]
            try? proc.run()
            dismiss()

        case .copyToClipboard(let text):
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(text, forType: .string)
            dismiss()

        case .openTerminal(let cmd):
            openTerminalApp(command: cmd)
            dismiss()

        case .openEditor(let mode):
            EditorWindowController.shared.show(mode: mode)
            dismiss()

        case .runTile(let command):
            runTile(command)
            dismiss()

        case .openSettings:
            NotificationCenter.default.post(name: .roverOpenSettings, object: nil)

        case .enterSubmenu(let title, _):
            query = title

        case .stay:
            // Re-run the query so the list reflects any state change (e.g. item deleted).
            runQuery()
        }
    }

    // MARK: - Helpers

    private func dismiss() {
        NotificationCenter.default.post(name: .roverDismiss, object: nil)
    }

    private func openTerminalApp(command: String?) {
        TerminalWindowController.shared.show(command: command)
    }

    private func runTile(_ command: TileCommand) {
        let tile = TileBackend()
        do {
            switch command {
            case .frontmost(let action):
                _ = try tile.applyToFrontmost(action: action)
            case .mosaic(let layout):
                _ = try tile.mosaicVisibleWindows(layout: layout)
            case .requestAccessibility:
                _ = tile.requestAccessibility()
            }
        } catch {
            let alert = NSAlert()
            alert.messageText = "Tile action failed"
            alert.informativeText = error.localizedDescription
            alert.alertStyle = .warning
            alert.runModal()
        }
    }

    private func appURL(named name: String) -> URL? {
        let dirs: [URL] = [
            URL(fileURLWithPath: "/Applications"),
            URL(fileURLWithPath: "/System/Applications"),
            FileManager.default.homeDirectoryForCurrentUser
                .appendingPathComponent("Applications"),
        ]
        for dir in dirs {
            let url = dir.appendingPathComponent("\(name).app")
            if FileManager.default.fileExists(atPath: url.path) { return url }
        }
        return nil
    }
}

// MARK: - Notification names

extension Notification.Name {
    static let roverDismiss = Notification.Name("dev.undivisible.rover.dismiss")
    static let roverOpenSettings = Notification.Name("dev.undivisible.rover.openSettings")
    static let roverOpenBrowser = Notification.Name("dev.undivisible.rover.openBrowser")
    static let roverToggle = Notification.Name("dev.undivisible.rover.toggle")
    static let roverHotkeyChanged = Notification.Name("dev.undivisible.rover.hotkeyChanged")
}
