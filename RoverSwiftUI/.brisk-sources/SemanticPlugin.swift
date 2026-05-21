import Foundation

/// Semantic (meaning-based) search over apps, clipboard, and files.
///
/// Uses the Rust `SemanticSearch` backend (fastembed AllMiniLML6V2, ~33 MB).
/// The embedding model loads on a background thread at startup — queries return
/// empty until ready, then results appear automatically.
///
/// Activated by any query ≥ 3 characters that begins with `~` (explicit),
/// or passively surfaces results below fuzzy hits for queries ≥ 6 characters.
@MainActor
public final class SemanticPlugin: RoverPlugin {
    public let id  = "builtin.semantic"
    public let name = "Semantic Search"

    // Loaded asynchronously; nil until ready.
    private var index: SemanticSearch?
    private var isReady = false
    private var indexTask: Task<Void, Never>?

    // Cached results for the last query — updated off-thread.
    private var cachedQuery   = ""
    private var cachedResults: [PickerItem] = []
    private var searchTask: Task<Void, Never>?
    private static let debounceNs: UInt64 = 300_000_000   // 300 ms

    public init() {}

    public func onLoad() {
        indexTask = Task { [weak self] in
            await self?.loadIndex()
        }
    }

    deinit {
        indexTask?.cancel()
    }

    // MARK: - RoverPlugin

    public func query(_ query: String) -> [PickerItem] {
        guard isReady, let index else { return [] }
        let q = query.trimmingCharacters(in: .whitespaces)

        // Explicit prefix `~` forces semantic; otherwise require ≥ 6 chars.
        let isExplicit = q.hasPrefix("~")
        let term = isExplicit ? String(q.dropFirst()).trimmingCharacters(in: .whitespaces) : q
        guard term.count >= (isExplicit ? 2 : 6) else { return [] }

        // Return cached results immediately; kick off a background re-rank.
        if term != cachedQuery {
            cachedQuery = term
            kickSearch(term: term, index: index)
        }

        return cachedResults
    }

    public func confirm(itemID: String, query: String, secondary: Bool) -> ConfirmAction {
        // IDs are "app:<path>", "clip:<entryID>".
        if itemID.hasPrefix("app:") {
            let path = String(itemID.dropFirst(4))
            let name = URL(fileURLWithPath: path).deletingPathExtension().lastPathComponent
            return .launchApp(name)
        }
        if itemID.hasPrefix("clip:"),
           let entryID = Int64(itemID.dropFirst(5))
        {
            guard
                let db = try? RoverDatabase.open(),
                let entry = try? db.clipboardHistory().first(where: { $0.id == entryID })
            else { return .dismiss }
            return secondary ? .stay : .copyToClipboard(entry.content)
        }
        return .dismiss
    }

    // MARK: - Private

    private func loadIndex() async {
        // Open the index on a background thread (blocks while model loads).
        let result = await Task.detached(priority: .background) {
            Result { try SemanticSearch.open() }
        }.value

        switch result {
        case .success(let idx):
            index = idx
            isReady = true
            // Seed the index with installed apps and recent clipboard.
            await seedIndex(idx)
        case .failure(let err):
            // Non-fatal — semantic search just won't work.
            print("[SemanticPlugin] failed to load index: \(err)")
        }
    }

    private func seedIndex(_ idx: SemanticSearch) async {
        // Apps — scan on background, then batch-upsert.
        let apps = await Task.detached(priority: .background) {
            SemanticPlugin.scanApps()
        }.value

        let appDocs = apps.map { app in
            SemanticDoc(id: "app:\(app.path)", collection: "apps", text: app.name)
        }
        if !appDocs.isEmpty {
            try? idx.upsertBatch(docs: appDocs)
        }

        // Clipboard — most recent 100 entries.
        if let db = try? RoverDatabase.open(),
           let entries = try? db.clipboardHistory()
        {
            let clipDocs = entries.prefix(100).map { e in
                SemanticDoc(id: "clip:\(e.id)", collection: "clipboard", text: e.content)
            }
            if !clipDocs.isEmpty {
                try? idx.upsertBatch(docs: Array(clipDocs))
            }
        }
    }

    private func kickSearch(term: String, index: SemanticSearch) {
        // Cancel any in-flight search before starting a new one.
        searchTask?.cancel()
        searchTask = Task { [weak self] in
            // 300ms debounce — coalesce rapid keystrokes.
            try? await Task.sleep(nanoseconds: Self.debounceNs)
            guard !Task.isCancelled else { return }

            let results = await Task.detached(priority: .userInitiated) {
                (try? index.search(query: term, collection: "", topK: 6)) ?? []
            }.value

            guard let self, !Task.isCancelled else { return }
            self.cachedResults = results.map { r in
                PickerItem(
                    id: r.id,
                    pluginID: self.id,
                    title: SemanticPlugin.title(for: r),
                    subtitle: SemanticPlugin.subtitle(for: r),
                    iconName: SemanticPlugin.icon(for: r),
                    // Cosine similarity in [-1,1] → score in [0, 0.7] so semantic
                    // results sit below strong fuzzy matches but above weak ones.
                    score: Double((r.score + 1) / 2) * 0.7
                )
            }
            // Notify the ViewModel to re-query — same pattern as AIPlugin.
            NotificationCenter.default.post(name: .roverSemanticUpdate, object: nil)
        }
    }

    // MARK: - Display helpers

    private static func title(for r: SemanticResult) -> String {
        switch r.collection {
        case "apps":
            return URL(fileURLWithPath: r.text).deletingPathExtension().lastPathComponent
        case "clipboard":
            return String(r.text.prefix(80)).replacingOccurrences(of: "\n", with: " ")
        default:
            return r.text
        }
    }

    private static func subtitle(for r: SemanticResult) -> String? {
        switch r.collection {
        case "apps":      return "App (semantic match)"
        case "clipboard": return "Clipboard (semantic match)"
        default:          return nil
        }
    }

    private static func icon(for r: SemanticResult) -> String? {
        switch r.collection {
        case "apps":      return "app.dashed"
        case "clipboard": return "doc.on.clipboard"
        default:          return "sparkle.magnifyingglass"
        }
    }

    private nonisolated static func scanApps() -> [(name: String, path: String)] {
        let dirs: [URL] = [
            URL(fileURLWithPath: "/Applications"),
            URL(fileURLWithPath: "/System/Applications"),
            FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Applications"),
        ]
        var result: [(name: String, path: String)] = []
        for dir in dirs {
            guard let enumerator = FileManager.default.enumerator(
                at: dir,
                includingPropertiesForKeys: [.isDirectoryKey],
                options: [.skipsPackageDescendants]
            ) else { continue }
            for case let url as URL in enumerator where url.pathExtension == "app" {
                result.append((
                    name: url.deletingPathExtension().lastPathComponent,
                    path: url.path
                ))
            }
        }
        return result
    }
}

// MARK: - Notification name

extension Notification.Name {
    public static let roverSemanticUpdate = Notification.Name("dev.undivisible.rover.semanticUpdate")
}
