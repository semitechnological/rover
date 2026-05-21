import Foundation
import RoverCore

/// Shell command runner and Terminal launcher.
///
/// Two modes:
///   - Prefix `>` or `!` — runs the command directly via `/bin/sh -c`
///   - Plain query matching "terminal" / "shell" — shows an "Open Terminal" item
@MainActor
public final class TerminalPlugin: RoverPlugin {
    public let id = "builtin.terminal"
    public let name = "Terminal"

    public init() {}

    public func query(_ query: String) -> [PickerItem] {
        let trimmed = query.trimmingCharacters(in: .whitespaces)

        // Prefix mode: run a shell command.
        if trimmed.hasPrefix(">") || trimmed.hasPrefix("!") {
            let cmd = String(trimmed.dropFirst()).trimmingCharacters(in: .whitespaces)
            guard !cmd.isEmpty else { return [] }
            return [
                PickerItem(
                    id: "run:\(cmd)",
                    pluginID: id,
                    title: cmd,
                    subtitle: "Run shell command",
                    iconName: "terminal",
                    score: 1.0
                ),
            ]
        }

        // Search mode: surface "Open Terminal" when query resembles the plugin name.
        let q = trimmed.lowercased()
        guard !q.isEmpty, ("terminal".hasPrefix(q) || "shell".hasPrefix(q)) else { return [] }
        return [
            PickerItem(
                id: "__open_terminal__",
                pluginID: id,
                title: "Terminal",
                subtitle: "Open embedded terminal",
                iconName: "terminal",
                score: 0.85
            ),
        ]
    }

    public func confirm(itemID: String, query: String, secondary: Bool) -> ConfirmAction {
        if itemID == "__open_terminal__" {
            return .openTerminal(initialCommand: nil)
        }
        // Strip the "run:" prefix to recover the raw command.
        let cmd = itemID.hasPrefix("run:") ? String(itemID.dropFirst(4)) : itemID
        return .runShell(cmd)
    }
}
