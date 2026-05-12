import AppKit
import RoverCore
import SwiftUI

@MainActor
final class EditorWindowController: NSWindowController, NSWindowDelegate {
    static let shared = EditorWindowController()

    private init() {
        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 900, height: 640),
            styleMask: [.titled, .closable, .miniaturizable, .resizable, .fullSizeContentView],
            backing: .buffered,
            defer: false
        )
        window.title = "Editor — Rover"
        window.minSize = NSSize(width: 520, height: 360)
        window.center()
        window.setFrameAutosaveName("RoverEditorWindow")
        super.init(window: window)
        window.delegate = self
    }

    required init?(coder: NSCoder) { nil }

    func show(mode: EditorMode) {
        window?.title = "\(mode.title) — Rover"
        window?.contentView = NSHostingView(rootView: RoverEditorView(mode: mode))
        window?.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
    }
}

private extension EditorMode {
    var title: String {
        switch self {
        case .code: "Code Editor"
        case .text: "Text Editor"
        case .markdown: "Markdown Editor"
        }
    }

    var storageKey: String {
        "rover.editor.\(rawValue).content"
    }

    var placeholder: String {
        switch self {
        case .code: "Write code here..."
        case .text: "Write text here..."
        case .markdown: "# Untitled\n\nStart writing..."
        }
    }
}

private struct RoverEditorView: View {
    let mode: EditorMode

    @State private var text: String
    @State private var preview = false
    @State private var wordCount = 0
    @State private var characterCount = 0

    init(mode: EditorMode) {
        self.mode = mode
        let saved = UserDefaults.standard.string(forKey: mode.storageKey)
        _text = State(initialValue: saved?.isEmpty == false ? saved ?? "" : mode.placeholder)
        _wordCount = State(initialValue: Self.words(in: saved ?? mode.placeholder))
        _characterCount = State(initialValue: (saved ?? mode.placeholder).count)
    }

    var body: some View {
        VStack(spacing: 0) {
            toolbar
            Divider()
            Group {
                if mode == .markdown && preview {
                    ScrollView {
                        markdownPreview
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .padding(28)
                    }
                    .background(Color(nsColor: .textBackgroundColor))
                } else {
                    TextEditor(text: $text)
                        .font(mode == .code ? .system(.body, design: .monospaced) : .system(.body, design: .default))
                        .scrollContentBackground(.hidden)
                        .padding(20)
                        .background(Color(nsColor: .textBackgroundColor))
                }
            }
            Divider()
            statusBar
        }
        .frame(minWidth: 520, minHeight: 360)
        .onChange(of: text) { _, newValue in
            UserDefaults.standard.set(newValue, forKey: mode.storageKey)
            wordCount = Self.words(in: newValue)
            characterCount = newValue.count
        }
    }

    private var toolbar: some View {
        HStack(spacing: 10) {
            Label(mode.title, systemImage: iconName)
                .font(.headline)
            Spacer()
            if mode == .markdown {
                Toggle(isOn: $preview) {
                    Label("Preview", systemImage: preview ? "pencil" : "doc.text.magnifyingglass")
                }
                .toggleStyle(.button)
            }
            Button {
                NSPasteboard.general.clearContents()
                NSPasteboard.general.setString(text, forType: .string)
            } label: {
                Label("Copy", systemImage: "doc.on.doc")
            }
            Button {
                text = ""
            } label: {
                Label("Clear", systemImage: "trash")
            }
        }
        .padding(.horizontal, 16)
        .padding(.vertical, 10)
        .background(.regularMaterial)
    }

    private var statusBar: some View {
        HStack {
            Text(mode.rawValue.capitalized)
            Spacer()
            Text("\(wordCount) words")
            Text("\(characterCount) chars")
        }
        .font(.caption)
        .foregroundStyle(.secondary)
        .padding(.horizontal, 16)
        .padding(.vertical, 8)
        .background(.thinMaterial)
    }

    private var markdownPreview: some View {
        VStack(alignment: .leading, spacing: 12) {
            if let attributed = try? AttributedString(markdown: text) {
                Text(attributed)
                    .textSelection(.enabled)
            } else {
                Text(text)
                    .textSelection(.enabled)
            }
        }
        .font(.system(.body, design: .serif))
        .lineSpacing(4)
    }

    private var iconName: String {
        switch mode {
        case .code: "chevron.left.forwardslash.chevron.right"
        case .text: "text.alignleft"
        case .markdown: "doc.richtext"
        }
    }

    private static func words(in text: String) -> Int {
        text.split { $0.isWhitespace || $0.isNewline }.count
    }
}
