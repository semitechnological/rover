import AppKit
import RoverCore
import SwiftUI

/// Non-activating NSPanel that hosts the SwiftUI launcher.
///
/// Stays above all windows, does not steal key status from the frontmost app
/// (LSUIElement = YES is set in Info.plist — no Dock icon, menu-bar only).
@MainActor
final class LauncherPanel: NSPanel {
    private var hostingView: NSHostingView<LauncherView>?
    private let vm: LauncherViewModel

    init(vm: LauncherViewModel) {
        self.vm = vm

        super.init(
            contentRect: .zero,
            styleMask: [.nonactivatingPanel, .fullSizeContentView, .borderless],
            backing: .buffered,
            defer: false
        )

        isFloatingPanel = true
        level = .floating
        isOpaque = false
        backgroundColor = .clear
        hasShadow = true
        collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        isMovableByWindowBackground = false
        titleVisibility = .hidden
        titlebarAppearsTransparent = true

        let view = LauncherView(vm: vm)
        let hosting = NSHostingView(rootView: view)
        hosting.translatesAutoresizingMaskIntoConstraints = false
        contentView = hosting
        hostingView = hosting

        // Size to fit, then center on screen.
        hosting.setFrameSize(hosting.fittingSize)
        center()
    }

    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { false }

    // MARK: - Show / Hide

    func show() {
        vm.query = ""
        // Re-center every time (active space / display may have changed).
        center()
        makeKeyAndOrderFront(nil)
        // Focus the text field inside the SwiftUI view.
        makeFirstResponder(contentView)
    }

    func hide() {
        orderOut(nil)
    }

    func toggle() {
        if isVisible { hide() } else { show() }
    }
}
