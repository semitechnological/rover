import AppKit
import RoverCore

/// Scans /Applications, /System/Applications, and ~/Applications for .app bundles.
@MainActor
public final class AppLauncherPlugin: RoverPlugin {
    public let id = "builtin.app_launcher"
    public let name = "App Launcher"

    private var apps: [(name: String, path: String, bundleID: String?)] = []

    public init() {}

    public func onLoad() {
        Task.detached(priority: .background) { [weak self] in
            let found = Self.scanApps()
            await MainActor.run { self?.apps = found }
        }
    }

    public func query(_ query: String) -> [PickerItem] {
        let q = query.lowercased()
        return apps
            .compactMap { app -> PickerItem? in
                let lower = app.name.lowercased()
                guard lower.contains(q) else { return nil }
                // Simple prefix bonus.
                let score: Double = lower.hasPrefix(q) ? 1.0 : 0.6
                return PickerItem(
                    id: app.path,
                    pluginID: id,
                    title: app.name,
                    subtitle: app.path,
                    iconName: app.bundleID,
                    score: score
                )
            }
            .sorted { $0.score > $1.score }
            .prefix(8)
            .map { $0 }
    }

    public func confirm(itemID: String, query: String, secondary: Bool) -> ConfirmAction {
        let appName = URL(fileURLWithPath: itemID)
            .deletingPathExtension()
            .lastPathComponent
        return .launchApp(appName)
    }

    // MARK: - Private

    private static func scanApps() -> [(name: String, path: String, bundleID: String?)] {
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
