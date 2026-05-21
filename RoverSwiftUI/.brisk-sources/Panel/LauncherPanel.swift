import AppKit
import SwiftUI

/// Non-activating NSPanel that hosts the SwiftUI launcher.
///
/// Matches Raycast behaviour:
/// - Click anywhere outside the panel → dismiss
/// - Global event monitor installed while visible
/// - Smooth fade + height animations
@MainActor
final class LauncherPanel: NSPanel {
    private var hostingView: NSHostingView<LauncherView>?
    private let vm: LauncherViewModel

    /// Global mouse-down monitor — active only while the panel is visible.
    private var clickOutsideMonitor: Any?

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

        let view = LauncherView(vm: vm) { [weak self] height in
            self?.animateToHeight(height)
        }
        let hosting = NSHostingView(rootView: view)
        hosting.translatesAutoresizingMaskIntoConstraints = false
        contentView = hosting
        hostingView = hosting

        hosting.setFrameSize(hosting.fittingSize)
        place()
    }

    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { false }

    // MARK: - Show / Hide

    func show(initialQuery: String = "") {
        vm.query = initialQuery
        place()

        // Start transparent, animate in.
        alphaValue = 0
        makeKeyAndOrderFront(nil)
        makeFirstResponder(contentView)

        NSAnimationContext.runAnimationGroup { ctx in
            ctx.duration = 0.18
            ctx.timingFunction = CAMediaTimingFunction(name: .easeOut)
            animator().alphaValue = 1
        }

        installClickOutsideMonitor()
    }

    func hide() {
        removeClickOutsideMonitor()
        NSAnimationContext.runAnimationGroup { ctx in
            ctx.duration = 0.12
            ctx.timingFunction = CAMediaTimingFunction(name: .easeIn)
            animator().alphaValue = 0
        } completionHandler: { [weak self] in
            self?.orderOut(nil)
            self?.alphaValue = 1  // reset for next show
        }
    }

    func toggle() {
        if isVisible { hide() } else { show() }
    }

    func showEmojiPicker() {
        show(initialQuery: ":")
    }

    // MARK: - Smooth height resize

    /// Animate the panel height while anchoring the top edge.
    ///
    /// Called by `LauncherView` whenever its natural height changes (items
    /// appear / disappear). Only runs while the panel is visible to avoid
    /// jarring flashes during a show/hide cycle.
    private func animateToHeight(_ height: CGFloat) {
        guard isVisible, height > 0 else { return }
        let current = self.frame
        guard abs(height - current.height) > 1 else { return }

        let newFrame = frame(forHeight: height)

        NSAnimationContext.runAnimationGroup { ctx in
            ctx.duration = 0.2
            ctx.timingFunction = CAMediaTimingFunction(name: .easeOut)
            animator().setFrame(newFrame, display: true, animate: true)
        }
    }

    // MARK: - Click-outside monitor

    private func installClickOutsideMonitor() {
        guard clickOutsideMonitor == nil else { return }
        clickOutsideMonitor = NSEvent.addGlobalMonitorForEvents(
            matching: [.leftMouseDown, .rightMouseDown, .otherMouseDown]
        ) { [weak self] event in
            guard let self, self.isVisible else { return }
            // If the click landed outside our frame → dismiss.
            let screenPoint = NSEvent.mouseLocation
            if !self.frame.contains(screenPoint) {
                Task { @MainActor in self.hide() }
            }
        }
    }

    private func removeClickOutsideMonitor() {
        if let monitor = clickOutsideMonitor {
            NSEvent.removeMonitor(monitor)
            clickOutsideMonitor = nil
        }
    }

    private func place() {
        setFrame(frame(forHeight: max(frame.height, hostingView?.fittingSize.height ?? 80)), display: true)
    }

    private func frame(forHeight height: CGFloat) -> NSRect {
        let visible = NSScreen.main?.visibleFrame ?? NSRect(x: 0, y: 0, width: 1440, height: 900)
        let width = RoverGlass.panelWidth
        let x = visible.midX - width / 2
        let position = UserDefaults.standard.string(forKey: "rover.bar_position") ?? "center"
        let y: CGFloat
        switch position {
        case "top":
            y = visible.maxY - height - 96
        case "bottom":
            y = visible.minY + 96
        default:
            y = visible.midY - height / 2
        }
        return NSRect(x: x, y: y, width: width, height: height)
    }
}
