import AppKit
import RoverCore
import SwiftUI

/// Drives the launcher panel. All state lives here; views are stateless renderers.
@MainActor
@Observable
final class LauncherViewModel {
    var query: String = "" { didSet { runQuery() } }
    var items: [PickerItem] = []
    var cards: [InlineCard] = []
    var selectedIndex: Int = 0

    private let registry: PluginRegistry

    init(registry: PluginRegistry) {
        self.registry = registry
    }

    // MARK: - Query

    private func runQuery() {
        let (newItems, newCards) = registry.query(query)
        items = newItems
        cards = newCards
        selectedIndex = 0
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

        case .launchApp(let name):
            if let url = appURL(named: name) {
                NSWorkspace.shared.openApplication(at: url, configuration: .init())
            }
            dismiss()

        case .runShell(let cmd):
            // Process() replaces the deprecated launchedProcess(launchPath:arguments:).
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

        case .openSettings:
            NotificationCenter.default.post(name: .roverOpenSettings, object: nil)

        case .enterSubmenu(let title, _):
            query = title

        case .stay:
            break
        }
    }

    // MARK: - Helpers

    private func dismiss() {
        NotificationCenter.default.post(name: .roverDismiss, object: nil)
    }

    private func openTerminalApp(command: String?) {
        let script = command.map {
            "tell application \"Terminal\" to do script \"\($0)\""
        } ?? "tell application \"Terminal\" to activate"
        var err: NSDictionary?
        NSAppleScript(source: script)?.executeAndReturnError(&err)
    }

    /// Locate a .app bundle by display name in standard application directories.
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
    static let roverToggle = Notification.Name("dev.undivisible.rover.toggle")
}
