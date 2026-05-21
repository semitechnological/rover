import Foundation

private struct MacOSNote {
    let title: String
    let body: String
    let modified: String
}

private struct MacOSReminder {
    let title: String
    let dueDate: String
    let list: String
    let completed: Bool
}

private struct MacOSEvent {
    let title: String
    let startDate: String
    let endDate: String
    let calendar: String
}

@MainActor
public final class MacOSToolsPlugin: RoverPlugin {
    public let id = "builtin.macos_tools"
    public let name = "macOS Tools"

    private var loaded = false
    private var notes: [MacOSNote] = []
    private var reminders: [MacOSReminder] = []
    private var events: [MacOSEvent] = []

    public init() {}

    public func onLoad() {
        load()
    }

    public func query(_ query: String) -> [PickerItem] {
        let trimmed = query.trimmingCharacters(in: .whitespaces)
        guard trimmed.hasPrefix("/") else { return [] }
        if !loaded { load() }

        if trimmed == "/" || trimmed == "/notes" || trimmed == "/reminders" || trimmed == "/calendar" || trimmed == "/events" {
            return overview(for: trimmed)
        }

        if let term = Self.term(after: "/notes", in: trimmed) {
            return noteItems(matching: term)
        }
        if let term = Self.term(after: "/reminders", in: trimmed) {
            return reminderItems(matching: term)
        }
        if let term = Self.term(after: "/calendar", in: trimmed) ?? Self.term(after: "/events", in: trimmed) {
            return eventItems(matching: term)
        }

        let term = String(trimmed.dropFirst()).trimmingCharacters(in: .whitespaces)
        guard term.count >= 2 else { return [] }
        return noteItems(matching: term) + reminderItems(matching: term) + eventItems(matching: term)
    }

    public func confirm(itemID: String, query: String, secondary: Bool) -> ConfirmAction {
        if itemID.hasPrefix("note:") {
            _ = Self.runAppleScript("tell application \"Notes\" to activate")
            return .dismiss
        }
        if itemID.hasPrefix("reminder:") {
            _ = Self.runAppleScript("tell application \"Reminders\" to activate")
            return .dismiss
        }
        if itemID.hasPrefix("event:") {
            _ = Self.runAppleScript("tell application \"Calendar\" to activate")
            return .dismiss
        }
        return .dismiss
    }

    private func load() {
        notes = Self.loadNotes()
        reminders = Self.loadReminders()
        events = Self.loadEvents(days: 7)
        loaded = true
    }

    private func overview(for query: String) -> [PickerItem] {
        var items: [PickerItem] = []
        if query == "/" || query == "/calendar" || query == "/events" {
            items += events.prefix(5).map(eventItem)
        }
        if query == "/" || query == "/reminders" {
            items += reminders.filter { !$0.completed }.prefix(5).map(reminderItem)
        }
        if query == "/" || query == "/notes" {
            items += notes.prefix(5).map(noteItem)
        }
        return items
    }

    private func noteItems(matching term: String) -> [PickerItem] {
        let needle = term.lowercased()
        return notes
            .filter { $0.title.lowercased().contains(needle) || $0.body.lowercased().contains(needle) }
            .prefix(10)
            .map(noteItem)
    }

    private func reminderItems(matching term: String) -> [PickerItem] {
        let needle = term.lowercased()
        return reminders
            .filter { !$0.completed && $0.title.lowercased().contains(needle) }
            .prefix(10)
            .map(reminderItem)
    }

    private func eventItems(matching term: String) -> [PickerItem] {
        let needle = term.lowercased()
        return events
            .filter { $0.title.lowercased().contains(needle) }
            .prefix(10)
            .map(eventItem)
    }

    private func noteItem(_ note: MacOSNote) -> PickerItem {
        PickerItem(id: "note:\(note.title)", pluginID: id, title: note.title, subtitle: "Note \(note.modified)", iconName: "note.text", score: 0.86)
    }

