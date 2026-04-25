import Combine
import RoverCore
import RoverPlugins
import SwiftUI

/// Drives the launcher panel. Lives on the main actor.
@MainActor
@Observable
final class LauncherViewModel {
    var query: String = "" {
        didSet { runQuery() }
    }

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
        guard items.indices.contains(selectedIndex) else { return nil }
        return items[selectedIndex]
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
            NotificationCenter.default.post(name: .roverDismiss, object: nil)

        case .openURL(let url):
            NSWorkspace.shared.open(url)
            NotificationCenter.default.post(name: .roverDismiss, object: nil)

        case .launchApp(let name):
            NSWorkspace.shared.launchApplication(name)
            NotificationCenter.default.post(name: .roverDismiss, object: nil)

        case .runShell(let cmd):
            Process.launchedProcess(launchPath: "/bin/sh", arguments: ["-c", cmd])
            NotificationCenter.default.post(name: .roverDismiss, object: nil)

        case .copyToClipboard(let text):
            NSPasteboard.general.clearContents()
            NSPasteboard.general.setString(text, forType: .string)
            NotificationCenter.default.post(name: .roverDismiss, object: nil)

        case .openTerminal(let cmd):
            let script = cmd.map { "tell application \"Terminal\" to do script \"\($0)\"" }
                ?? "tell application \"Terminal\" to activate"
            var error: NSDictionary?
            NSAppleScript(source: script)?.executeAndReturnError(&error)
            NotificationCenter.default.post(name: .roverDismiss, object: nil)

        case .openSettings:
            NotificationCenter.default.post(name: .roverOpenSettings, object: nil)

        case .enterSubmenu(let title, _):
            // Replace query with submenu title so UX feels natural.
            query = title

        case .stay:
            break
        }
    }
}

extension Notification.Name {
    static let roverDismiss = Notification.Name("dev.undivisible.rover.dismiss")
    static let roverOpenSettings = Notification.Name("dev.undivisible.rover.openSettings")
    static let roverToggle = Notification.Name("dev.undivisible.rover.toggle")
}
