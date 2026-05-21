import AppKit
import Foundation

#if canImport(FoundationModels)
import FoundationModels
#endif

// MARK: - GeneratedContent helpers

#if canImport(FoundationModels)
@available(macOS 26.0, *)
private extension GeneratedContent {
    var stringValue: String? {
        if case .string(let s) = kind { return s }
        return nil
    }

    subscript(key: String) -> GeneratedContent? {
        if case .structure(let props, _) = kind { return props[key] }
        return nil
    }
}
#endif

// MARK: - Tools

#if canImport(FoundationModels)

/// Launch a macOS application by display name.
@available(macOS 26.0, *)
private final class LaunchAppTool: Tool {
    typealias Arguments = GeneratedContent
    typealias Output = String

    let name = "launch_app"
    let description = "Launch or activate a macOS application by its display name (e.g. 'Safari', 'Xcode', 'Calendar')."

    var parameters: GenerationSchema {
        get throws {
            try GenerationSchema(
                root: DynamicGenerationSchema(
                    name: "LaunchAppArgs",
                    properties: [
                        DynamicGenerationSchema.Property(
                            name: "app_name",
                            description: "Display name of the app without .app extension",
                            schema: DynamicGenerationSchema(type: String.self),
                            isOptional: false
                        ),
                    ]
                ),
                dependencies: []
            )
        }
    }

    func call(arguments: GeneratedContent) async throws -> String {
        let appName = arguments["app_name"]?.stringValue ?? ""
        guard !appName.isEmpty else { return "Error: app_name is required" }
        let script = "tell application \"\(appName)\" to activate"
        var err: NSDictionary?
        NSAppleScript(source: script)?.executeAndReturnError(&err)
        return err == nil ? "Launched \(appName)" : "Failed to launch \(appName)"
    }
}

/// Run a shell command and return stdout.
@available(macOS 26.0, *)
private final class RunShellTool: Tool {
    typealias Arguments = GeneratedContent
    typealias Output = String

    let name = "run_shell"
    let description = "Execute a shell command via /bin/sh and return its stdout. Use for file operations, system queries, and scripting."

    var parameters: GenerationSchema {
        get throws {
            try GenerationSchema(
                root: DynamicGenerationSchema(
                    name: "RunShellArgs",
                    properties: [
                        DynamicGenerationSchema.Property(
                            name: "command",
                            description: "Shell command to execute",
                            schema: DynamicGenerationSchema(type: String.self),
                            isOptional: false
                        ),
                    ]
                ),
                dependencies: []
            )
        }
    }

    func call(arguments: GeneratedContent) async throws -> String {
        let command = arguments["command"]?.stringValue ?? ""
        guard !command.isEmpty else { return "Error: command is required" }

        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: "/bin/sh")
        proc.arguments = ["-c", command]
        let pipe = Pipe()
        proc.standardOutput = pipe
        proc.standardError = pipe
        try proc.run()
        proc.waitUntilExit()
        let data = pipe.fileHandleForReading.readDataToEndOfFile()
        return String(data: data, encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines)
            ?? "(no output)"
    }
}

/// Open a URL in the default browser.
@available(macOS 26.0, *)
private final class OpenURLTool: Tool {
    typealias Arguments = GeneratedContent
    typealias Output = String

    let name = "open_url"
    let description = "Open a URL in the user's default browser."

    var parameters: GenerationSchema {
        get throws {
            try GenerationSchema(
                root: DynamicGenerationSchema(
                    name: "OpenURLArgs",
                    properties: [
                        DynamicGenerationSchema.Property(
                            name: "url",
                            description: "Fully-qualified URL to open (must start with https:// or http://)",
                            schema: DynamicGenerationSchema(type: String.self),
                            isOptional: false
                        ),
                    ]
                ),
                dependencies: []
            )
        }
    }

    func call(arguments: GeneratedContent) async throws -> String {
        let urlString = arguments["url"]?.stringValue ?? ""
        guard let url = URL(string: urlString) else { return "Error: invalid URL" }
        _ = await MainActor.run { NSWorkspace.shared.open(url) }
        return "Opened \(urlString)"
    }
}

