import AppKit
import Foundation
import RoverCore

/// Shows clipboard history backed by the Rust RoverDatabase.
@MainActor
public final class ClipboardPlugin: RoverPlugin {
    public let id = "builtin.clipboard"
    public let name = "Clipboard History"

    private let db: RoverDatabase?

    public init() {
        db = try? RoverDatabase.open()
    }

    public func onLoad() {
        // Start monitoring the clipboard for new entries.
        Task { await self.startMonitoring() }
    }

    public func query(_ query: String) -> [PickerItem] {
        // Clipboard history is typically entered via the "Clipboard History" command;
        // we also surface it for prefix searches.
        guard query.hasPrefix("clip") || query.isEmpty else { return [] }
        guard let entries = try? db?.clipboardHistory() else { return [] }

        return entries.prefix(20).enumerated().map { i, entry in
            let preview = entry.content.prefix(60).replacingOccurrences(of: "\n", with: " ")
            return PickerItem(
                id: "\(entry.id)",
                pluginID: id,
                title: String(preview),
                iconName: "doc.on.clipboard",
                score: Double(entries.count - i) / Double(entries.count)
            )
        }
    }

    public func confirm(itemID: String, query: String, secondary: Bool) -> ConfirmAction {
        guard
            let id = Int64(itemID),
            let entries = try? db?.clipboardHistory(),
            let entry = entries.first(where: { $0.id == id })
        else { return .dismiss }

        if secondary {
            // Secondary action: delete from history.
            try? db?.deleteClipboard(id: id)
            return .stay
        }
        return .copyToClipboard(entry.content)
    }

    // MARK: - Monitoring

    private var lastCount = NSPasteboard.general.changeCount

    private func startMonitoring() async {
        while true {
            try? await Task.sleep(nanoseconds: 500_000_000) // 0.5 s
            let count = NSPasteboard.general.changeCount
            guard count != lastCount else { continue }
            lastCount = count
            if let text = NSPasteboard.general.string(forType: .string) {
                try? db?.pushClipboard(text: text)
            }
        }
    }
}
