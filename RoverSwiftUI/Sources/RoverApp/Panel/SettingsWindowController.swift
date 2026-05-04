import AppKit
import RoverCore
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
        case ai       = "AI"
        case data     = "Data"
        case about    = "About"
        var id: String { rawValue }

        var icon: String {
            switch self {
            case .general: "gear"
            case .hotkeys: "keyboard"
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
                    .background(RoundedRectangle(cornerRadius: 5).fill(Color.primary.opacity(0.08)))
            }
        }
    }
}

// MARK: - Hotkeys Section

private struct HotkeysSection: View {
    @AppStorage("rover.hotkeyPreset") private var hotkeyPreset = HotkeyPreset.commandSpace.rawValue

    var selectedPreset: Binding<String> {
        Binding {
            hotkeyPreset
        } set: { value in
            hotkeyPreset = value
            NotificationCenter.default.post(name: .roverHotkeyChanged, object: nil)
        }
    }

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
                Picker("", selection: selectedPreset) {
                    ForEach(HotkeyPreset.allCases) { preset in
                        Text(preset.display).tag(preset.rawValue)
                    }
                }
                .labelsHidden()
                .frame(width: 180)
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
    }
}

// MARK: - AI Section

private struct AISection: View {
    @AppStorage("rover.ai_enabled") private var aiEnabled = true

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
        .background(RoundedRectangle(cornerRadius: 4).fill(Color.primary.opacity(0.08)))
        .overlay(RoundedRectangle(cornerRadius: 4).stroke(Color.primary.opacity(0.15)))
}

// MARK: - Import guard for FoundationModels check

#if canImport(FoundationModels)
import FoundationModels
#endif
