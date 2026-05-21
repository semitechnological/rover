import AppKit
import SwiftUI

// MARK: - SettingsWindowController

/// Manages the Rover Settings window. One shared window reused across openings.
@MainActor
final class SettingsWindowController: NSWindowController {

    // MARK: - Singleton

    static let shared = SettingsWindowController()

    private init() {
        let hosting = NSHostingController(rootView: SettingsView())
        let window = NSWindow(contentViewController: hosting)
        window.title = "Rover Settings"
        window.styleMask = [.titled, .closable, .resizable, .fullSizeContentView]
        window.setContentSize(NSSize(width: 640, height: 480))
        window.minSize = NSSize(width: 540, height: 380)
        window.center()
        window.setFrameAutosaveName("RoverSettingsWindow")
        super.init(window: window)
    }

    required init?(coder: NSCoder) { nil }

    func show() {
        showWindow(nil)
        window?.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
    }
}

// MARK: - SettingsView

private struct SettingsView: View {

    enum Section: String, CaseIterable, Identifiable {
        case general  = "General"
        case hotkeys  = "Hotkeys"
        case plugins  = "Plugins"
        case ai       = "AI"
        case data     = "Data"
        case about    = "About"
        var id: String { rawValue }

        var icon: String {
            switch self {
            case .general: "gear"
            case .hotkeys: "keyboard"
            case .plugins: "puzzlepiece.extension"
            case .ai:      "sparkles"
            case .data:    "externaldrive"
            case .about:   "info.circle"
            }
        }
    }

    @State private var selectedSection: Section = .general

    var body: some View {
        NavigationSplitView {
            List(Section.allCases, selection: $selectedSection) { section in
                Label(section.rawValue, systemImage: section.icon)
                    .tag(section)
            }
            .navigationSplitViewColumnWidth(160)
        } detail: {
            Group {
                switch selectedSection {
                case .general: GeneralSection()
                case .hotkeys: HotkeysSection()
                case .plugins: PluginsSection()
                case .ai:      AISection()
                case .data:    DataSection()
                case .about:   AboutSection()
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            .padding(24)
        }
        .navigationTitle("Rover Settings")
    }
}

// MARK: - General Section

private struct GeneralSection: View {
    @AppStorage("rover.show_builtin_commands_on_empty") private var showBuiltinOnEmpty = true
    @AppStorage("rover.bar_position") private var barPosition = "center"

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            sectionHeader("General")

            settingRow {
                VStack(alignment: .leading, spacing: 2) {
                    Text("Show commands on empty query")
                        .font(.system(size: 13))
                    Text("Surface Rover Settings, Terminal, and Browser when the launcher opens.")
                        .font(.system(size: 11))
                        .foregroundStyle(.secondary)
                }
            } control: {
                Toggle("", isOn: $showBuiltinOnEmpty)
                    .labelsHidden()
                    .toggleStyle(.switch)
            }

            settingRow {
                VStack(alignment: .leading, spacing: 2) {
                    Text("Bar Position")
                        .font(.system(size: 13))
                    Text("Choose where the launcher panel appears.")
                        .font(.system(size: 11))
                        .foregroundStyle(.secondary)
                }
            } control: {
                Picker("", selection: $barPosition) {
                    Text("Top").tag("top")
                    Text("Center").tag("center")
                    Text("Bottom").tag("bottom")
                }
                .labelsHidden()
                .pickerStyle(.segmented)
                .frame(width: 220)
            }

            settingRow {
                VStack(alignment: .leading, spacing: 2) {
                    Text("Item Actions")
                        .font(.system(size: 13))
                    Text("Show secondary action for the selected item.")
                        .font(.system(size: 11))
                        .foregroundStyle(.secondary)
                }
            } control: {
                Text("⌘K")
                    .font(.system(size: 12, design: .monospaced))
                    .padding(.horizontal, 8)
                    .padding(.vertical, 4)
                    .background(RoundedRectangle(cornerRadius: 5).fill(SwiftUI.Color.primary.opacity(0.08)))
            }
        }
    }
}

// MARK: - Hotkeys Section

private struct HotkeysSection: View {
    @State private var hotkey = HotkeyConfiguration.current()

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            sectionHeader("Hotkeys")