    private func reminderItem(_ reminder: MacOSReminder) -> PickerItem {
        let due = reminder.dueDate.isEmpty ? "No due date" : reminder.dueDate
        return PickerItem(id: "reminder:\(reminder.title)", pluginID: id, title: reminder.title, subtitle: "\(reminder.list) \(due)", iconName: "checklist", score: 0.86)
    }

    private func eventItem(_ event: MacOSEvent) -> PickerItem {
        PickerItem(id: "event:\(event.title)", pluginID: id, title: event.title, subtitle: "\(event.calendar) \(event.startDate)-\(event.endDate)", iconName: "calendar", score: 0.86)
    }

    private static func term(after prefix: String, in query: String) -> String? {
        let lower = query.lowercased()
        guard lower == prefix || lower.hasPrefix("\(prefix) ") else { return nil }
        let term = String(query.dropFirst(prefix.count)).trimmingCharacters(in: .whitespaces)
        return term.isEmpty ? nil : term
    }

    private static func loadNotes() -> [MacOSNote] {
        let script = """
        tell application "Notes"
            set noteList to {}
            repeat with n in notes
                set end of noteList to (name of n) & "|||" & (plaintext of n) & "|||" & ((modified date of n) as string)
            end repeat
            set AppleScript's text item delimiters to linefeed
            return noteList as text
        end tell
        """
        return parseRows(runAppleScript(script)).prefix(50).map {
            MacOSNote(title: part($0, 0), body: part($0, 1), modified: part($0, 2))
        }
    }

    private static func loadReminders() -> [MacOSReminder] {
        let script = """
        tell application "Reminders"
            set remList to {}
            repeat with r in reminders
                set remDue to due date of r
                if remDue is missing value then
                    set dueStr to ""
                else
                    set dueStr to (remDue as string)
                end if
                set end of remList to (name of r) & "|||" & dueStr & "|||" & (name of container of r) & "|||" & ((completed of r) as string)
            end repeat
            set AppleScript's text item delimiters to linefeed
            return remList as text
        end tell
        """
        return parseRows(runAppleScript(script)).prefix(50).map {
            MacOSReminder(title: part($0, 0), dueDate: part($0, 1), list: part($0, 2), completed: part($0, 3) == "true")
        }
    }

    private static func loadEvents(days: Int) -> [MacOSEvent] {
        let script = """
        tell application "Calendar"
            set eventList to {}
            set startDate to (current date)
            set endDate to startDate + \(days) * days
            repeat with cal in calendars
                repeat with e in events of cal
                    set eStart to start date of e
                    if eStart >= startDate and eStart <= endDate then
                        set end of eventList to (summary of e) & "|||" & (eStart as string) & "|||" & ((end date of e) as string) & "|||" & (name of cal)
                    end if
                end repeat
            end repeat
            set AppleScript's text item delimiters to linefeed
            return eventList as text
        end tell
        """
        return parseRows(runAppleScript(script)).prefix(50).map {
            MacOSEvent(title: part($0, 0), startDate: part($0, 1), endDate: part($0, 2), calendar: part($0, 3))
        }
    }

    private static func runAppleScript(_ script: String) -> String {
        let process = Process()
        let output = Pipe()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/osascript")
        process.arguments = ["-e", script]
        process.standardOutput = output
        process.standardError = Pipe()
        do {
            try process.run()
            process.waitUntilExit()
            guard process.terminationStatus == 0 else { return "" }
            return String(data: output.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8) ?? ""
        } catch {
            return ""
        }
    }

    private static func parseRows(_ output: String) -> [[String]] {
        output
            .split(whereSeparator: \.isNewline)
            .map { $0.split(separator: "|||", omittingEmptySubsequences: false).map(String.init) }
    }

    private static func part(_ parts: [String], _ index: Int) -> String {
        parts.indices.contains(index) ? parts[index] : ""
    }
}
