import AppKit
import SwiftUI

@MainActor
final class TerminalWindowController: NSWindowController, NSWindowDelegate {
    static let shared = TerminalWindowController()
    private var lastCommand: String?

    private init() {
        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 720, height: 440),
            styleMask: [.titled, .closable, .miniaturizable, .resizable, .fullSizeContentView],
            backing: .buffered,
            defer: false
        )
        window.title = "Terminal — Rover"
        window.minSize = NSSize(width: 400, height: 260)
        window.center()
        super.init(window: window)
        window.delegate = self
    }

    required init?(coder: NSCoder) { fatalError("not used") }

    func show(command: String? = nil) {
        lastCommand = command
        let termView = GhosttyTerminalView(initialCommand: command)
        let hosting = NSHostingView(rootView: termView)
        window?.contentView = hosting
        window?.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
    }

    func hide() {
        window?.orderOut(nil)
    }

    func windowWillClose(_ notification: Notification) {
        lastCommand = nil
    }
}