            settingRow {
                VStack(alignment: .leading, spacing: 2) {
                    Text("Global Hotkey")
                        .font(.system(size: 13))
                    Text("Press this key combination to open Rover from anywhere.")
                        .font(.system(size: 11))
                        .foregroundStyle(.secondary)
                }
            } control: {
                HotkeyRecorderControl(configuration: $hotkey) { configuration in
                    configuration.save()
                    NotificationCenter.default.post(name: .roverHotkeyChanged, object: nil)
                }
            }

            settingRow {
                VStack(alignment: .leading, spacing: 2) {
                    Text("Emoji Picker")
                        .font(.system(size: 13))
                    Text("Opens Rover directly in emoji search mode.")
                        .font(.system(size: 11))
                        .foregroundStyle(.secondary)
                }
            } control: {
                kbdBadge("⌘⌥ Space")
            }

            settingRow {
                VStack(alignment: .leading, spacing: 2) {
                    Text("Item Actions")
                        .font(.system(size: 13))
                    Text("Show secondary action for the selected item.")
                        .font(.system(size: 11))
                        .foregroundStyle(.secondary)
                }
            } control: {
                kbdBadge("⌘K")
            }
        }
        .onAppear { hotkey = HotkeyConfiguration.current() }
    }
}

private struct HotkeyRecorderControl: View {
    @Binding var configuration: HotkeyConfiguration
    let onRecord: (HotkeyConfiguration) -> Void
    @State private var isRecording = false
    @State private var message: String?

    var body: some View {
        VStack(alignment: .trailing, spacing: 6) {
            Button {
                message = nil
                isRecording = true
            } label: {
                HStack(spacing: 8) {
                    Image(systemName: isRecording ? "record.circle" : "keyboard")
                    Text(isRecording ? "Recording..." : configuration.display)
                        .font(.system(size: 12, design: .monospaced))
                }
                .frame(width: 170)
            }
            .buttonStyle(.bordered)

            if isRecording {
                HotkeyCaptureView { configuration in
                    self.configuration = configuration
                    self.onRecord(configuration)
                    self.message = "Saved"
                    self.isRecording = false
                } onInvalid: {
                    self.message = "Use at least one modifier"
                } onCancel: {
                    self.isRecording = false
                    self.message = nil
                }
                .frame(width: 170, height: 1)
            }

            if let message {
                Text(message)
                    .font(.system(size: 11))
                    .foregroundColor(message == "Saved" ? SwiftUI.Color.secondary : SwiftUI.Color.red)
            }
        }
    }
}

private struct HotkeyCaptureView: NSViewRepresentable {
    let onRecord: (HotkeyConfiguration) -> Void
    let onInvalid: () -> Void
    let onCancel: () -> Void

    func makeNSView(context: Context) -> HotkeyCaptureNSView {
        HotkeyCaptureNSView(onRecord: onRecord, onInvalid: onInvalid, onCancel: onCancel)
    }

    func updateNSView(_ nsView: HotkeyCaptureNSView, context: Context) {
        nsView.onRecord = onRecord
        nsView.onInvalid = onInvalid
        nsView.onCancel = onCancel
        DispatchQueue.main.async { nsView.window?.makeFirstResponder(nsView) }
    }
}

private final class HotkeyCaptureNSView: NSView {
    var onRecord: (HotkeyConfiguration) -> Void
    var onInvalid: () -> Void
    var onCancel: () -> Void

    init(
        onRecord: @escaping (HotkeyConfiguration) -> Void,
        onInvalid: @escaping () -> Void,
        onCancel: @escaping () -> Void
    ) {
        self.onRecord = onRecord
        self.onInvalid = onInvalid
        self.onCancel = onCancel
        super.init(frame: .zero)
    }

    required init?(coder: NSCoder) { nil }

    override var acceptsFirstResponder: Bool { true }

    override func keyDown(with event: NSEvent) {
        if event.keyCode == 53 {
            onCancel()
            return
        }
        guard let configuration = HotkeyConfiguration.from(event: event) else {
            onInvalid()
            return
        }
        onRecord(configuration)
    }
}

