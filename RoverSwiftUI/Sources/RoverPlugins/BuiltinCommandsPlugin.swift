import AppKit
import Foundation
import RoverCore

/// Always-available Rover system commands: Settings, Terminal, Browser, Quit.
///
/// Surfaces on empty query (score 0.8) and boosts to 1.0 when the query
/// matches a command keyword.
@MainActor
public final class BuiltinCommandsPlugin: RoverPlugin {
    public let id = "builtin.commands"
    public let name = "Rover Commands"

    public init() {}

    private struct Command {
        let itemID: String
        let title: String
        let subtitle: String
        let iconName: String
        let keywords: [String]
        let action: ConfirmAction
    }

    private let commands: [Command] = [
        Command(
            itemID: "rover.settings",
            title: "Rover Settings",
            subtitle: "Open Rover preferences",
            iconName: "gear",
            keywords: ["settings", "preferences", "config", "rover"],
            action: .openSettings
        ),
        Command(
            itemID: "rover.terminal",
            title: "Open Terminal",
            subtitle: "Launch Terminal.app",
            iconName: "terminal",
            keywords: ["terminal", "shell", "console", "rover"],
            action: .openTerminal(initialCommand: nil)
        ),
        Command(
            itemID: "rover.browser",
            title: "Open Browser",
            subtitle: "Open Rover's built-in WebKit browser",
            iconName: "globe",
            keywords: ["browser", "safari", "web", "rover"],
            action: .openBrowser(URL(string: "https://")!)
        ),
        Command(
            itemID: "rover.quit",
            title: "Quit Rover",
            subtitle: "Exit the launcher",
            iconName: "power",
            keywords: ["quit", "exit", "close", "rover"],
            action: .dismiss
        ),
    ]

    public func query(_ query: String) -> [PickerItem] {
        let q = query.lowercased().trimmingCharacters(in: .whitespaces)
        if q.isEmpty,
           UserDefaults.standard.object(forKey: "rover.show_builtin_commands_on_empty") != nil,
           !UserDefaults.standard.bool(forKey: "rover.show_builtin_commands_on_empty")
        {
            return []
        }
        return commands.compactMap { cmd -> PickerItem? in
            let score: Double
            if q.isEmpty {
                score = 0.8
            } else {
                let matched = cmd.keywords.contains { $0.hasPrefix(q) }
                    || cmd.title.lowercased().contains(q)
                guard matched else { return nil }
                score = cmd.keywords.first?.hasPrefix(q) == true ? 1.0 : 0.85
            }
            return PickerItem(
                id: cmd.itemID,
                pluginID: id,
                title: cmd.title,
                subtitle: cmd.subtitle,
                iconName: cmd.iconName,
                score: score
            )
        }
    }

    public func confirm(itemID: String, query: String, secondary: Bool) -> ConfirmAction {
        if itemID == "rover.quit" {
            NSApp.terminate(nil)
            return .dismiss
        }
        return commands.first { $0.itemID == itemID }?.action ?? .dismiss
    }
}
