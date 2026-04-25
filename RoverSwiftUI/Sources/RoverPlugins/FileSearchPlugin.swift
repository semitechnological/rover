import Foundation
import RoverCore

/// Spotlight-powered file search via `MDQuery`.
@MainActor
public final class FileSearchPlugin: RoverPlugin {
    public let id = "builtin.file_search"
    public let name = "File Search"

    private var currentQuery: MDQuery?
    private var latestResults: [PickerItem] = []

    public init() {}

    public func query(_ query: String) -> [PickerItem] {
        // Activate on explicit "/" prefix or "file:" prefix.
        guard query.hasPrefix("/") || query.lowercased().hasPrefix("file:") else {
            return []
        }
        let term = query
            .replacingOccurrences(of: "^file:", with: "", options: .regularExpression)
            .replacingOccurrences(of: "^/", with: "", options: .regularExpression)
            .trimmingCharacters(in: .whitespaces)

        guard term.count >= 2 else { return [] }

        runSpotlight(term)
        return latestResults
    }

    public func confirm(itemID: String, query: String, secondary: Bool) -> ConfirmAction {
        // Open the file with the default app.
        return .runShell("open \"\(itemID)\"")
    }

    // MARK: - Spotlight

    private func runSpotlight(_ term: String) {
        currentQuery.map { MDQueryStop($0) }

        let predicate = "kMDItemDisplayName == '*\(term)*'cdw"
        guard let mdQuery = MDQueryCreate(nil, predicate as CFString, nil, nil) else { return }

        MDQuerySetMaxCount(mdQuery, 12)
        MDQueryExecute(mdQuery, CFOptionFlags(kMDQuerySynchronous.rawValue))

        let count = MDQueryGetResultCount(mdQuery)
        var items: [PickerItem] = []
        for i in 0 ..< count {
            guard let rawPtr = MDQueryGetResultAtIndex(mdQuery, i) else { continue }
            let item = Unmanaged<MDItem>.fromOpaque(rawPtr).takeUnretainedValue()
            guard
                let path = MDItemCopyAttribute(item, kMDItemPath) as? String,
                let name = MDItemCopyAttribute(item, kMDItemDisplayName) as? String
            else { continue }
            items.append(PickerItem(
                id: path,
                pluginID: id,
                title: name,
                subtitle: path,
                iconName: "doc",
                score: 0.5
            ))
        }

        currentQuery = mdQuery
        latestResults = items
    }
}
