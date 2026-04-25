import Foundation
import RoverCore

private struct EmojiEntry {
    let char: String
    let name: String
}

/// Emoji search — activated by `:` prefix.
@MainActor
public final class EmojiPlugin: RoverPlugin {
    public let id = "builtin.emoji"
    public let name = "Emoji"

    // A representative subset; a full list can replace this.
    private static let emojis: [EmojiEntry] = [
        EmojiEntry(char: "😀", name: "grinning face"),
        EmojiEntry(char: "😂", name: "face with tears of joy"),
        EmojiEntry(char: "❤️", name: "red heart"),
        EmojiEntry(char: "👍", name: "thumbs up"),
        EmojiEntry(char: "🔥", name: "fire"),
        EmojiEntry(char: "✨", name: "sparkles"),
        EmojiEntry(char: "🎉", name: "party popper"),
        EmojiEntry(char: "🚀", name: "rocket"),
        EmojiEntry(char: "🌟", name: "glowing star"),
        EmojiEntry(char: "💯", name: "hundred points"),
        EmojiEntry(char: "🤔", name: "thinking face"),
        EmojiEntry(char: "👀", name: "eyes"),
        EmojiEntry(char: "🙌", name: "raising hands"),
        EmojiEntry(char: "💪", name: "flexed biceps"),
        EmojiEntry(char: "🎯", name: "bullseye"),
        EmojiEntry(char: "🐛", name: "bug"),
        EmojiEntry(char: "🦀", name: "crab"),
        EmojiEntry(char: "🐍", name: "snake"),
        EmojiEntry(char: "⚡", name: "high voltage"),
        EmojiEntry(char: "🔑", name: "key"),
    ]

    public init() {}

    public func query(_ query: String) -> [PickerItem] {
        guard query.hasPrefix(":") else { return [] }
        let term = String(query.dropFirst()).lowercased()
        guard !term.isEmpty else {
            return Self.emojis.prefix(8).enumerated().map { item(index: $0, entry: $1) }
        }

        return Self.emojis
            .filter { $0.name.contains(term) }
            .prefix(8)
            .enumerated()
            .map { item(index: $0, entry: $1) }
    }

    public func confirm(itemID: String, query: String, secondary: Bool) -> ConfirmAction {
        return .copyToClipboard(itemID)
    }

    private func item(index: Int, entry: EmojiEntry) -> PickerItem {
        PickerItem(
            id: entry.char,
            pluginID: id,
            title: "\(entry.char)  \(entry.name)",
            iconName: nil,
            score: 1.0 - Double(index) * 0.01
        )
    }
}
