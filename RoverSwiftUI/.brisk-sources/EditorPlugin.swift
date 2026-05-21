import Foundation

@MainActor
public final class EditorPlugin: RoverPlugin {
    public let id = "builtin.editor"
    public let name = "Editor"

    private struct Command {
        let itemID: String
        let title: String
        let subtitle: String
        let iconName: String
        let keywords: [String]
        let mode: EditorMode
    }

    private let commands: [Command] = [
        Command(
            itemID: "editor.markdown",
            title: "Markdown Editor",
            subtitle: "Open the Rover markdown workspace",
            iconName: "doc.richtext",
            keywords: ["markdown", "md", "notes", "note", "editor", "write"],
            mode: .markdown
        ),
        Command(
            itemID: "editor.code",
            title: "Code Editor",
            subtitle: "Open a monospaced code scratchpad",
            iconName: "chevron.left.forwardslash.chevron.right",
            keywords: ["code", "scratch", "snippet", "editor"],
            mode: .code
        ),
        Command(
            itemID: "editor.text",
            title: "Text Editor",
            subtitle: "Open a plain text scratchpad",
            iconName: "text.alignleft",
            keywords: ["text", "plain", "draft", "editor"],
            mode: .text
        ),
    ]

    public init() {}

    public func query(_ query: String) -> [PickerItem] {
        let q = query.lowercased().trimmingCharacters(in: .whitespaces)
        guard !q.isEmpty else { return [] }

        return commands.compactMap { command in
            let matched = command.keywords.contains { $0.hasPrefix(q) }
                || command.title.lowercased().contains(q)
                || command.subtitle.lowercased().contains(q)
            guard matched else { return nil }
            let score = command.keywords.first?.hasPrefix(q) == true ? 0.94 : 0.82
            return PickerItem(
                id: command.itemID,
                pluginID: id,
                title: command.title,
                subtitle: command.subtitle,
                iconName: command.iconName,
                score: score
            )
        }
    }

    public func confirm(itemID: String, query: String, secondary: Bool) -> ConfirmAction {
        guard let command = commands.first(where: { $0.itemID == itemID }) else {
            return .dismiss
        }
        return .openEditor(command.mode)
    }
}
