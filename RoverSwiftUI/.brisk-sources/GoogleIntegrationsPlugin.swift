import Foundation

@MainActor
public final class GoogleIntegrationsPlugin: RoverPlugin {
    public let id = "builtin.google"
    public let name = "Google"

    public init() {}

    public func query(_ query: String) -> [PickerItem] {
        let trimmed = query.trimmingCharacters(in: .whitespaces)
        guard !trimmed.isEmpty else { return [] }

        if let term = Self.term(after: "?gmaps ", in: trimmed) {
            let encoded = Self.encode(term)
            return [
                item("gmaps:search:\(encoded)", "Search Maps", term, "map", 0.96),
                item("gmaps:nearby:\(encoded)", "Nearby Places", term, "location", 0.9),
                item("gmaps:directions:\(encoded)", "Directions", term, "car", 0.84),
            ]
        }

        if Self.hasPrefix("?gcal", in: trimmed) {
            return [
                item("gcal:now", "View Calendar", "Open Google Calendar", "calendar", 0.96),
                item("gcal:create", "Create Event", "Create a Google Calendar event", "calendar.badge.plus", 0.9),
            ]
        }

        if let term = Self.term(after: "?gmail ", in: trimmed) {
            let encoded = Self.encode(term)
            return [
                item("gmail:search:\(encoded)", "Search Gmail", term, "envelope", 0.96),
                item("gmail:compose", "Compose Email", "Write a new email", "square.and.pencil", 0.9),
                item("gmail:sent", "Sent", "View sent emails", "paperplane", 0.82),
            ]
        }

        if let term = Self.term(after: "?gdrive ", in: trimmed) {
            let encoded = Self.encode(term)
            return [
                item("gdrive:search:\(encoded)", "Search Drive", term, "externaldrive", 0.96),
                item("gdrive:recent", "Recent Files", "View recent files", "clock", 0.9),
                item("gdrive:shared", "Shared with Me", "View shared files", "person.2", 0.82),
            ]
        }

        if let term = Self.term(after: "?gdocs ", in: trimmed) {
            let encoded = Self.encode(term)
            return [
                item("gdocs:search:\(encoded)", "Search Docs", term, "doc.text", 0.96),
                item("gdocs:new", "New Document", "Create a Google Doc", "doc.badge.plus", 0.86),
            ]
        }

        if Self.hasPrefix("?gsheets", in: trimmed) {
            return [
                item("gsheets:new", "New Spreadsheet", "Create a Google Sheet", "tablecells", 0.96),
            ]
        }

        if Self.hasPrefix("?gslides", in: trimmed) {
            return [
                item("gslides:new", "New Presentation", "Create a Google Slides deck", "rectangle.on.rectangle", 0.96),
            ]
        }

        return []
    }

    public func confirm(itemID: String, query: String, secondary: Bool) -> ConfirmAction {
        guard let url = Self.url(for: itemID) else { return .dismiss }
        return .openURL(url)
    }

    private func item(_ id: String, _ title: String, _ subtitle: String, _ icon: String, _ score: Double) -> PickerItem {
        PickerItem(id: id, pluginID: self.id, title: title, subtitle: subtitle, iconName: icon, score: score)
    }

    private static func hasPrefix(_ prefix: String, in query: String) -> Bool {
        query.lowercased() == prefix || query.lowercased().hasPrefix("\(prefix) ")
    }

    private static func term(after prefix: String, in query: String) -> String? {
        guard query.lowercased().hasPrefix(prefix) else { return nil }
        let term = String(query.dropFirst(prefix.count)).trimmingCharacters(in: .whitespaces)
        return term.isEmpty ? nil : term
    }

    private static func encode(_ value: String) -> String {
        value.addingPercentEncoding(withAllowedCharacters: .urlQueryAllowed) ?? value
    }

    private static func url(for itemID: String) -> URL? {
        if itemID.hasPrefix("gmaps:search:") {
            return URL(string: "https://www.google.com/maps/search/\(String(itemID.dropFirst(13)))")
        }
        if itemID.hasPrefix("gmaps:nearby:") {
            return URL(string: "https://www.google.com/maps/search/nearby+\(String(itemID.dropFirst(13)))")
        }
        if itemID.hasPrefix("gmaps:directions:") {
            return URL(string: "https://www.google.com/maps/dir/?api=1&destination=\(String(itemID.dropFirst(17)))")
        }
        if itemID.hasPrefix("gmail:search:") {
            return URL(string: "https://mail.google.com/mail/u/0/#search/\(String(itemID.dropFirst(13)))")
        }
        if itemID.hasPrefix("gdrive:search:") {
            return URL(string: "https://drive.google.com/drive/search?q=\(String(itemID.dropFirst(14)))")
        }
        if itemID.hasPrefix("gdocs:search:") {
            return URL(string: "https://docs.google.com/document/search?q=\(String(itemID.dropFirst(13)))")
        }

        let urls = [
            "gcal:now": "https://calendar.google.com/calendar/r",
            "gcal:create": "https://calendar.google.com/calendar/render?action=TEMPLATE",
            "gmail:compose": "https://mail.google.com/mail/u/0/#inbox?compose=new",
            "gmail:sent": "https://mail.google.com/mail/u/0/#sent",
            "gdrive:recent": "https://drive.google.com/drive/recent",
            "gdrive:shared": "https://drive.google.com/drive/shared-with-me",
            "gdocs:new": "https://docs.google.com/document/create",
            "gsheets:new": "https://docs.google.com/spreadsheets/create",
            "gslides:new": "https://docs.google.com/presentation/create",
        ]
        return urls[itemID].flatMap(URL.init(string:))
    }
}
