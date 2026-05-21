import Foundation

@MainActor
public final class TilePlugin: RoverPlugin {
    public let id = "builtin.tile"
    public let name = "Tile"

    private struct Command {
        let itemID: String
        let title: String
        let subtitle: String
        let iconName: String
        let keywords: [String]
        let command: TileCommand
    }

    private let commands: [Command] = [
        Command(itemID: "tile.left-half", title: "Tile Left Half", subtitle: "Move the frontmost window to the left half", iconName: "rectangle.leftthird.inset.filled", keywords: ["tile", "left", "half", "window"], command: .frontmost("left-half")),
        Command(itemID: "tile.right-half", title: "Tile Right Half", subtitle: "Move the frontmost window to the right half", iconName: "rectangle.rightthird.inset.filled", keywords: ["tile", "right", "half", "window"], command: .frontmost("right-half")),
        Command(itemID: "tile.top-half", title: "Tile Top Half", subtitle: "Move the frontmost window to the top half", iconName: "rectangle.topthird.inset.filled", keywords: ["tile", "top", "half", "window"], command: .frontmost("top-half")),
        Command(itemID: "tile.bottom-half", title: "Tile Bottom Half", subtitle: "Move the frontmost window to the bottom half", iconName: "rectangle.bottomthird.inset.filled", keywords: ["tile", "bottom", "half", "window"], command: .frontmost("bottom-half")),
        Command(itemID: "tile.top-left", title: "Tile Top Left", subtitle: "Move the frontmost window to the top-left quarter", iconName: "rectangle.inset.topleft.filled", keywords: ["tile", "top", "left", "quarter", "window"], command: .frontmost("top-left")),
        Command(itemID: "tile.top-right", title: "Tile Top Right", subtitle: "Move the frontmost window to the top-right quarter", iconName: "rectangle.inset.topright.filled", keywords: ["tile", "top", "right", "quarter", "window"], command: .frontmost("top-right")),
        Command(itemID: "tile.bottom-left", title: "Tile Bottom Left", subtitle: "Move the frontmost window to the bottom-left quarter", iconName: "rectangle.inset.bottomleft.filled", keywords: ["tile", "bottom", "left", "quarter", "window"], command: .frontmost("bottom-left")),
        Command(itemID: "tile.bottom-right", title: "Tile Bottom Right", subtitle: "Move the frontmost window to the bottom-right quarter", iconName: "rectangle.inset.bottomright.filled", keywords: ["tile", "bottom", "right", "quarter", "window"], command: .frontmost("bottom-right")),
        Command(itemID: "tile.maximize", title: "Tile Maximize", subtitle: "Maximize the frontmost window within the usable screen", iconName: "arrow.up.left.and.arrow.down.right", keywords: ["tile", "maximize", "full", "window"], command: .frontmost("maximize")),
        Command(itemID: "tile.center", title: "Tile Center", subtitle: "Center the frontmost window at 60% screen size", iconName: "rectangle.center.inset.filled", keywords: ["tile", "center", "window"], command: .frontmost("center")),
        Command(itemID: "tile.grid", title: "Mosaic Visible Windows", subtitle: "Arrange visible windows into a grid", iconName: "square.grid.2x2", keywords: ["tile", "mosaic", "grid", "windows"], command: .mosaic("grid")),
        Command(itemID: "tile.columns", title: "Mosaic Columns", subtitle: "Arrange visible windows into columns", iconName: "rectangle.split.3x1", keywords: ["tile", "mosaic", "columns", "windows"], command: .mosaic("columns")),
        Command(itemID: "tile.rows", title: "Mosaic Rows", subtitle: "Arrange visible windows into rows", iconName: "rectangle.split.1x2", keywords: ["tile", "mosaic", "rows", "windows"], command: .mosaic("rows")),
        Command(itemID: "tile.accessibility", title: "Enable Tile Accessibility", subtitle: "Request permission to move and resize windows", iconName: "accessibility", keywords: ["tile", "accessibility", "permission"], command: .requestAccessibility),
    ]

    public init() {}

    public func query(_ query: String) -> [PickerItem] {
        let q = query.lowercased().trimmingCharacters(in: .whitespaces)
        guard !q.isEmpty else { return [] }

        return commands.compactMap { command in
            let matched = command.keywords.contains { $0.hasPrefix(q) }
                || command.title.lowercased().contains(q)
                || command.subtitle.lowercased().contains(q)
            guard matched else { return nil }
            return PickerItem(
                id: command.itemID,
                pluginID: id,
                title: command.title,
                subtitle: command.subtitle,
                iconName: command.iconName,
                score: command.title.lowercased().hasPrefix(q) ? 0.93 : 0.8
            )
        }
    }

    public func confirm(itemID: String, query: String, secondary: Bool) -> ConfirmAction {
        guard let command = commands.first(where: { $0.itemID == itemID }) else {
            return .dismiss
        }
        return .runTile(command.command)
    }
}
