import AppKit
import RoverCore

/// Spotlight-style app launcher.
///
/// Scans /Applications, /System/Applications, and ~/Applications once at startup
/// on a background thread. Queries are scored via the Rust `FuzzyEngine`
/// (rayon-parallel skim algorithm) — the same scorer as the GPUI branch.
@MainActor
public final class AppLauncherPlugin: RoverPlugin {
    public let id = "builtin.app_launcher"
    public let name = "App Launcher"

    // App list populated in onLoad; frozen afterwards (read-only from query).
    private var apps: [(name: String, path: String, bundleID: String?)] = []
    // Parallel fuzzy scorer backed by rayon on the Rust side.
    private let fuzzy = FuzzyEngine()

    public init() {}

    public func onLoad() {
        // Initial scan then periodic refresh every 30 s so newly installed apps appear.
        Task { [weak self] in
            await self?.rescan()
            while !Task.isCancelled {
                try? await Task.sleep(nanoseconds: 30_000_000_000)
                guard !Task.isCancelled else { return }
                await self?.rescan()
            }
        }
    }

    private func rescan() async {
        let found = await Task.detached(priority: .background) {
            AppLauncherPlugin.scanApps()
        }.value
        self.apps = found
    }

    public func query(_ query: String) -> [PickerItem] {
        let q = query.trimmingCharacters(in: .whitespaces)
        guard !q.isEmpty else { return [] }

        let names = apps.map(\.name)
        let ranked = fuzzy.rank(query: q, candidates: names)

        return ranked.prefix(8).compactMap { ranked -> PickerItem? in
            // Find the app entry that matches the ranked candidate name.
            guard let app = apps.first(where: { $0.name == ranked.candidate }) else { return nil }
            // Normalise skim score (i64, higher = better) to [0, 1] Double.
            // Skim scores are unbounded; clamp to a sane max of 10 000.
            let normalisedScore = min(Double(ranked.score) / 10_000.0, 1.0)
            return PickerItem(
                id: app.path,
                pluginID: id,
                title: app.name,
                subtitle: app.path,
                iconName: app.bundleID,
                score: normalisedScore
            )
        }
    }

    public func confirm(itemID: String, query: String, secondary: Bool) -> ConfirmAction {
        let appName = URL(fileURLWithPath: itemID)
            .deletingPathExtension()
            .lastPathComponent
        return .launchApp(appName)
    }

    // MARK: - Private

    /// `nonisolated` — only uses thread-safe Foundation APIs.
    private nonisolated static func scanApps() -> [(name: String, path: String, bundleID: String?)] {
        let dirs: [URL] = [
            URL(fileURLWithPath: "/Applications"),
            URL(fileURLWithPath: "/System/Applications"),
            FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Applications"),
        ]

        var result: [(name: String, path: String, bundleID: String?)] = []

        for dir in dirs {
            guard let enumerator = FileManager.default.enumerator(
                at: dir,
                includingPropertiesForKeys: [.isDirectoryKey],
                options: [.skipsPackageDescendants]
            ) else { continue }

            for case let url as URL in enumerator {
                guard url.pathExtension == "app" else { continue }
                let name = url.deletingPathExtension().lastPathComponent
                let bundleID = Bundle(url: url)?.bundleIdentifier
                result.append((name: name, path: url.path, bundleID: bundleID))
            }
        }

        return result.sorted { $0.name.localizedCompare($1.name) == .orderedAscending }
    }
}
