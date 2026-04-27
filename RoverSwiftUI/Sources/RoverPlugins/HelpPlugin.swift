import AppKit
import RoverCore

public final class HelpPlugin: RoverPlugin {
    public let id = "builtin.help"
    public let name = "Help"

    // topic id → (title, icon, explanation)
    private let topics: [(String, String, String, String)] = [
        ("calculator", "Calculator", "function", "Type a math expression like `2^10`, `sqrt(144)`, or `(1+2)*3`. The result appears instantly in the card above results. Press ↵ to copy it."),
        ("ai",         "AI Assistant", "sparkles", "Type `? your question` or `ai your question` to ask Apple Intelligence. It can answer questions, run shell commands, check your calendar, weather, and more."),
        ("terminal",   "Terminal", "terminal", "Prefix `>` or `!` to run a shell command. Example: `> ls ~/Desktop`. An embedded terminal window opens with the command pre-filled."),
        ("files",      "File Search", "folder.badge.magnifyingglass", "Prefix `/` or `file:` to search your files with Spotlight. Example: `/report.pdf`. Press ↵ to reveal in Finder."),
        ("clipboard",  "Clipboard History", "doc.on.clipboard", "Search your clipboard history by typing any part of a previous copy. Press ↵ to paste it, ⌘⌫ to delete the entry."),
        ("hotkeys",    "Keyboard Shortcuts", "keyboard", "↑↓ navigate • Tab/Shift+Tab cycle • ↵ confirm • ⌘↵ secondary action • ⌘1–9 pick by number • ⌘K actions panel • Esc dismiss"),
        ("browser",    "Browser", "globe", "The built-in WebKit browser opens for any URL. Type a domain or paste a link. Inside: ⌘L for address bar, ⌘[ / ⌘] for back/forward."),
        ("semantic",   "Semantic Search", "brain", "Prefix `~` to search by meaning rather than keywords. Powered by on-device AI embeddings (AllMiniLML6V2). Example: `~ copy text to clipboard`"),
    ]

    public init() {}

    public func onLoad() {}

    public func query(_ query: String) -> [PickerItem] {
        let q = query.lowercased()
        let showAll = q == "help" || q == "help "
        let prefix = q.hasPrefix("help ") ? String(q.dropFirst(5)) : nil

        guard showAll || prefix != nil else { return [] }

        return topics.compactMap { id, title, icon, _ in
            if let p = prefix, !title.lowercased().contains(p), !id.contains(p) { return nil }
            return PickerItem(
                id: "help.\(id)",
                pluginID: self.id,
                title: title,
                subtitle: "Help — Rover",
                iconName: icon,
                score: showAll ? 0.6 : 0.9
            )
        }
    }

    public func confirm(itemID: String, query: String, secondary: Bool) -> ConfirmAction {
        let topicID = itemID.replacingOccurrences(of: "help.", with: "")
        guard let topic = topics.first(where: { $0.0 == topicID }) else { return .dismiss }
        // Return .stay so LauncherView shows the explanation as an inline card
        // (The card display needs to be driven by the AI plugin's card mechanism;
        //  for now, copy the explanation to clipboard as a quick win)
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(topic.3, forType: .string)
        return .stay
    }

    public func inlineCard(for query: String) -> InlineCard? {
        let q = query.lowercased()
        guard q.hasPrefix("help "), q.count > 5 else { return nil }
        let term = String(q.dropFirst(5))
        guard let topic = topics.first(where: { $0.0.contains(term) || $0.1.lowercased().contains(term) }) else { return nil }
        return InlineCard(kind: .info, content: "\(topic.1)\n\n\(topic.3)")
    }
}