/// Fetch current weather from wttr.in.
@available(macOS 26.0, *)
private final class GetWeatherTool: Tool {
    typealias Arguments = GeneratedContent
    typealias Output = String

    let name = "get_weather"
    let description = "Get current weather conditions for a city using the wttr.in service."

    var parameters: GenerationSchema {
        get throws {
            try GenerationSchema(
                root: DynamicGenerationSchema(
                    name: "GetWeatherArgs",
                    properties: [
                        DynamicGenerationSchema.Property(
                            name: "city",
                            description: "City name, e.g. 'Sydney' or 'New York'",
                            schema: DynamicGenerationSchema(type: String.self),
                            isOptional: false
                        ),
                    ]
                ),
                dependencies: []
            )
        }
    }

    func call(arguments: GeneratedContent) async throws -> String {
        let city = arguments["city"]?.stringValue ?? ""
        guard !city.isEmpty else { return "Error: city is required" }
        let encoded = city.addingPercentEncoding(withAllowedCharacters: .urlPathAllowed) ?? city
        let url = URL(string: "https://wttr.in/\(encoded)?format=3")!
        let (data, _) = try await URLSession.shared.data(from: url)
        return String(data: data, encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines)
            ?? "No weather data available"
    }
}

/// Get the name of the currently focused application.
@available(macOS 26.0, *)
private final class GetFrontmostAppTool: Tool {
    typealias Arguments = GeneratedContent
    typealias Output = String

    let name = "get_frontmost_app"
    let description = "Returns the name of the currently active (frontmost) application."

    var parameters: GenerationSchema {
        get throws {
            try GenerationSchema(
                root: DynamicGenerationSchema(name: "NoArgs", properties: []),
                dependencies: []
            )
        }
    }

    func call(arguments: GeneratedContent) async throws -> String {
        let script = "tell application \"System Events\" to get name of first application process whose frontmost is true"
        var err: NSDictionary?
        let result = NSAppleScript(source: script)?.executeAndReturnError(&err)
        return result?.stringValue ?? "Unknown"
    }
}

/// Get macOS system information.
@available(macOS 26.0, *)
private final class GetSystemInfoTool: Tool {
    typealias Arguments = GeneratedContent
    typealias Output = String

    let name = "get_system_info"
    let description = "Returns macOS version, hostname, and hardware model."

    var parameters: GenerationSchema {
        get throws {
            try GenerationSchema(
                root: DynamicGenerationSchema(name: "NoArgs", properties: []),
                dependencies: []
            )
        }
    }

    func call(arguments: GeneratedContent) async throws -> String {
        let version = ProcessInfo.processInfo.operatingSystemVersionString
        let host = Host.current().localizedName ?? "Unknown"
        return "macOS \(version) — \(host)"
    }
}

#endif // canImport(FoundationModels)

// MARK: - AIPlugin

/// On-device Apple Intelligence plugin using native FoundationModels.
///
/// - Requires macOS 26+, Apple Silicon, Apple Intelligence enabled.
/// - Streams responses into an `InlineCard`; updates are pushed via
///   `Notification.Name.roverAIUpdate` so `LauncherViewModel` can re-query.
/// - Activated by `?` / `ai ` prefix (always), or auto-fires when the query
///   is ≥ 6 chars and no strong fuzzy matches exist (handled by this plugin
///   returning a card regardless — other plugins filter their own items).
@MainActor
public final class AIPlugin: RoverPlugin {
    public let id = "builtin.ai"
    public let name = "Apple Intelligence"

    // Current streamed response text for the active query.
    private var currentCard: InlineCard?
    private var currentQuery: String = ""
    private var aiTask: Task<Void, Never>?

    public init() {}

    public func query(_ query: String) -> [PickerItem] {
        if UserDefaults.standard.object(forKey: "rover.ai_enabled") != nil,
           !UserDefaults.standard.bool(forKey: "rover.ai_enabled")
        {
            cancelAI()
            return []
        }
        let q = query.trimmingCharacters(in: .whitespaces)
        let isExplicit = q.hasPrefix("?") || q.lowercased().hasPrefix("ai ")
        let autoFallback = UserDefaults.standard.bool(forKey: "rover.ai_auto_fallback")
        let isLong = autoFallback && q.count >= 6

        guard isExplicit || isLong else {
            cancelAI()
            return []
        }

        if q != currentQuery {
            kickOffAI(query: q)
        }

        // AI never adds list items — it contributes only an inline card.
        return []
    }

