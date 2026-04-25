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

    /// Run all plugins against `query` and return sorted, deduplicated results.
    public func query(_ query: String) -> (items: [PickerItem], cards: [InlineCard]) {
        guard !query.trimmingCharacters(in: .whitespaces).isEmpty else {
            return ([], [])
        }

        var items: [PickerItem] = []
        var cards: [InlineCard] = []

        for plugin in plugins {
            let results = plugin.query(query)
            items.append(contentsOf: results)

            if let card = plugin.inlineCard(for: query) {
                cards.append(card)
            }
        }

        // Sort by score descending, stable.
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
