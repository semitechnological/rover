import Foundation
import RoverCore

/// Browser launcher and URL opener.
///
/// Three modes:
///   - `http://` / `https://` prefix — opens the URL directly
///   - Query with `.com`, `.io`, `.dev`, `.org` etc — treats as URL
///   - Plain query matching "safari" / "browser" — shows "Open Browser" item
@MainActor
public final class BrowserPlugin: RoverPlugin {
    public let id = "builtin.browser"
    public let name = "Browser"

    private static let tldPattern = #/\.\w{2,6}(?:/|$)/#

    public init() {}

    public func query(_ query: String) -> [PickerItem] {
        let trimmed = query.trimmingCharacters(in: .whitespaces)
        guard !trimmed.isEmpty else { return [] }

        // Direct URL.
        if let url = urlFromQuery(trimmed) {
            return [
                PickerItem(
                    id: "url:\(url.absoluteString)",
                    pluginID: id,
                    title: "Open \(url.host ?? trimmed)",
                    subtitle: url.absoluteString,
                    iconName: "safari",
                    score: 0.95
                ),
            ]
        }

        // "browser" or "safari" keyword.
        let q = trimmed.lowercased()
        if "browser".hasPrefix(q) || "safari".hasPrefix(q) {
            return [
                PickerItem(
                    id: "__open_browser__",
                    pluginID: id,
                    title: "Browser",
                    subtitle: "Open Safari",
                    iconName: "safari",
                    score: 0.8
                ),
            ]
        }

        return []
    }

    public func confirm(itemID: String, query: String, secondary: Bool) -> ConfirmAction {
        if itemID == "__open_browser__" {
            return .openURL(URL(string: "https://")!)
        }
        if itemID.hasPrefix("url:"),
            let url = URL(string: String(itemID.dropFirst(4)))
        {
            return .openURL(url)
        }
        return .dismiss
    }

    // MARK: - URL detection

    private func urlFromQuery(_ query: String) -> URL? {
        // Already has scheme.
        if query.hasPrefix("http://") || query.hasPrefix("https://") {
            return URL(string: query)
        }
        // Looks like a domain (contains a TLD pattern).
        if query.contains(Self.tldPattern) {
            return URL(string: "https://\(query)")
        }
        return nil
    }
}
