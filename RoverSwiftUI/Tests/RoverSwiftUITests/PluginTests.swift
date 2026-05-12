import Foundation
import RoverCore
import RoverPlugins
import Testing

@MainActor
@Suite("Rover SwiftUI plugins")
struct PluginTests {
    @Test func webSearchRoutesToExpectedEngines() {
        let plugin = WebSearchPlugin()

        let google = plugin.query("search rover launcher")
        #expect(google.count == 1)
        #expect(google[0].id.contains("google.com/search"))
        #expect(google[0].id.contains("rover%20launcher"))

        let youtube = plugin.query("yt swiftui")
        #expect(youtube.count == 1)
        #expect(youtube[0].id.contains("youtube.com/results"))
        #expect(youtube[0].title == "Search YouTube")
    }

    @Test func googleShortcutsBuildQuerySpecificURLs() {
        let plugin = GoogleIntegrationsPlugin()

        let maps = plugin.query("?gmaps Melbourne coffee")
        #expect(maps.count == 3)

        let action = plugin.confirm(itemID: maps[0].id, query: "?gmaps Melbourne coffee", secondary: false)
        guard case .openURL(let url) = action else {
            Issue.record("expected openURL")
            return
        }
        #expect(url.absoluteString.contains("google.com/maps/search"))
        #expect(url.absoluteString.contains("Melbourne%20coffee"))
    }

    @Test func emojiSearchAndConfirmCopiesSelectedEmoji() {
        let plugin = EmojiPlugin()

        let results = plugin.query(":rocket")
        #expect(results.contains { $0.title == "🚀" })

        let action = plugin.confirm(itemID: "emoji.🚀", query: ":rocket", secondary: false)
        guard case .copyToClipboard(let value) = action else {
            Issue.record("expected copyToClipboard")
            return
        }
        #expect(value == "🚀")
    }

    @Test func editorPluginRoutesModes() {
        let plugin = EditorPlugin()

        let markdown = plugin.query("markdown")
        #expect(markdown.first?.title == "Markdown Editor")

        let action = plugin.confirm(itemID: "editor.code", query: "code", secondary: false)
        guard case .openEditor(let mode) = action else {
            Issue.record("expected openEditor")
            return
        }
        #expect(mode == .code)
    }

    @Test func tilePluginRoutesCommands() {
        let plugin = TilePlugin()

        let results = plugin.query("mosaic")
        #expect(results.contains { $0.id == "tile.grid" })

        let action = plugin.confirm(itemID: "tile.left-half", query: "tile left", secondary: false)
        guard case .runTile(let command) = action else {
            Issue.record("expected runTile")
            return
        }
        #expect(command == .frontmost("left-half"))
    }

    @Test func registrySortsPluginResultsByScore() {
        let registry = PluginRegistry()
        registry.register(CalculatorPlugin())
        registry.register(WebSearchPlugin())

        let result = registry.query("search 1+1")
        #expect(result.items.first?.pluginID == "builtin.web_search")
    }
}
