import Foundation

/// Holds all registered plugins and dispatches queries.
@MainActor
public final class PluginRegistry: ObservableObject {
    private var plugins: [any RoverPlugin] = []

    public init() {}

    public func register(_ plugin: any RoverPlugin) {
        plugins.append(plugin)
        plugin.onLoad()
    }

    // MARK: - Query

    /// Run all plugins against `query` and return sorted results.
    ///
    /// Plugins receive the query even when it is empty so always-active plugins
    /// (e.g. `BuiltinCommandsPlugin`) can surface items on an empty launcher.
    /// Each plugin is responsible for returning `[]` when it has nothing to show.
    public func query(_ query: String) -> (items: [PickerItem], cards: [InlineCard]) {
        return self.query(query, pluginIDs: nil)
    }

    /// Run a subset of plugins (by ID) against `query`. Used for partial refreshes
    /// triggered by background completions (AI stream, Spotlight, semantic search).
    public func query(_ query: String, pluginIDs: [String]?) -> (items: [PickerItem], cards: [InlineCard]) {
        var items: [PickerItem] = []
        var cards: [InlineCard] = []

        let targets = pluginIDs.map { ids in plugins.filter { ids.contains($0.id) } } ?? plugins

        for plugin in targets {
            items.append(contentsOf: plugin.query(query))
            if let card = plugin.inlineCard(for: query) {
                cards.append(card)
            }
        }

        items.sort { $0.score > $1.score }
        return (items, cards)
    }

    // MARK: - Confirm

    public func confirm(pluginID: String, itemID: String, query: String, secondary: Bool = false) -> ConfirmAction {
        guard let plugin = plugins.first(where: { $0.id == pluginID }) else {
            return .dismiss
        }
        return plugin.confirm(itemID: itemID, query: query, secondary: secondary)
    }
}