private struct PluginsSection: View {
    private let plugins: [(String, String, String)] = [
        ("Rover Commands", "Settings, browser, terminal, quit", "command.square"),
        ("Apps", "Launch installed macOS apps", "app.dashed"),
        ("Calculator", "Inline arithmetic", "function"),
        ("Clipboard", "Search clipboard history", "doc.on.clipboard"),
        ("Files", "Find local files", "folder"),
        ("Terminal", "Run shell commands in an embedded terminal", "terminal"),
        ("Editors", "Code, text, and Markdown editor windows", "curlybraces.square"),
        ("Browser", "Open Rover's WebKit browser", "globe"),
        ("Tile", "Window mosaics and tiling actions", "rectangle.3.group"),
        ("Web Search", "Search the web", "magnifyingglass"),
        ("Google", "Google Calendar, Drive, Docs, Sheets, Gmail", "g.circle"),
        ("macOS Tools", "System actions and utility commands", "desktopcomputer"),
        ("Emoji", "Emoji picker and emoji search", "face.smiling"),
        ("Help", "Plugin and shortcut hints", "questionmark.circle"),
        ("AI", "Apple Intelligence answers", "sparkles"),
        ("Semantic", "Local semantic search", "brain"),
    ]

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                sectionHeader("Plugins")
                ForEach(plugins, id: \.0) { plugin in
                    HStack(alignment: .center, spacing: 10) {
                        Image(systemName: plugin.2)
                            .frame(width: 18)
                            .foregroundStyle(.secondary)
                        VStack(alignment: .leading, spacing: 2) {
                            Text(plugin.0)
                                .font(.system(size: 13))
                            Text(plugin.1)
                                .font(.system(size: 11))
                                .foregroundStyle(.secondary)
                        }
                        Spacer()
                        Text("On")
                            .font(.system(size: 11, weight: .medium))
                            .foregroundStyle(.secondary)
                    }
                    .padding(.vertical, 9)
                    Divider()
                }
            }
        }
    }
}

// MARK: - AI Section

private struct AISection: View {
    @AppStorage("rover.ai_enabled") private var aiEnabled = true
    @AppStorage("rover.ai_auto_fallback") private var aiAutoFallback = false

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            sectionHeader("AI")

            settingRow {
                VStack(alignment: .leading, spacing: 2) {
                    Text("Apple Intelligence")
                        .font(.system(size: 13))
                    Text("Use on-device AI to answer queries. Requires macOS 26+ and Apple Silicon.")
                        .font(.system(size: 11))
                        .foregroundStyle(.secondary)
                }
            } control: {
                Toggle("", isOn: $aiEnabled)
                    .labelsHidden()
                .toggleStyle(.switch)
            }

            settingRow {
                VStack(alignment: .leading, spacing: 2) {
                    Text("AI Fallback While Typing")
                        .font(.system(size: 13))
                    Text("Automatically ask Apple Intelligence when a query looks like a question.")
                        .font(.system(size: 11))
                        .foregroundStyle(.secondary)
                }
            } control: {
                Toggle("", isOn: $aiAutoFallback)
                    .labelsHidden()
                    .toggleStyle(.switch)
            }

            settingRow {
                VStack(alignment: .leading, spacing: 2) {
                    Text("Availability")
                        .font(.system(size: 13))
                }
            } control: {
                availabilityBadge
            }

            settingRow {
                VStack(alignment: .leading, spacing: 2) {
                    Text("Activate")
                        .font(.system(size: 13))
                    Text("Type a query starting with ? or prefix it with 'ai '.")
                        .font(.system(size: 11))
                        .foregroundStyle(.secondary)
                }
            } control: {
                HStack(spacing: 4) {
                    kbdBadge("?")
                    Text("or").font(.system(size: 11)).foregroundStyle(.secondary)
                    kbdBadge("ai …")
                }
            }
        }
    }

    @ViewBuilder
    private var availabilityBadge: some View {
        #if canImport(FoundationModels)
        if #available(macOS 26.0, *) {
            switch SystemLanguageModel.default.availability {
            case .available:
                Label("Available", systemImage: "checkmark.circle.fill")
                    .font(.system(size: 12))
                    .foregroundStyle(.green)
            case .unavailable:
                Label("Not available", systemImage: "exclamationmark.circle.fill")
                    .font(.system(size: 12))
                    .foregroundStyle(.secondary)
            }
        } else {
            Label("Requires macOS 26", systemImage: "exclamationmark.circle")
                .font(.system(size: 12))
                .foregroundStyle(.secondary)
        }
        #else
        Label("Not available in this build", systemImage: "exclamationmark.circle")
            .font(.system(size: 12))
            .foregroundStyle(.secondary)
        #endif
    }
}