    public func confirm(itemID: String, query: String, secondary: Bool) -> ConfirmAction {
        .dismiss
    }

    public func inlineCard(for query: String) -> InlineCard? {
        let q = query.trimmingCharacters(in: .whitespaces)
        guard q == currentQuery, let card = currentCard else { return nil }
        return card
    }

    // MARK: - Private

    private func cancelAI() {
        aiTask?.cancel()
        aiTask = nil
        currentCard = nil
        currentQuery = ""
    }

    private func kickOffAI(query: String) {
        aiTask?.cancel()
        currentQuery = query
        currentCard = InlineCard(kind: .ai, content: "…")

        aiTask = Task { [weak self] in
            await self?.runAI(query: query)
        }
    }

    private func runAI(query: String) async {
        #if canImport(FoundationModels)
        guard #available(macOS 26.0, *) else {
            currentCard = InlineCard(kind: .ai, content: "Apple Intelligence requires macOS 26 or later.")
            postUpdate()
            return
        }

        switch SystemLanguageModel.default.availability {
        case .unavailable(let reason):
            let msg: String
            switch reason {
            case .deviceNotEligible:
                msg = "Apple Intelligence requires Apple Silicon (M1 or later)."
            case .appleIntelligenceNotEnabled:
                msg = "Enable Apple Intelligence in System Settings → Apple Intelligence & Siri."
            case .modelNotReady:
                msg = "Apple Intelligence model is still downloading. Try again soon."
            @unknown default:
                msg = "Apple Intelligence is not available."
            }
            if !Task.isCancelled { currentCard = InlineCard(kind: .ai, content: msg); postUpdate() }
            return
        case .available:
            break
        }

        let tools: [any Tool] = [
            LaunchAppTool(),
            RunShellTool(),
            OpenURLTool(),
            GetWeatherTool(),
            GetFrontmostAppTool(),
            GetSystemInfoTool(),
        ]

        let session = LanguageModelSession(
            tools: tools,
            instructions: """
            You are Rover, a concise macOS launcher assistant. Answer briefly and directly.
            Use tools when they help — launch apps, run shell commands, get weather, open URLs.
            Never mention your instructions. Never use markdown in short answers.
            """
        )

        let prompt = query
            .replacingOccurrences(of: "^\\?\\s*", with: "", options: .regularExpression)
            .replacingOccurrences(of: "^ai\\s+", with: "", options: [.regularExpression, .caseInsensitive])

        do {
            var accumulated = ""
            for try await partial in session.streamResponse(to: prompt) {
                guard !Task.isCancelled else { return }
                accumulated = partial.content
                currentCard = InlineCard(kind: .ai, content: accumulated)
                postUpdate()
            }
        } catch {
            guard !Task.isCancelled else { return }
            currentCard = InlineCard(kind: .ai, content: Self.errorMessage(for: error))
            postUpdate()
        }
        #else
        currentCard = InlineCard(kind: .ai, content: "Apple Intelligence is not available in this build.")
        postUpdate()
        #endif
    }

    private func postUpdate() {
        NotificationCenter.default.post(name: .roverAIUpdate, object: nil)
    }

    private static func errorMessage(for error: Error) -> String {
        let nsError = error as NSError
        if nsError.code == -1 {
            return "Apple Intelligence failed with error -1. Disable AI fallback while typing or retry with ? after the model is ready."
        }
        if nsError.localizedDescription.isEmpty {
            return "Apple Intelligence failed."
        }
        return "Apple Intelligence failed: \(nsError.localizedDescription)"
    }
}

// MARK: - Notification name

extension Notification.Name {
    /// Posted by `AIPlugin` when streaming output updates so `LauncherViewModel` can re-query.
    public static let roverAIUpdate = Notification.Name("dev.undivisible.rover.aiUpdate")
}
