import Foundation
import RoverCore

@MainActor
public final class WebSearchPlugin: RoverPlugin {
    public let id = "builtin.web_search"
    public let name = "Web Search"

    public init() {}

    public func query(_ query: String) -> [PickerItem] {
        let trimmed = query.trimmingCharacters(in: .whitespaces)
        guard !trimmed.isEmpty else { return [] }

        let match = Self.parse(trimmed)
        guard let match else { return [] }

        let url = Self.url(for: match.term, engine: match.engine)
        return [
            PickerItem(
                id: "search:\(url.absoluteString)",
                pluginID: id,
                title: "Search \(match.label)",
                subtitle: match.term,
                iconName: "magnifyingglass",
                score: 0.88
            ),
        ]
    }

    public func confirm(itemID: String, query: String, secondary: Bool) -> ConfirmAction {
        guard itemID.hasPrefix("search:"),
            let url = URL(string: String(itemID.dropFirst(7)))
        else { return .dismiss }
        return .openURL(url)
    }

    private static func parse(_ query: String) -> (engine: String, label: String, term: String)? {
        let lower = query.lowercased()
        let prefixes: [(String, String, String)] = [
            ("search ", "google", "Google"),
            ("web ", "google", "Google"),
            ("google ", "google", "Google"),
            ("ddg ", "duckduckgo", "DuckDuckGo"),
            ("duckduckgo ", "duckduckgo", "DuckDuckGo"),
            ("bing ", "bing", "Bing"),
            ("youtube ", "youtube", "YouTube"),
            ("yt ", "youtube", "YouTube"),
            ("wiki ", "wikipedia", "Wikipedia"),
            ("wikipedia ", "wikipedia", "Wikipedia"),
            ("github ", "github", "GitHub"),
            ("reddit ", "reddit", "Reddit"),
            ("amazon ", "amazon", "Amazon"),
        ]

        for (prefix, engine, label) in prefixes where lower.hasPrefix(prefix) {
            let term = String(query.dropFirst(prefix.count)).trimmingCharacters(in: .whitespaces)
            return term.isEmpty ? nil : (engine, label, term)
        }

        return nil
    }

    private static func url(for term: String, engine: String) -> URL {
        let encoded = term.addingPercentEncoding(withAllowedCharacters: .urlQueryAllowed) ?? term
        let value: String
        switch engine {
        case "duckduckgo":
            value = "https://duckduckgo.com/?q=\(encoded)"
        case "bing":
            value = "https://www.bing.com/search?q=\(encoded)"
        case "youtube":
            value = "https://www.youtube.com/results?search_query=\(encoded)"
        case "wikipedia":
            value = "https://en.wikipedia.org/w/index.php?search=\(encoded)"
        case "github":
            value = "https://github.com/search?q=\(encoded)"
        case "reddit":
            value = "https://www.reddit.com/search/?q=\(encoded)"
        case "amazon":
            value = "https://www.amazon.com/s?k=\(encoded)"
        default:
            value = "https://www.google.com/search?q=\(encoded)"
        }
        return URL(string: value)!
    }
}