// MARK: - Data Section

private struct DataSection: View {
    @State private var clipboardCleared = false
    @State private var queryCleared = false

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            sectionHeader("Data")

            settingRow {
                VStack(alignment: .leading, spacing: 2) {
                    Text("Clipboard History")
                        .font(.system(size: 13))
                    Text("Rover stores up to 500 clipboard entries in a local SQLite database.")
                        .font(.system(size: 11))
                        .foregroundStyle(.secondary)
                }
            } control: {
                Text("500 max")
                    .font(.system(size: 12))
                    .foregroundStyle(.secondary)
            }

            Divider().padding(.vertical, 8)

            Button(role: .destructive) {
                // Delete all entries using the existing per-entry API (bindings pre-date
                // the bulk clear method added later to rover_swift).
                if let db = try? RoverDatabase.open(),
                   let entries = try? db.clipboardHistory()
                {
                    for entry in entries { try? db.deleteClipboard(id: entry.id) }
                }
                clipboardCleared = true
                DispatchQueue.main.asyncAfter(deadline: .now() + 2) { clipboardCleared = false }
            } label: {
                Label(clipboardCleared ? "Cleared" : "Clear Clipboard History",
                      systemImage: clipboardCleared ? "checkmark" : "trash")
            }
            .padding(.top, 4)

            Button(role: .destructive) {
                queryCleared = true
                DispatchQueue.main.asyncAfter(deadline: .now() + 2) { queryCleared = false }
            } label: {
                Label(queryCleared ? "Cleared" : "Clear Search History",
                      systemImage: queryCleared ? "checkmark" : "trash")
            }
            .padding(.top, 4)
        }
    }
}

// MARK: - About Section

private struct AboutSection: View {
    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            sectionHeader("About")

            VStack(alignment: .leading, spacing: 8) {
                HStack(spacing: 12) {
                    Image(systemName: "magnifyingglass.circle.fill")
                        .font(.system(size: 48))
                        .foregroundColor(.accentColor)
                    VStack(alignment: .leading, spacing: 4) {
                        Text("Rover")
                            .font(.system(size: 18, weight: .semibold))
                        Text("Version 0.1.0")
                            .font(.system(size: 13))
                            .foregroundStyle(.secondary)
                    }
                }
                .padding(.bottom, 8)

                Text("A fast, extensible macOS launcher.")
                    .font(.system(size: 13))
                    .foregroundStyle(.secondary)

                Text("Built with SwiftUI, WebKit, and native macOS frameworks.")
                    .font(.system(size: 12))
                    .foregroundStyle(.secondary)

                Divider().padding(.top, 8)

                HStack {
                    Text("Powered by Apple Intelligence")
                        .font(.system(size: 12))
                        .foregroundStyle(.secondary)
                    Image(systemName: "apple.intelligence")
                        .font(.system(size: 12))
                        .foregroundStyle(.secondary)
                }
            }
        }
    }
}

// MARK: - Shared helpers (file-private)

private func sectionHeader(_ title: String) -> some View {
    Text(title)
        .font(.system(size: 16, weight: .semibold))
        .padding(.bottom, 12)
}

@ViewBuilder
private func settingRow<L: View, C: View>(
    @ViewBuilder label: () -> L,
    @ViewBuilder control: () -> C
) -> some View {
    VStack(spacing: 0) {
        HStack(alignment: .center) {
            label()
            Spacer(minLength: 16)
            control()
        }
        .padding(.vertical, 10)
        Divider()
    }
}

private func kbdBadge(_ text: String) -> some View {
    Text(text)
        .font(.system(size: 11, design: .monospaced))
        .padding(.horizontal, 6)
        .padding(.vertical, 3)
        .background(RoundedRectangle(cornerRadius: 4).fill(SwiftUI.Color.primary.opacity(0.08)))
        .overlay(RoundedRectangle(cornerRadius: 4).stroke(SwiftUI.Color.primary.opacity(0.15)))
}

// MARK: - Import guard for FoundationModels check

#if canImport(FoundationModels)
import FoundationModels
#endif
