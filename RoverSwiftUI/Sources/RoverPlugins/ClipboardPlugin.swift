import AppKit
import Foundation
import RoverCore

/// Clipboard history backed by `RoverDatabase`.
///
/// Monitors the system pasteboard with **adaptive polling** — matches the main
/// Rover app's strategy: 500 ms fast interval while the clipboard is changing,
/// backing off to 2 s after 8 consecutive idle cycles (~4 s quiet).
/// The monitor task is cancelled on deinit so there are no stray background tasks.
@MainActor
public final class ClipboardPlugin: RoverPlugin {
    public let id = "builtin.clipboard"
    public let name = "Clipboard History"

    private let db: RoverDatabase?
    private var monitorTask: Task<Void, Never>?

    // Adaptive polling tuning (matches rover main).
    private static let fastIntervalNs: UInt64  =   500_000_000  // 0.5 s
    private static let slowIntervalNs: UInt64  = 2_000_000_000  // 2.0 s
    private static let idleThreshold: Int = 8  // cycles before backing off

    public init() {
        db = try? RoverDatabase.open()
    }

    deinit {
        monitorTask?.cancel()
    }

    public func onLoad() {
        monitorTask = Task { [weak self] in await self?.runMonitor() }
    }

    public func query(_ query: String) -> [PickerItem] {
        let q = query.lowercased()
        guard q.isEmpty || "clipboard".hasPrefix(q) || "history".hasPrefix(q) else { return [] }
        guard let entries = try? db?.clipboardHistory() else { return [] }

        let count = entries.count
        return entries.prefix(20).enumerated().map { i, entry in
            let preview = String(entry.content.prefix(80))
                .replacingOccurrences(of: "\n", with: " ")
            return PickerItem(
                id: "\(entry.id)",
                pluginID: id,
                title: preview,
                iconName: "doc.on.clipboard",
                score: Double(count - i) / Double(max(count, 1))
            )
        }
    }

    public func confirm(itemID: String, query: String, secondary: Bool) -> ConfirmAction {
        guard
            let entryID = Int64(itemID),
            let entries = try? db?.clipboardHistory(),
            let entry = entries.first(where: { $0.id == entryID })
        else { return .dismiss }

        if secondary {
            try? db?.deleteClipboard(id: entryID)
            return .stay
        }
        return .copyToClipboard(entry.content)
    }

    // MARK: - Monitor

    private var lastChangeCount = NSPasteboard.general.changeCount

    private func runMonitor() async {
        var idleCount = 0

        while !Task.isCancelled {
            // Adaptive interval: fast when active, slow when idle.
            let interval = idleCount >= Self.idleThreshold
                ? Self.slowIntervalNs
                : Self.fastIntervalNs
            try? await Task.sleep(nanoseconds: interval)
            guard !Task.isCancelled else { return }

            let count = NSPasteboard.general.changeCount
            guard count != lastChangeCount else {
                idleCount += 1
                continue
            }

            // Clipboard changed — reset idle counter and record entry.
            lastChangeCount = count
            idleCount = 0

            if let text = NSPasteboard.general.string(forType: .string), !text.isEmpty {
                try? db?.pushClipboard(text: text)
            }
        }
    }
}
