import Foundation

/// Spotlight-powered file search via `MDQuery`.
///
/// Activated by `/` or `file:` prefix. Runs Spotlight asynchronously on a
/// background task so the main thread is never blocked.
@MainActor
public final class FileSearchPlugin: RoverPlugin {
    public let id = "builtin.file_search"
    public let name = "File Search"

    private var latestResults: [PickerItem] = []
    private var searchTask: Task<Void, Never>?

    public init() {}

    public func query(_ query: String) -> [PickerItem] {
        guard query.hasPrefix("/") || query.lowercased().hasPrefix("file:") else { return [] }
        let term = extractTerm(from: query)
        guard term.count >= 2 else { return [] }

        // Cancel any previous search and kick off a new one.
        searchTask?.cancel()
        searchTask = Task { [weak self] in
            let results = await Self.runSpotlight(term: term)
            guard !Task.isCancelled else { return }
            await MainActor.run {
                self?.latestResults = results
                // Notify the ViewModel to re-run the query now that fresh results are ready.
                NotificationCenter.default.post(name: .roverFileSearchUpdate, object: nil)
            }
        }

        // Return current (possibly one keystroke stale) results immediately.
        return latestResults
    }

    public func confirm(itemID: String, query: String, secondary: Bool) -> ConfirmAction {
        return .runShell("open \"\(itemID)\"")
    }

    // MARK: - Helpers

    private func extractTerm(from query: String) -> String {
        query
            .replacingOccurrences(of: "^file:", with: "", options: .regularExpression)
            .replacingOccurrences(of: "^/", with: "", options: .regularExpression)
            .trimmingCharacters(in: .whitespaces)
    }

    /// Runs `MDQueryExecute` off the main thread so we don't block the UI.
    private static func runSpotlight(term: String) async -> [PickerItem] {
        await Task.detached(priority: .userInitiated) {
            let predicate = "kMDItemDisplayName == '*\(term)*'cdw"
            guard let q = MDQueryCreate(nil, predicate as CFString, nil, nil) else { return [] }
            MDQuerySetMaxCount(q, 12)
            MDQueryExecute(q, CFOptionFlags(kMDQuerySynchronous.rawValue))

            let count = MDQueryGetResultCount(q)
            var items: [PickerItem] = []
            for i in 0 ..< count {
                guard let raw = MDQueryGetResultAtIndex(q, i) else { continue }
                let item = Unmanaged<MDItem>.fromOpaque(raw).takeUnretainedValue()
                guard
                    let path = MDItemCopyAttribute(item, kMDItemPath) as? String,
                    let name = MDItemCopyAttribute(item, kMDItemDisplayName) as? String
                else { continue }
                items.append(PickerItem(
                    id: path, pluginID: "builtin.file_search",
                    title: name, subtitle: path, iconName: "doc",
                    score: 0.5
                ))
            }
            return items
        }.value
    }
}

// MARK: - Notification name

extension Notification.Name {
    public static let roverFileSearchUpdate = Notification.Name("dev.undivisible.rover.fileSearchUpdate")
}
