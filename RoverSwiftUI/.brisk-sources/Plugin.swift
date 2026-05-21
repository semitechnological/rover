import Foundation

// MARK: - Plugin item

/// One result row shown in the launcher list.
public struct PickerItem: Identifiable, Sendable {
    public let id: String
    public let pluginID: String
    public let title: String
    public let subtitle: String?
    /// SF Symbol name, app bundle id, file path, or emoji.
    public let iconName: String?
    /// Higher = better match.
    public let score: Double

    public init(
        id: String,
        pluginID: String,
        title: String,
        subtitle: String? = nil,
        iconName: String? = nil,
        score: Double = 0
    ) {
        self.id = id
        self.pluginID = pluginID
        self.title = title
        self.subtitle = subtitle
        self.iconName = iconName
        self.score = score
    }
}

// MARK: - Confirm action

/// What the launcher should do after the user confirms an item.
public enum ConfirmAction: Sendable {
    case dismiss
    /// Open a URL in the user's default browser (NSWorkspace).
    case openURL(URL)
    /// Open a URL in Rover's built-in WebKit browser window.
    case openBrowser(URL)
    case launchApp(String)
    case runShell(String)
    case copyToClipboard(String)
    case openTerminal(initialCommand: String?)
    case openEditor(EditorMode)
    case runTile(TileCommand)
    case openSettings
    case enterSubmenu(title: String, pluginID: String)
    case stay
}

public enum EditorMode: String, Sendable {
    case code
    case text
    case markdown
}

public enum TileCommand: Sendable, Equatable {
    case frontmost(String)
    case mosaic(String)
    case requestAccessibility
}

// MARK: - Inline card

/// Content shown as a card above the results list (calculator, AI, etc.).
public struct InlineCard: Sendable {
    public enum Kind: Sendable { case calculator, ai, terminal, info }
    public let kind: Kind
    public let content: String

    public init(kind: Kind, content: String) {
        self.kind = kind
        self.content = content
    }
}

// MARK: - Plugin protocol

/// Every native Swift plugin conforms to this protocol.
///
/// Methods are called on the main actor via `PluginRegistry`.
@MainActor
public protocol RoverPlugin: AnyObject {
    /// Stable unique identifier, e.g. `"builtin.app_launcher"`.
    var id: String { get }
    /// Human-readable name.
    var name: String { get }

    /// Called once at startup.
    func onLoad()

    /// Return match results for the current query. Fast — runs on main actor.
    func query(_ query: String) -> [PickerItem]

    /// Confirm a result. Returns the action to execute.
    func confirm(itemID: String, query: String, secondary: Bool) -> ConfirmAction

    /// Optional inline card for the current query (e.g. calculator result).
    func inlineCard(for query: String) -> InlineCard?
}

public extension RoverPlugin {
    func onLoad() {}
    func inlineCard(for _: String) -> InlineCard? { nil }
}
