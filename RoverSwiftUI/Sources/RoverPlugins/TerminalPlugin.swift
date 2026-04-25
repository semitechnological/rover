import Foundation
import RoverCore

/// Shell command runner — activated by `>` or `!` prefix.
@MainActor
public final class TerminalPlugin: RoverPlugin {
    public let id = "builtin.terminal"
    public let name = "Terminal"

    public init() {}

    public func query(_ query: String) -> [PickerItem] {
        guard query.hasPrefix(">") || query.hasPrefix("!") else { return [] }
        let cmd = String(query.dropFirst()).trimmingCharacters(in: .whitespaces)
        guard !cmd.isEmpty else { return [] }

        return [
            PickerItem(
                id: cmd,
                pluginID: id,
                title: cmd,
                subtitle: "Run in terminal",
                iconName: "terminal",
                score: 1.0
            )
        ]
    }

    public func confirm(itemID: String, query: String, secondary: Bool) -> ConfirmAction {
        return .runShell(itemID)
    }
}
